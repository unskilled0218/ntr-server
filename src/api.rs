//! HTTP API routes.

use crate::config::Config;
use crate::labelstore::{LabelStore, Labels};
use crate::note::NoteStore;
use crate::store::{Entry, Filter, Store, StoreError};
use crate::task_backend::{Mutation, TaskBackend};
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::io;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

/// How many live-update events the broadcast channel buffers per subscriber before a
/// slow client starts missing them (it then re-syncs on reconnect). The signal is tiny,
/// so a small buffer is plenty.
const EVENT_CHANNEL_CAPACITY: usize = 16;

/// Which resource changed, broadcast to live clients over the SSE stream. This is the
/// whole wire contract for an event: [`Change::as_str`] is the payload. SSE clients dispatch
/// on these same two strings to decide which resource to re-fetch — keep the wire payloads and
/// any consumer in sync.
#[derive(Debug, Clone, Copy)]
pub enum Change {
  Tasks,
  Note,
}

/// Origin is advisory routing metadata, never authentication.
#[derive(Debug, Clone)]
pub struct Notification {
  pub change: Change,
  pub origin: Option<String>,
}

fn client_id(headers: &HeaderMap) -> Option<String> {
  headers
    .get("x-ntr-client-id")
    .and_then(|value| value.to_str().ok())
    .filter(|value| !value.is_empty() && value.len() <= 128)
    .map(str::to_owned)
}

impl Change {
  fn as_str(self) -> &'static str {
    match self {
      Change::Tasks => "tasks",
      Change::Note => "note",
    }
  }
}

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
  pub store: TaskBackend,
  pub note: Arc<RwLock<NoteStore>>,
  pub labels: Arc<RwLock<LabelStore>>,
  pub config: Arc<Config>,
  /// Sender half of the live-update channel. The watcher (the sole producer) calls
  /// [`AppState::notify`]; each SSE client connection subscribes a fresh receiver.
  pub tx: broadcast::Sender<Notification>,
}

impl AppState {
  pub fn new(store: Store, note: NoteStore, labels: LabelStore, config: Config) -> Self {
    let (tx, _rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
    AppState {
      store: TaskBackend::file(store),
      note: Arc::new(RwLock::new(note)),
      labels: Arc::new(RwLock::new(labels)),
      config: Arc::new(config),
      tx,
    }
  }

  pub fn from_config(config: Config) -> Result<Self, AppError> {
    let (tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
    Ok(Self {
      store: TaskBackend::from_config(&config, tx.clone())?,
      note: Arc::new(RwLock::new(NoteStore::load(&config.note_file)?)),
      labels: Arc::new(RwLock::new(LabelStore::load(&config.labels_file)?)),
      config: Arc::new(config),
      tx,
    })
  }

  /// Announce a change to every connected live client. A send with no subscribers is
  /// fine (returns `Err`, ignored), so this is always safe to call.
  ///
  /// Called from two places, because a change has two origins the other cannot see:
  /// (1) every mutation handler, after a successful write — these update memory and disk
  /// together, so the watcher (which fires only when disk and memory *differ*) never sees
  /// them; and (2) the file watcher, for out-of-band edits made to the file behind the
  /// server's back. The two paths never double-fire for one change: an API write leaves
  /// disk == memory, so the watcher stays silent for it.
  pub fn notify(&self, change: Change) {
    self.notify_from(change, None);
  }

  fn notify_from(&self, change: Change, origin: Option<String>) {
    let _ = self.tx.send(Notification { change, origin });
  }
}

/// Build the router with all API routes wired up.
pub fn router(state: AppState) -> Router {
  Router::new()
    .route("/api/health", get(health))
    .route("/api/capabilities", get(capabilities))
    .route("/api/tasks", get(list_tasks).post(create_task))
    .route(
      "/api/tasks/{id}",
      get(get_task).put(update_task).delete(delete_task),
    )
    .route("/api/tasks/{id}/complete", post(complete_task))
    .route("/api/tasks/{id}/uncomplete", post(uncomplete_task))
    .route("/api/raw", get(get_raw).put(put_raw))
    .route("/api/note", get(get_note).put(put_note))
    .route("/api/labels", get(get_labels).put(put_labels))
    .route("/api/events", get(events))
    // No auth by design (userless server); permissive CORS lets other
    // programs and origins call the API. Bind to loopback unless trusted.
    .layer(CorsLayer::permissive())
    .with_state(state)
}

/// JSON representation of a task.
#[derive(Debug, Serialize)]
struct TaskDto {
  id: u64,
  completed: bool,
  completion_date: Option<String>,
  creation_date: Option<String>,
  description: String,
  projects: Vec<String>,
  contexts: Vec<String>,
  tags: BTreeMap<String, String>,
  raw: String,
}

impl TaskDto {
  fn from_entry(e: &Entry) -> Self {
    let d = e.task.derived();
    TaskDto {
      id: e.id,
      completed: e.task.completed,
      completion_date: e.task.completion_date.map(|x| x.to_string()),
      creation_date: e.task.creation_date.map(|x| x.to_string()),
      description: e.task.description.clone(),
      projects: d.projects,
      contexts: d.contexts,
      tags: d.tags,
      raw: e.task.to_string(),
    }
  }
}

/// Request body carrying a raw todo.txt line.
#[derive(Debug, Deserialize)]
struct TextRequest {
  text: String,
}

/// Query parameters for listing tasks.
#[derive(Debug, Deserialize)]
struct ListQuery {
  completed: Option<bool>,
  project: Option<String>,
  context: Option<String>,
}

/// API error mapped to an HTTP status and JSON body.
#[derive(Debug)]
pub enum AppError {
  NotFound,
  BadRequest(String),
  Internal(String),
  Upstream(String),
  Unavailable(String),
  Conflict(String),
  Timeout,
}

impl IntoResponse for AppError {
  fn into_response(self) -> Response {
    let (status, message) = match self {
      AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
      AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
      AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
      AppError::Upstream(m) => (StatusCode::BAD_GATEWAY, m),
      AppError::Unavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
      AppError::Conflict(m) => (StatusCode::CONFLICT, m),
      AppError::Timeout => (
        StatusCode::GATEWAY_TIMEOUT,
        "Home Assistant timed out; a write may have committed. Reload before retrying.".into(),
      ),
    };
    (status, Json(json!({ "error": message }))).into_response()
  }
}

impl From<StoreError> for AppError {
  fn from(e: StoreError) -> Self {
    match e {
      StoreError::NotFound => AppError::NotFound,
      StoreError::Invalid(m) => AppError::BadRequest(m),
      StoreError::Io(e) => AppError::Internal(e.to_string()),
    }
  }
}

impl From<io::Error> for AppError {
  fn from(e: io::Error) -> Self {
    AppError::Internal(e.to_string())
  }
}

impl std::fmt::Display for AppError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self:?}")
  }
}
impl std::error::Error for AppError {}

