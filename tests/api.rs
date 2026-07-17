//! Integration tests driving the axum router directly via `oneshot`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use my_ntr_server::api::{router, AppState, Change};
use my_ntr_server::config::Config;
use my_ntr_server::labelstore::LabelStore;
use my_ntr_server::note::NoteStore;
use my_ntr_server::store::Store;
use serde_json::Value;
use std::path::PathBuf;
use tower::ServiceExt;

fn temp_path(tag: &str) -> PathBuf {
  let mut p = std::env::temp_dir();
  p.push(format!("ntr-api-{}-{}.txt", tag, std::process::id()));
  let _ = std::fs::remove_file(&p);
  p
}

fn app(tag: &str) -> axum::Router {
  app_with_state(tag).0
}

// Build the router together with its AppState, so a test can both drive HTTP requests and
// observe the live-update channel. `app` delegates here, keeping the construction rule in
// one place.
fn app_with_state(tag: &str) -> (axum::Router, AppState) {
  let path = temp_path(tag);
  let note_path = temp_path(&format!("{tag}-note"));
  let labels_path = temp_path(&format!("{tag}-labels"));
  let store = Store::load(&path).unwrap();
  let note = NoteStore::load(&note_path).unwrap();
  let labels = LabelStore::load(&labels_path).unwrap();
  let config = Config {
    file: path,
    note_file: note_path,
    labels_file: labels_path,
    ..Config::default()
  };
  let state = AppState::new(store, note, labels, config);
  (router(state.clone()), state)
}

async fn body_bytes(resp: axum::response::Response) -> Vec<u8> {
  axum::body::to_bytes(resp.into_body(), usize::MAX)
    .await
    .unwrap()
    .to_vec()
}

async fn body_json(resp: axum::response::Response) -> Value {
  serde_json::from_slice(&body_bytes(resp).await).unwrap()
}

async fn body_text(resp: axum::response::Response) -> String {
  String::from_utf8(body_bytes(resp).await).unwrap()
}

