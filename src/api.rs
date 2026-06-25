//! HTTP API routes.

use crate::config::Config;
use crate::note::NoteStore;
use crate::store::{Entry, Filter, Store, StoreError};
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Local, NaiveDate};
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
  pub store: Arc<RwLock<Store>>,
  pub note: Arc<RwLock<NoteStore>>,
  pub config: Arc<Config>,
  /// Sender half of the live-update channel. The watcher (the sole producer) calls
  /// [`AppState::notify`]; each SSE client connection subscribes a fresh receiver.
  pub tx: broadcast::Sender<Change>,
}

impl AppState {
  pub fn new(store: Store, note: NoteStore, config: Config) -> Self {
    let (tx, _rx) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
    AppState {
      store: Arc::new(RwLock::new(store)),
      note: Arc::new(RwLock::new(note)),
      config: Arc::new(config),
      tx,
    }
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
    let _ = self.tx.send(change);
  }
}

/// Build the router with all API routes wired up.
pub fn router(state: AppState) -> Router {
  Router::new()
    .route("/api/health", get(health))
    .route("/api/tasks", get(list_tasks).post(create_task))
    .route(
      "/api/tasks/{id}",
      get(get_task).put(update_task).delete(delete_task),
    )
    .route("/api/tasks/{id}/complete", post(complete_task))
    .route("/api/tasks/{id}/uncomplete", post(uncomplete_task))
    .route("/api/raw", get(get_raw).put(put_raw))
    .route("/api/note", get(get_note).put(put_note))
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
  priority: Option<char>,
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
      priority: e.task.priority,
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
  priority: Option<char>,
  project: Option<String>,
  context: Option<String>,
}

/// API error mapped to an HTTP status and JSON body.
#[derive(Debug)]
pub enum AppError {
  NotFound,
  BadRequest(String),
  Internal(String),
}

impl IntoResponse for AppError {
  fn into_response(self) -> Response {
    let (status, message) = match self {
      AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
      AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
      AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
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

fn today() -> NaiveDate {
  Local::now().date_naive()
}

async fn health() -> Json<serde_json::Value> {
  Json(json!({ "status": "ok" }))
}

async fn list_tasks(
  State(state): State<AppState>,
  Query(q): Query<ListQuery>,
) -> Result<Json<Vec<TaskDto>>, AppError> {
  let store = state.store.read().map_err(lock_err)?;
  let filter = Filter {
    completed: q.completed,
    priority: q.priority,
    project: q.project,
    context: q.context,
  };
  let tasks = store
    .list(&filter)
    .into_iter()
    .map(TaskDto::from_entry)
    .collect();
  Ok(Json(tasks))
}

async fn create_task(
  State(state): State<AppState>,
  Json(req): Json<TextRequest>,
) -> Result<(StatusCode, Json<TaskDto>), AppError> {
  let mut store = state.store.write().map_err(lock_err)?;
  let id = store.add(&req.text, state.config.add_creation_date, today())?;
  let dto = TaskDto::from_entry(store.get(id).expect("task was just added"));
  state.notify(Change::Tasks);
  Ok((StatusCode::CREATED, Json(dto)))
}

async fn get_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
) -> Result<Json<TaskDto>, AppError> {
  let store = state.store.read().map_err(lock_err)?;
  let entry = store.get(id).ok_or(AppError::NotFound)?;
  Ok(Json(TaskDto::from_entry(entry)))
}

async fn update_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
  Json(req): Json<TextRequest>,
) -> Result<Json<TaskDto>, AppError> {
  let mut store = state.store.write().map_err(lock_err)?;
  store.update(id, &req.text)?;
  let dto = TaskDto::from_entry(store.get(id).expect("task exists"));
  state.notify(Change::Tasks);
  Ok(Json(dto))
}

async fn complete_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
) -> Result<Json<TaskDto>, AppError> {
  let mut store = state.store.write().map_err(lock_err)?;
  store.complete(id, today())?;
  let dto = TaskDto::from_entry(store.get(id).expect("task exists"));
  state.notify(Change::Tasks);
  Ok(Json(dto))
}

async fn uncomplete_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
) -> Result<Json<TaskDto>, AppError> {
  let mut store = state.store.write().map_err(lock_err)?;
  store.uncomplete(id)?;
  let dto = TaskDto::from_entry(store.get(id).expect("task exists"));
  state.notify(Change::Tasks);
  Ok(Json(dto))
}

async fn delete_task(
  State(state): State<AppState>,
  Path(id): Path<u64>,
) -> Result<StatusCode, AppError> {
  let mut store = state.store.write().map_err(lock_err)?;
  store.delete(id)?;
  state.notify(Change::Tasks);
  Ok(StatusCode::NO_CONTENT)
}

async fn get_raw(State(state): State<AppState>) -> Result<Response, AppError> {
  let store = state.store.read().map_err(lock_err)?;
  let body = store.raw();
  Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response())
}

async fn put_raw(State(state): State<AppState>, body: String) -> Result<StatusCode, AppError> {
  let mut store = state.store.write().map_err(lock_err)?;
  store.replace_raw(&body)?;
  state.notify(Change::Tasks);
  Ok(StatusCode::NO_CONTENT)
}

async fn get_note(State(state): State<AppState>) -> Result<Response, AppError> {
  let note = state.note.read().map_err(lock_err)?;
  let headers = [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")];
  Ok((headers, note.content().to_string()).into_response())
}

async fn put_note(State(state): State<AppState>, body: String) -> Result<StatusCode, AppError> {
  let mut note = state.note.write().map_err(lock_err)?;
  note.replace(&body)?;
  state.notify(Change::Note);
  Ok(StatusCode::NO_CONTENT)
}

/// Live-update stream: a Server-Sent Events connection that emits one `change` event
/// (payload "tasks" or "note") each time that resource changes — whether from an API
/// mutation or an out-of-band file edit (see [`AppState::notify`]). Clients re-fetch the
/// named resource through the normal GET endpoints. A lagged subscriber (one that fell
/// behind the buffer) skips the missed events and
/// keeps streaming; it re-syncs on its next event or on reconnect.
async fn events(
  State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
  let rx = state.tx.subscribe();
  let stream = stream::unfold(rx, |mut rx| async move {
    loop {
      match rx.recv().await {
        Ok(change) => {
          let event = Event::default().event("change").data(change.as_str());
          return Some((Ok(event), rx));
        }
        Err(broadcast::error::RecvError::Lagged(_)) => continue,
        Err(broadcast::error::RecvError::Closed) => return None,
      }
    }
  });
  Sse::new(stream).keep_alive(KeepAlive::default())
}

fn lock_err<E>(_: E) -> AppError {
  AppError::Internal("store lock poisoned".to_string())
}
