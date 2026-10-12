use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use my_ntr_server::api::{router, AppState};
use my_ntr_server::config::Config;
use my_ntr_server::labelstore::LabelStore;
use my_ntr_server::note::NoteStore;
use my_ntr_server::store::Store;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
  dir: PathBuf,
  config: Config,
}

impl Fixture {
  fn new() -> Self {
    let dir = std::env::temp_dir().join(format!(
      "ntr-date-api-{}-{}",
      std::process::id(),
      NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).unwrap();
    let config = Config {
      file: dir.join("todo.txt"),
      note_file: dir.join("Note.md"),
      labels_file: dir.join("labels.json"),
      add_creation_date: false,
      ..Config::default()
    };
    Self { dir, config }
  }

  fn app(&self) -> Router {
    router(AppState::new(
      Store::load(&self.config.file).unwrap(),
      NoteStore::load(&self.config.note_file).unwrap(),
      LabelStore::load(&self.config.labels_file).unwrap(),
      self.config.clone(),
    ))
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.dir);
  }
}

async fn request(
  app: &Router,
  method: &str,
  path: &str,
  body: Option<String>,
) -> (StatusCode, String) {
  let response = app
    .clone()
    .oneshot(
      Request::builder()
        .method(method)
        .uri(path)
        .header(
          "content-type",
          if path == "/api/raw" {
            "text/plain"
          } else {
            "application/json"
          },
        )
        .body(body.map_or_else(Body::empty, Body::from))
        .unwrap(),
    )
    .await
    .unwrap();
  let status = response.status();
  let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
  (status, String::from_utf8(bytes.to_vec()).unwrap())
}

async fn write_task(app: &Router, method: &str, path: &str, text: &str) -> Value {
  let (status, body) = request(app, method, path, Some(json!({ "text": text }).to_string())).await;
  assert!(status.is_success(), "{status}: {body}");
  serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn edit_dates_roundtrip_through_http_raw_storage_and_restart() {
  let fixture = Fixture::new();
  let app = fixture.app();
  let initial = write_task(
    &app,
    "POST",
    "/api/tasks",
    "(A) 2026-10-10 Do something +cart @phone due:2026-10-12 custom:old",
  )
  .await;
  let path = format!("/api/tasks/{}", initial["id"]);
  for stamp in ["2026-10-11", "2026-10-12"] {
    let raw = format!("(A) 2026-10-10 Do something +cart @phone due:2026-10-12 custom:old custom:new updated:{stamp}");
    let saved = write_task(&app, "PUT", &path, &raw).await;
    assert_eq!(saved["creation_date"], "2026-10-10");
    assert_eq!(saved["tags"]["updated"], stamp);
    assert_eq!(saved["tags"]["custom"], "new");
    assert_eq!(saved["tags"]["due"], "2026-10-12");
    assert_eq!(saved["projects"], json!(["cart"]));
    assert_eq!(saved["contexts"], json!(["phone"]));
    let (_, stored) = request(&app, "GET", "/api/raw", None).await;
    assert_eq!(stored, raw + "\n");
    assert_eq!(fs::read_to_string(&fixture.config.file).unwrap(), stored);
    let (_, restarted) = request(&fixture.app(), "GET", "/api/tasks", None).await;
    let restarted: Value = serde_json::from_str(&restarted).unwrap();
    assert_eq!(restarted[0]["creation_date"], "2026-10-10");
    assert_eq!(restarted[0]["tags"], saved["tags"]);
  }
  let (status, body) = request(&app, "POST", &format!("{path}/complete"), None).await;
  assert_eq!(status, StatusCode::OK);
  let completed: Value = serde_json::from_str(&body).unwrap();
  assert_eq!(completed["creation_date"], "2026-10-10");
  assert!(completed["completion_date"].is_string());
  assert_eq!(completed["tags"]["updated"], "2026-10-12");
  let (status, body) = request(&app, "POST", &format!("{path}/uncomplete"), None).await;
  assert_eq!(status, StatusCode::OK);
  let reopened: Value = serde_json::from_str(&body).unwrap();
  assert_eq!(reopened["priority"], "A");
  assert_eq!(reopened["creation_date"], "2026-10-10");
  assert_eq!(reopened["tags"]["updated"], "2026-10-12");
  let (_, raw) = request(&app, "GET", "/api/raw", None).await;
  let (status, _) = request(&app, "PUT", "/api/raw", Some(raw.clone())).await;
  assert!(status.is_success());
  let (_, restarted) = request(&fixture.app(), "GET", "/api/tasks", None).await;
  let restarted: Value = serde_json::from_str(&restarted).unwrap();
  assert_eq!(restarted[0]["raw"], raw.trim());
  assert_eq!(restarted[0]["tags"]["updated"], "2026-10-12");
}

#[tokio::test]
async fn completed_and_legacy_dates_are_preserved_without_inventing_history() {
  let fixture = Fixture::new();
  let app = fixture.app();
  for initial in [
    "x 2026-10-09 2026-10-01 finished +cart",
    "x legacy +cart",
    "legacy +cart",
  ] {
    let created = write_task(&app, "POST", "/api/tasks", initial).await;
    let raw = format!("{initial} updated:2026-10-11");
    let path = format!("/api/tasks/{}", created["id"]);
    let edited = write_task(&app, "PUT", &path, &raw).await;
    assert_eq!(edited["creation_date"], created["creation_date"]);
    assert_eq!(edited["completion_date"], created["completion_date"]);
    assert_eq!(edited["tags"]["updated"], "2026-10-11");
    assert_eq!(edited["raw"], raw);
  }
  let (_, restarted) = request(&fixture.app(), "GET", "/api/tasks", None).await;
  let restarted: Value = serde_json::from_str(&restarted).unwrap();
  assert_eq!(restarted[0]["creation_date"], "2026-10-01");
  assert!(restarted[1]["creation_date"].is_null());
  assert!(restarted[2]["creation_date"].is_null());
}

#[tokio::test]
async fn rejected_writes_leave_cached_and_persisted_dates_unchanged() {
  let fixture = Fixture::new();
  let app = fixture.app();
  let initial = write_task(
    &app,
    "POST",
    "/api/tasks",
    "2026-10-10 original updated:2026-10-11",
  )
  .await;
  let path = format!("/api/tasks/{}", initial["id"]);
  let before = fs::read_to_string(&fixture.config.file).unwrap();
  let (status, _) = request(&app, "PUT", &path, Some(json!({ "text": "" }).to_string())).await;
  assert_eq!(status, StatusCode::BAD_REQUEST);
  // Occupy the atomic writer's sibling temp path with a directory to force a real I/O failure.
  fs::create_dir(fixture.dir.join("todo.txt.tmp")).unwrap();
  let (status, _) = request(
    &app,
    "PUT",
    &path,
    Some(json!({ "text": "2026-10-10 edited updated:2026-10-12" }).to_string()),
  )
  .await;
  assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
  let (_, current) = request(&app, "GET", &path, None).await;
  assert_eq!(serde_json::from_str::<Value>(&current).unwrap(), initial);
  assert_eq!(fs::read_to_string(&fixture.config.file).unwrap(), before);
  let (_, restarted) = request(&fixture.app(), "GET", "/api/tasks", None).await;
  let restarted: Value = serde_json::from_str(&restarted).unwrap();
  assert_eq!(restarted[0]["creation_date"], "2026-10-10");
  assert_eq!(restarted[0]["tags"]["updated"], "2026-10-11");
}