async fn health() -> Json<serde_json::Value> {
  Json(json!({ "status": "ok" }))
}

async fn capabilities(State(state): State<AppState>) -> Json<serde_json::Value> {
  Json(
    json!({ "task_backend": if state.store.is_ha() { "homeassistant" } else { "file" }, "raw_task_replace": !state.store.is_ha() }),
  )
}

async fn list_tasks(
  State(state): State<AppState>,
  OriginalUri(uri): OriginalUri,
  Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<Vec<TaskDto>>, AppError> {
  if pairs.iter().any(|(key, _)| key == "priority") {
    return Err(AppError::BadRequest(
      "Priority filtering is no longer supported".into(),
    ));
  }
  // Parse supported fields with the original typed query validation after checking removed keys.
  let q = Query::<ListQuery>::try_from_uri(&uri)
    .map_err(|err| AppError::BadRequest(err.to_string()))?
    .0;
  let filter = Filter {
    completed: q.completed,
    project: q.project,
    context: q.context,
  };
  let tasks = state
    .store
    .list(&filter)
    .await?
    .into_iter()
    .map(|e| TaskDto::from_entry(&e))
    .collect();
  Ok(Json(tasks))
}

async fn create_task(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(req): Json<TextRequest>,
) -> Result<(StatusCode, Json<TaskDto>), AppError> {
  let entry = state
    .store
    .mutate(
      Mutation::Create(req.text),
      &state.config,
      &state.tx,
      client_id(&headers),
    )
    .await?
    .ok_or_else(|| AppError::Internal("Missing created task".into()))?;
  let dto = TaskDto::from_entry(&entry);
  Ok((StatusCode::CREATED, Json(dto)))
}

async fn get_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
) -> Result<Json<TaskDto>, AppError> {
  let entry = state.store.get(id).await?;
  Ok(Json(TaskDto::from_entry(&entry)))
}

