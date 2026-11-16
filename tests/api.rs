//! Integration tests driving the axum router directly via `oneshot`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures_util::StreamExt;
use my_ntr_server::api::{router, AppState, Change, Notification};
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
async fn legacy_and_list_filters_select_the_same_tasks_without_changing_wire_keys() {
  let app = app("list-compatibility");
  let response = app
    .clone()
    .oneshot(put_text(
      "/api/raw",
      "project planning +my-project\nother task +other\n",
    ))
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::NO_CONTENT);
  for query in ["project=my-project", "list=my-project"] {
    let response = app
      .clone()
      .oneshot(get(&format!("/api/tasks?{query}")))
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let tasks = body_json(response).await;
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_eq!(tasks[0]["raw"], "project planning +my-project");
    assert_eq!(tasks[0]["projects"], serde_json::json!(["my-project"]));
    assert!(tasks[0].get("lists").is_none());
  }
}

#[tokio::test]
async fn rejects_every_priority_query_before_supported_filter_validation() {
  let app = app("removed-filter");
  for query in [
    "priority",
    "priority=",
    "priority=A",
    "priority=invalid",
    "priority=A&priority=B",
    "%70riority=A",
    "completed=bad&priority=",
    "list=work&context=phone&priority=A",
  ] {
    let response = app
      .clone()
      .oneshot(get(&format!("/api/tasks?{query}")))
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{query}");
    assert_eq!(
      body_json(response).await["error"],
      "Priority filtering is no longer supported"
    );
  }
  for query in [
    "completed=false",
    "list=work&context=phone",
    "unknown=value",
  ] {
    let response = app
      .clone()
      .oneshot(get(&format!("/api/tasks?{query}")))
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{query}");
  }
  for query in ["completed=bad", "completed=true&completed=false"] {
    let response = app
      .clone()
      .oneshot(get(&format!("/api/tasks?{query}")))
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{query}");
  }
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
  assert!(created.get("priority").is_none());
  assert!(created["raw"].as_str().unwrap().starts_with("(A) "));
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
  assert!(back.get("priority").is_none());
  assert!(back["raw"].as_str().unwrap().starts_with("(B) "));
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
    matches!(rx.try_recv().map(|event| event.change), Ok(Change::Tasks)),
    "creating a task should broadcast Change::Tasks"
  );

  app
    .oneshot(put_text("/api/note", "live note"))
    .await
    .unwrap();
  assert!(
    matches!(rx.try_recv().map(|event| event.change), Ok(Change::Note)),
    "writing the note should broadcast Change::Note"
  );
}

fn origin(mut request: Request<Body>, id: &str) -> Request<Body> {
  request
    .headers_mut()
    .insert("x-ntr-client-id", id.parse().unwrap());
  request
}

async fn stream_token(
  stream: &mut (impl futures_util::Stream<Item = Result<axum::body::Bytes, axum::Error>> + Unpin),
  kind: &str,
) {
  let bytes = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
    .await
    .unwrap()
    .unwrap()
    .unwrap();
  let text = std::str::from_utf8(&bytes).unwrap();
  assert!(text.contains("event: change"), "{text}");
  assert!(text.contains(&format!("data: {kind}")), "{text}");
}

#[tokio::test]
async fn every_mutation_attaches_origin_and_sse_filters_only_the_author() {
  let (app, state) = app_with_state("origin");
  let author = app
    .clone()
    .oneshot(origin(get("/api/events"), "author"))
    .await
    .unwrap();
  let other = app
    .clone()
    .oneshot(origin(get("/api/events"), "other"))
    .await
    .unwrap();
  let legacy = app.clone().oneshot(get("/api/events")).await.unwrap();
  let mut author = author.into_body().into_data_stream();
  let mut other = other.into_body().into_data_stream();
  let mut legacy = legacy.into_body().into_data_stream();
  let mut rx = state.tx.subscribe();
  let created = app
    .clone()
    .oneshot(origin(
      json_post("/api/tasks", serde_json::json!({"text": "one"})),
      "author",
    ))
    .await
    .unwrap();
  let id = body_json(created).await["id"].as_u64().unwrap();
  let mutations = [
    put_json(
      &format!("/api/tasks/{id}"),
      serde_json::json!({"text": "two"}),
    ),
    json_post(&format!("/api/tasks/{id}/complete"), serde_json::json!({})),
    json_post(
      &format!("/api/tasks/{id}/uncomplete"),
      serde_json::json!({}),
    ),
    Request::builder()
      .method("DELETE")
      .uri(format!("/api/tasks/{id}"))
      .body(Body::empty())
      .unwrap(),
    put_text("/api/raw", "replacement\n"),
    put_text("/api/note", "note"),
  ];
  for request in mutations {
    assert!(app
      .clone()
      .oneshot(origin(request, "author"))
      .await
      .unwrap()
      .status()
      .is_success());
  }
  for kind in ["tasks", "tasks", "tasks", "tasks", "tasks", "tasks", "note"] {
    assert_eq!(rx.try_recv().unwrap().origin.as_deref(), Some("author"));
    stream_token(&mut other, kind).await;
    stream_token(&mut legacy, kind).await;
  }
  // The author consumes all its own queued events but still receives watcher changes.
  state.notify(Change::Note);
  stream_token(&mut author, "note").await;
  stream_token(&mut other, "note").await;
  stream_token(&mut legacy, "note").await;
  assert!(app
    .clone()
    .oneshot(put_text("/api/note", "no id"))
    .await
    .unwrap()
    .status()
    .is_success());
  stream_token(&mut author, "note").await;
  stream_token(&mut other, "note").await;
  stream_token(&mut legacy, "note").await;
}

#[tokio::test]
async fn lag_rescans_both_resources_even_when_all_retained_events_are_self_events() {
  let (app, state) = app_with_state("lag-origin");
  let subscriber = app
    .clone()
    .oneshot(origin(get("/api/events"), "self"))
    .await
    .unwrap();
  let legacy = app.oneshot(get("/api/events")).await.unwrap();
  let mut subscriber = subscriber.into_body().into_data_stream();
  let mut legacy = legacy.into_body().into_data_stream();
  // Receivers exist, but bodies have not been polled: overflow is deterministic.
  for _ in 0..2 {
    state.notify(Change::Tasks);
    for _ in 0..40 {
      state
        .tx
        .send(Notification {
          change: Change::Note,
          origin: Some("self".into()),
        })
        .unwrap();
    }
    stream_token(&mut subscriber, "tasks").await;
    // The second rescan must survive another poll, without another event.
    stream_token(&mut subscriber, "note").await;
    stream_token(&mut legacy, "tasks").await;
    stream_token(&mut legacy, "note").await;
  }
  state.notify(Change::Tasks);
  stream_token(&mut subscriber, "tasks").await;
}
