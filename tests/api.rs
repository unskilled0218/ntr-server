//! Integration tests driving the axum router directly via `oneshot`.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use my_ntr_server::api::{router, AppState};
use my_ntr_server::config::Config;
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
  let path = temp_path(tag);
  let store = Store::load(&path).unwrap();
  let config = Config {
    file: path,
    ..Config::default()
  };
  router(AppState::new(store, config))
}

async fn body_json(resp: axum::response::Response) -> Value {
  let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
    .await
    .unwrap();
  serde_json::from_slice(&bytes).unwrap()
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