async fn update_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
  headers: HeaderMap,
  Json(req): Json<TextRequest>,
) -> Result<Json<TaskDto>, AppError> {
  let entry = state
    .store
    .mutate(
      Mutation::Update(id, req.text),
      &state.config,
      &state.tx,
      client_id(&headers),
    )
    .await?
    .ok_or(AppError::NotFound)?;
  let dto = TaskDto::from_entry(&entry);
  Ok(Json(dto))
}

async fn complete_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
  headers: HeaderMap,
) -> Result<Json<TaskDto>, AppError> {
  let entry = state
    .store
    .mutate(
      Mutation::Complete(id),
      &state.config,
      &state.tx,
      client_id(&headers),
    )
    .await?
    .ok_or(AppError::NotFound)?;
  let dto = TaskDto::from_entry(&entry);
  Ok(Json(dto))
}

async fn uncomplete_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
  headers: HeaderMap,
) -> Result<Json<TaskDto>, AppError> {
  let entry = state
    .store
    .mutate(
      Mutation::Reopen(id),
      &state.config,
      &state.tx,
      client_id(&headers),
    )
    .await?
    .ok_or(AppError::NotFound)?;
  let dto = TaskDto::from_entry(&entry);
  Ok(Json(dto))
}

async fn delete_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
  headers: HeaderMap,
) -> Result<StatusCode, AppError> {
  state
    .store
    .mutate(
      Mutation::Delete(id),
      &state.config,
      &state.tx,
      client_id(&headers),
    )
    .await?;
  Ok(StatusCode::NO_CONTENT)
}

async fn get_raw(State(state): State<AppState>) -> Result<Response, AppError> {
  let body = state.store.raw().await?;
  Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response())
}

async fn put_raw(
  State(state): State<AppState>,
  headers: HeaderMap,
  body: String,
) -> Result<StatusCode, AppError> {
  state.store.replace_raw(&body)?;
  state.notify_from(Change::Tasks, client_id(&headers));
  Ok(StatusCode::NO_CONTENT)
}

async fn get_note(State(state): State<AppState>) -> Result<Response, AppError> {
  let note = state.note.read().map_err(lock_err)?;
  let headers = [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")];
  Ok((headers, note.content().to_string()).into_response())
}

async fn put_note(
  State(state): State<AppState>,
  headers: HeaderMap,
  body: String,
) -> Result<StatusCode, AppError> {
  let mut note = state.note.write().map_err(lock_err)?;
  note.replace(&body)?;
  state.notify_from(Change::Note, client_id(&headers));
  Ok(StatusCode::NO_CONTENT)
}

async fn get_labels(State(state): State<AppState>) -> Result<Json<Labels>, AppError> {
  let labels = state.labels.read().map_err(lock_err)?;
  Ok(Json(labels.get()))
}

async fn put_labels(
  State(state): State<AppState>,
  Json(labels): Json<Labels>,
) -> Result<StatusCode, AppError> {
  let mut store = state.labels.write().map_err(lock_err)?;
  store.replace(labels)?;
  Ok(StatusCode::NO_CONTENT)
}

/// Live-update stream: a Server-Sent Events connection that emits one `change` event
/// (payload "tasks" or "note") each time that resource changes — whether from an API
/// mutation or an out-of-band file edit (see [`AppState::notify`]). Clients re-fetch the
/// named resource through the normal GET endpoints. A lagged subscriber (one that fell
/// behind the buffer) skips the missed events and
/// emits both resource tokens to recover lost changes, then resumes buffered events.
async fn events(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
  let rx = state.tx.subscribe();
  let origin = client_id(&headers);
  let stream = stream::unfold(
    (rx, origin, false),
    |(mut rx, origin, rescan_note)| async move {
      if rescan_note {
        return Some((
          Ok(Event::default().event("change").data("note")),
          (rx, origin, false),
        ));
      }
      loop {
        match rx.recv().await {
          Ok(notification) => {
            if origin.is_some() && notification.origin == origin {
              continue;
            }
            let event = Event::default()
              .event("change")
              .data(notification.change.as_str());
            return Some((Ok(event), (rx, origin, false)));
          }
          Err(broadcast::error::RecvError::Lagged(_)) => {
            return Some((
              Ok(Event::default().event("change").data("tasks")),
              (rx, origin, true),
            ));
          }
          Err(broadcast::error::RecvError::Closed) => return None,
        }
      }
    },
  );
  Sse::new(stream).keep_alive(KeepAlive::default())
}

fn lock_err<E>(_: E) -> AppError {
  AppError::Internal("store lock poisoned".to_string())
}