fn get(uri: &str) -> Request<Body> {
  Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn put_text(uri: &str, body: &'static str) -> Request<Body> {
  Request::builder()
    .method("PUT")
    .uri(uri)
    .header("content-type", "text/plain")
    .body(Body::from(body))
    .unwrap()
}

fn put_json(uri: &str, body: Value) -> Request<Body> {
  Request::builder()
    .method("PUT")
    .uri(uri)
    .header("content-type", "application/json")
    .body(Body::from(body.to_string()))
    .unwrap()
}

fn json_post(path: &str, body: Value) -> Request<Body> {
  Request::builder()
    .method("POST")
    .uri(path)
    .header("content-type", "application/json")
    .body(Body::from(body.to_string()))
    .unwrap()
}

#[tokio::test]
async fn health_ok() {
  let app = app("health");
  let resp = app
    .oneshot(
      Request::builder()
        .uri("/api/health")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  let json = body_json(resp).await;
  assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn create_then_list() {
  let app = app("create-list");

  let resp = app
    .clone()
    .oneshot(json_post(
      "/api/tasks",
      serde_json::json!({ "text": "(A) buy milk +groceries" }),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::CREATED);
  let created = body_json(resp).await;
  assert_eq!(created["id"], 1);
  assert_eq!(created["priority"], "A");
  assert_eq!(created["projects"][0], "groceries");

  let resp = app
    .oneshot(
      Request::builder()
        .uri("/api/tasks")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  let list = body_json(resp).await;
  assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn get_missing_is_404() {
  let app = app("missing");
  let resp = app
    .oneshot(
      Request::builder()
        .uri("/api/tasks/42")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_with_empty_text_is_400() {
  let app = app("empty");
  let resp = app
    .oneshot(json_post(
      "/api/tasks",
      serde_json::json!({ "text": "   " }),
    ))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn complete_and_uncomplete() {
  let app = app("complete");

  app
    .clone()
    .oneshot(json_post(
      "/api/tasks",
      serde_json::json!({ "text": "(B) write tests" }),
    ))
    .await
    .unwrap();

  let resp = app
    .clone()
    .oneshot(json_post("/api/tasks/1/complete", serde_json::json!({})))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  let done = body_json(resp).await;
  assert_eq!(done["completed"], true);
  assert!(done["completion_date"].is_string());

  let resp = app
    .oneshot(json_post("/api/tasks/1/uncomplete", serde_json::json!({})))
    .await
    .unwrap();
  let back = body_json(resp).await;
  assert_eq!(back["completed"], false);
  assert_eq!(back["priority"], "B");
}

#[tokio::test]
async fn delete_returns_204_then_404() {
  let app = app("delete");

  app
    .clone()
    .oneshot(json_post(
      "/api/tasks",
      serde_json::json!({ "text": "throwaway" }),
    ))
    .await
    .unwrap();

  let resp = app
    .clone()
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri("/api/tasks/1")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::NO_CONTENT);

  let resp = app
    .oneshot(
      Request::builder()
        .uri("/api/tasks/1")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn raw_get_and_replace() {
  let app = app("raw");

  let resp = app
    .clone()
    .oneshot(
      Request::builder()
        .method("PUT")
        .uri("/api/raw")
        .header("content-type", "text/plain")
        .body(Body::from("(A) one\n2026-06-19 two +proj\n"))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::NO_CONTENT);

  let resp = app
    .oneshot(
      Request::builder()
        .uri("/api/tasks")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  let list = body_json(resp).await;
  assert_eq!(list.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn note_get_and_replace() {
  let app = app("note");

  // Empty by default.
  let resp = app.clone().oneshot(get("/api/note")).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(body_text(resp).await, "");

  // PUT replaces, 204.
  let resp = app
    .clone()
    .oneshot(put_text("/api/note", "# Title\n\n- a note\n"))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::NO_CONTENT);

  // GET returns the written text.
  let resp = app.oneshot(get("/api/note")).await.unwrap();
  assert_eq!(body_text(resp).await, "# Title\n\n- a note\n");
}

#[tokio::test]
async fn labels_get_and_replace() {
  let (app, state) = app_with_state("labels");

  let resp = app.clone().oneshot(get("/api/labels")).await.unwrap();
  assert_eq!(
    body_json(resp).await,
    serde_json::json!({ "project": [], "context": [] })
  );

  let expected = serde_json::json!({ "project": ["work"], "context": ["phone"] });
  let resp = app
    .clone()
    .oneshot(put_json("/api/labels", expected.clone()))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::NO_CONTENT);
  let persisted: Value =
    serde_json::from_str(&std::fs::read_to_string(&state.config.labels_file).unwrap()).unwrap();
  assert_eq!(persisted, expected);

  let resp = app.oneshot(get("/api/labels")).await.unwrap();
  assert_eq!(body_json(resp).await, expected);
}

#[tokio::test]
async fn events_stream_opens() {
  // The body is an open SSE stream, so we only assert on the response head: a 200 with
  // the text/event-stream content type confirms the route and SSE wiring. The stream is
  // dropped without being consumed.
  let app = app("events");
  let resp = app.oneshot(get("/api/events")).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  let content_type = resp
    .headers()
    .get("content-type")
    .and_then(|v| v.to_str().ok())
    .unwrap_or_default();
  assert!(
    content_type.starts_with("text/event-stream"),
    "unexpected content-type: {content_type}"
  );
}

#[tokio::test]
async fn mutation_broadcasts_event() {
  // The behavior live sync depends on: a successful mutation announces the matching
  // Change on the broadcast channel. Subscribe before mutating so nothing is missed.
  let (app, state) = app_with_state("events-broadcast");
  let mut rx = state.tx.subscribe();

  app
    .clone()
    .oneshot(json_post(
      "/api/tasks",
      serde_json::json!({ "text": "live task" }),
    ))
    .await
    .unwrap();
  assert!(
    matches!(rx.try_recv(), Ok(Change::Tasks)),
    "creating a task should broadcast Change::Tasks"
  );

  app
    .oneshot(put_text("/api/note", "live note"))
    .await
    .unwrap();
  assert!(
    matches!(rx.try_recv(), Ok(Change::Note)),
    "writing the note should broadcast Change::Note"
  );
}
