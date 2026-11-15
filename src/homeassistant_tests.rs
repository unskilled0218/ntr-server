use super::*;
use crate::api::{self, AppState};
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use futures_util::StreamExt;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use tower::ServiceExt;

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Fixture {
  child: Child,
  config: Config,
  dir: PathBuf,
  client: Client,
}

impl Fixture {
  fn new() -> Self {
    let dir = std::env::temp_dir().join(format!(
      "ntr-ha-{}-{}",
      std::process::id(),
      NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = Command::new("python3")
      .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/fake_ha.py"
      ))
      .arg(&dir)
      .stdout(Stdio::piped())
      .spawn()
      .expect("Python 3 and OpenSSL are required for HA TLS fixtures");
    let mut port = String::new();
    BufReader::new(child.stdout.take().unwrap())
      .read_line(&mut port)
      .unwrap();
    let port: u16 = port.trim().parse().expect("fake HA startup");
    std::fs::write(dir.join("token"), "fixture-secret\n").unwrap();
    let config = Config {
      task_backend: "homeassistant".into(),
      ha_url: format!("https://localhost:{port}"),
      ha_todo_entity: "todo.fixture".into(),
      ha_token_file: dir.join("token"),
      ha_ca_file: Some(dir.join("ca.pem")),
      ha_metadata_file: dir.join("metadata.json"),
      file: dir.join("untouched.txt"),
      note_file: dir.join("Note.md"),
      labels_file: dir.join("labels.json"),
      ..Config::default()
    };
    let client = Client::builder()
      .add_root_certificate(
        reqwest::Certificate::from_pem(&std::fs::read(dir.join("ca.pem")).unwrap()).unwrap(),
      )
      .build()
      .unwrap();
    Self {
      child,
      config,
      dir,
      client,
    }
  }

  fn store(&self) -> (HaStore, broadcast::Receiver<Notification>) {
    let (tx, rx) = broadcast::channel(32);
    (HaStore::new(&self.config, tx).unwrap(), rx)
  }

  async fn set(&self, value: Value) {
    self
      .client
      .post(format!("{}/control", self.config.ha_url))
      .json(&value)
      .send()
      .await
      .unwrap()
      .error_for_status()
      .unwrap();
  }

  async fn state(&self) -> Value {
    self
      .client
      .get(format!("{}/control", self.config.ha_url))
      .send()
      .await
      .unwrap()
      .json()
      .await
      .unwrap()
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
    let _ = std::fs::remove_dir_all(&self.dir);
  }
}

fn item(uid: &str, summary: &str) -> Value {
  json!({"uid": uid, "summary": summary, "status": "needs_action"})
}

#[tokio::test(start_paused = true)]
async fn busy_coordinator_returns_http_error_before_client_fallback_deadline() {
  let f = Fixture::new();
  let (store, _) = f.store();
  let store = std::sync::Arc::new(tokio::sync::Mutex::new(store));
  let backend = crate::task_backend::TaskBackend::HomeAssistant(store.clone());
  let _held = store.lock().await;
  assert!(matches!(
    backend.list(&crate::store::Filter::default()).await,
    Err(AppError::Unavailable(_))
  ));
}

#[tokio::test]
async fn tls_trust_hostname_auth_redirect_and_data_errors_are_safe() {
  let f = Fixture::new();
  let (mut store, _) = f.store();
  assert!(store.list().await.unwrap().is_empty());
  let (tx, _) = broadcast::channel(8);
  let mut config = f.config.clone();
  config.ha_ca_file = None;
  assert!(matches!(
    HaStore::new(&config, tx.clone()).unwrap().list().await,
    Err(AppError::Unavailable(_))
  ));
  config = f.config.clone();
  config.ha_url = config.ha_url.replace("localhost", "127.0.0.1");
  config.ha_metadata_file = f.dir.join("other.json");
  assert!(matches!(
    HaStore::new(&config, tx.clone()).unwrap().list().await,
    Err(AppError::Unavailable(_))
  ));
  std::fs::write(&f.config.ha_token_file, "wrong-secret").unwrap();
  let mut denied = HaStore::new(&f.config, tx).unwrap();
  let error = denied.list().await.unwrap_err();
  assert!(!format!("{error:?}").contains("secret"));
  for mode in ["redirect", "malformed", "unavailable"] {
    f.set(json!({"mode": mode})).await;
    let error = store.list().await.unwrap_err();
    assert!(!format!("{error:?}").contains("fixture-secret"));
    if mode == "redirect" {
      assert_eq!(
        f.state().await["redirect_hits"].as_u64().unwrap_or(0),
        0,
        "HA redirects must not be followed"
      );
    }
  }
  f.set(json!({"mode":"", "entity_state":"unavailable"}))
    .await;
  assert!(matches!(store.list().await, Err(AppError::Unavailable(_))));
}

#[tokio::test]
async fn crud_duplicate_summaries_due_descriptions_and_ids_survive_restart() {
  let f = Fixture::new();
  f.set(json!({"items": [item("external", "same")]})).await;
  let (mut store, _) = f.store();
  let external_id = store.list().await.unwrap()[0].id;
  let created = store
    .mutate(
      Mutation::Create("2026-11-01 same +work @home due:2026-11-16".into()),
      true,
      Some("client".into()),
    )
    .await
    .unwrap()
    .unwrap();
  assert_ne!(created.id, external_id);
  assert_eq!(created.task.derived().tags["due"], "2026-11-16");
  assert!(created.task.derived().projects.contains(&"work".into()));
  let uid = f.state().await["items"]
    .as_array()
    .unwrap()
    .iter()
    .find(|i| i["uid"] != "external")
    .unwrap()["uid"]
    .as_str()
    .unwrap()
    .to_owned();
  let mut items = f.state().await["items"].as_array().unwrap().clone();
  items.iter_mut().find(|i| i["uid"] == uid).unwrap()["description"] =
    json!("Markdown\n**retained**");
  f.set(json!({"items": items})).await;
  store
    .mutate(
      Mutation::Update(
        created.id,
        "2026-11-01 changed +work due:2026-11-16T23:00:00-05:00".into(),
      ),
      true,
      None,
    )
    .await
    .unwrap();
  let state = f.state().await;
  let saved = state["items"]
    .as_array()
    .unwrap()
    .iter()
    .find(|i| i["uid"] == uid)
    .unwrap();
  assert_eq!(saved["description"], "Markdown\n**retained**");
  assert_eq!(saved["due"], "2026-11-16T23:00:00-05:00");
  store
    .mutate(
      Mutation::Update(created.id, "2026-11-01 changed +work".into()),
      true,
      None,
    )
    .await
    .unwrap();
  assert!(f.state().await["items"]
    .as_array()
    .unwrap()
    .iter()
    .find(|i| i["uid"] == uid)
    .unwrap()["due"]
    .is_null());
  let done = store
    .mutate(Mutation::Complete(created.id), true, None)
    .await
    .unwrap()
    .unwrap();
  assert!(done.task.completion_date.is_some());
  assert_eq!(Task::parse(&done.task.to_string()), Some(done.task));
  let (mut restarted, _) = f.store();
  let found = restarted
    .list()
    .await
    .unwrap()
    .into_iter()
    .find(|e| e.id == created.id)
    .unwrap();
  assert!(
    found.task.completed
      && found.task.creation_date.is_none()
      && found.task.completion_date.is_none()
  );
  let reopened = restarted
    .mutate(Mutation::Reopen(created.id), true, None)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(
    reopened.task.creation_date.unwrap().to_string(),
    "2026-11-01"
  );
  restarted
    .mutate(Mutation::Delete(created.id), true, None)
    .await
    .unwrap();
  let replacement = restarted
    .mutate(Mutation::Create("same".into()), false, None)
    .await
    .unwrap()
    .unwrap();
  assert!(replacement.id > created.id);
  assert!(restarted
    .list()
    .await
    .unwrap()
    .iter()
    .any(|e| e.id == external_id));
}

#[tokio::test]
async fn external_completion_projection_preserves_private_creation_through_edit_and_reopen() {
  let f = Fixture::new();
  let (mut store, _) = f.store();
  let created = store
    .mutate(Mutation::Create("2026-11-01 task".into()), true, None)
    .await
    .unwrap()
    .unwrap();
  let mut items = f.state().await["items"].as_array().unwrap().clone();
  items[0]["status"] = json!("completed");
  f.set(json!({"items": items})).await;
  let completed = store.list().await.unwrap().remove(0);
  assert!(completed.task.creation_date.is_none());
  assert!(completed.task.completion_date.is_none());
  assert_eq!(completed.task.to_string(), "x task");
  let edited = store
    .mutate(
      Mutation::Update(created.id, "x changed updated:2026-11-15".into()),
      true,
      None,
    )
    .await
    .unwrap()
    .unwrap();
  assert_eq!(Task::parse(&edited.task.to_string()), Some(edited.task));
  let reopened = store
    .mutate(Mutation::Reopen(created.id), true, None)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(
    reopened.task.creation_date.unwrap().to_string(),
    "2026-11-01"
  );
}

#[tokio::test]
async fn prewrite_and_during_write_external_deltas_bypass_origin_filter() {
  let f = Fixture::new();
  f.set(json!({"items": [item("a", "A"), item("b", "B")]}))
    .await;
  let (mut store, mut rx) = f.store();
  store.poll().await.unwrap();
  rx.try_recv().unwrap();
  let a = store
    .list()
    .await
    .unwrap()
    .into_iter()
    .find(|e| e.task.description == "A")
    .unwrap()
    .id;
  f.set(json!({"items": [item("a", "A"), item("b", "B before")], "on_write": [{"uid": "b", "summary": "B during"}]})).await;
  store
    .mutate(
      Mutation::Update(a, "A own".into()),
      false,
      Some("initiator".into()),
    )
    .await
    .unwrap();
  assert!(
    rx.try_recv().unwrap().origin.is_none(),
    "pre-write external delta is unfiltered"
  );
  assert!(
    rx.try_recv().unwrap().origin.is_none(),
    "readback external delta is unfiltered"
  );
  store.poll().await.unwrap();
  assert!(
    rx.try_recv().is_err(),
    "identical poll does not repeat notifications"
  );
}

#[tokio::test]
async fn initiating_sse_subscriber_receives_concurrent_external_changes() {
  let f = Fixture::new();
  f.set(json!({"items": [item("a", "A"), item("b", "B")]}))
    .await;
  let state = AppState::from_config(f.config.clone()).unwrap();
  let backend = match &state.store {
    crate::task_backend::TaskBackend::HomeAssistant(store) => store.clone(),
    _ => unreachable!(),
  };
  backend.lock().await.poll().await.unwrap();
  let app = api::router(state);
  let events = app
    .clone()
    .oneshot(
      Request::builder()
        .uri("/api/events")
        .header("x-ntr-client-id", "initiator")
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  let mut stream = events.into_body().into_data_stream();
  f.set(json!({"on_write": [{"uid": "b", "summary": "B external"}]}))
    .await;
  let a = backend
    .lock()
    .await
    .list()
    .await
    .unwrap()
    .into_iter()
    .find(|e| e.task.description == "A")
    .unwrap()
    .id;
  let response = app
    .oneshot(
      Request::builder()
        .method("PUT")
        .uri(format!("/api/tasks/{a}"))
        .header("content-type", "application/json")
        .header("x-ntr-client-id", "initiator")
        .body(Body::from(r#"{"text":"A own"}"#))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  let event = tokio::time::timeout(Duration::from_secs(2), stream.next())
    .await
    .expect("external change must not be filtered for the author")
    .unwrap()
    .unwrap();
  assert!(std::str::from_utf8(&event).unwrap().contains("tasks"));
}

#[tokio::test]
async fn poll_waits_for_inflight_mutation_without_consuming_external_delta() {
  let f = Fixture::new();
  f.set(json!({"items": [item("a", "A"), item("b", "B")], "mode":"pause_write", "on_write":[{"uid":"b","summary":"B external"}]})).await;
  let (mut store, mut rx) = f.store();
  store.poll().await.unwrap();
  rx.try_recv().unwrap();
  let a = store
    .list()
    .await
    .unwrap()
    .into_iter()
    .find(|e| e.task.description == "A")
    .unwrap()
    .id;
  let store = std::sync::Arc::new(tokio::sync::Mutex::new(store));
  let writer = store.clone();
  let mutation = tokio::spawn(async move {
    writer
      .lock()
      .await
      .mutate(
        Mutation::Update(a, "A own".into()),
        false,
        Some("initiator".into()),
      )
      .await
  });
  let started: Value = f
    .client
    .get(format!("{}/barrier/wait", f.config.ha_url))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  assert_eq!(started["started"], true);
  let poller = store.clone();
  let poll = tokio::spawn(async move { poller.lock().await.poll().await });
  f.client
    .get(format!("{}/barrier/release", f.config.ha_url))
    .send()
    .await
    .unwrap()
    .error_for_status()
    .unwrap();
  mutation.await.unwrap().unwrap();
  poll.await.unwrap().unwrap();
  assert!(rx.try_recv().unwrap().origin.is_none());
  assert!(
    rx.try_recv().is_err(),
    "queued poll does not duplicate the confirmed snapshot"
  );
}

#[tokio::test]
async fn uncertain_creates_readbacks_and_metadata_failures_never_replay_writes() {
  for mode in ["ambiguous_create", "write_then_fail", "block_metadata"] {
    let f = Fixture::new();
    f.set(json!({"mode": mode})).await;
    let (mut store, mut rx) = f.store();
    assert!(matches!(
      store
        .mutate(Mutation::Create("one".into()), false, Some("self".into()))
        .await,
      Err(AppError::Conflict(_))
    ));
    assert_eq!(f.state().await["writes"].as_array().unwrap().len(), 1);
    assert!(rx.try_recv().unwrap().origin.is_none());
  }
  let f = Fixture::new();
  let (mut store, _) = f.store();
  store.poll().await.unwrap();
  // A directory at the atomic temp-file path causes sidecar writes to fail safely.
  std::fs::create_dir(f.dir.join("metadata.json.tmp")).unwrap();
  assert!(store
    .mutate(
      Mutation::Create("blocked before HA write".into()),
      false,
      None
    )
    .await
    .is_err());
  assert!(f.state().await["writes"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn timeout_after_write_is_uncertain_and_recovery_notifies_without_replay() {
  let f = Fixture::new();
  let (mut store, mut rx) = f.store();
  store.client = Client::builder()
    .add_root_certificate(
      reqwest::Certificate::from_pem(&std::fs::read(f.dir.join("ca.pem")).unwrap()).unwrap(),
    )
    .default_headers({
      let mut h = header::HeaderMap::new();
      h.insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_static("Bearer fixture-secret"),
      );
      h
    })
    .timeout(Duration::from_millis(100))
    .build()
    .unwrap();
  f.set(json!({"mode": "write_then_timeout"})).await;
  assert!(matches!(
    store
      .mutate(
        Mutation::Create("timeout".into()),
        false,
        Some("self".into())
      )
      .await,
    Err(AppError::Timeout)
  ));
  // The control endpoint's lock waits for the committed write to settle; no retry is issued.
  f.set(json!({"mode": ""})).await;
  assert_eq!(f.state().await["writes"].as_array().unwrap().len(), 1);
  store.poll().await.unwrap();
  assert!(rx.try_recv().unwrap().origin.is_none());
  assert_eq!(store.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn capabilities_raw_safety_note_labels_and_filters_use_real_router() {
  let f = Fixture::new();
  std::fs::write(&f.config.file, "do not migrate\n").unwrap();
  let mut directory_task_path = f.config.clone();
  directory_task_path.file = f.dir.clone();
  assert!(
    AppState::from_config(directory_task_path).is_ok(),
    "HA startup must not even read TODO_FILE"
  );
  f.set(json!({"items": [item("a", "work +work @home"), item("b", "personal")]}))
    .await;
  let state = AppState::from_config(f.config.clone()).unwrap();
  let app = api::router(state);
  for (method, path, body, expected) in [
    ("GET", "/api/capabilities", "", StatusCode::OK),
    ("PUT", "/api/raw", "overwrite", StatusCode::BAD_REQUEST),
    ("PUT", "/api/note", "# Markdown", StatusCode::NO_CONTENT),
    ("GET", "/api/note", "", StatusCode::OK),
    (
      "GET",
      "/api/tasks?project=work&context=home",
      "",
      StatusCode::OK,
    ),
    (
      "PUT",
      "/api/labels",
      r#"{"project":["work"],"context":[]}"#,
      StatusCode::NO_CONTENT,
    ),
  ] {
    let response = app
      .clone()
      .oneshot(
        Request::builder()
          .method(method)
          .uri(path)
          .header("content-type", "application/json")
          .body(Body::from(body))
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(response.status(), expected, "{method} {path}");
    let bytes = to_bytes(response.into_body(), MAX_RESPONSE).await.unwrap();
    if path == "/api/capabilities" {
      assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"task_backend":"homeassistant","raw_task_replace":false})
      );
    }
    if path.starts_with("/api/tasks?") {
      assert_eq!(
        serde_json::from_slice::<Value>(&bytes)
          .unwrap()
          .as_array()
          .unwrap()
          .len(),
        1
      );
    }
  }
  assert_eq!(
    std::fs::read_to_string(&f.config.file).unwrap(),
    "do not migrate\n"
  );
  assert!(f.state().await["writes"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn unsupported_features_and_bad_inputs_are_rejected_before_writes() {
  let f = Fixture::new();
  let (mut store, _) = f.store();
  f.set(json!({"features": 7})).await;
  assert!(store
    .mutate(Mutation::Create("task due:2026-11-16".into()), false, None)
    .await
    .is_err());
  f.set(json!({"features": 3})).await;
  assert!(store
    .mutate(Mutation::Create("task".into()), false, None)
    .await
    .is_err());
  f.set(json!({"features": 127})).await;
  for text in [
    "",
    "first\nsecond",
    "task due:bad",
    "task due:2026-11-16 due:2026-11-17",
  ] {
    assert!(store
      .mutate(Mutation::Create(text.into()), false, None)
      .await
      .is_err());
  }
  assert!(f.state().await["writes"].as_array().unwrap().is_empty());
}

#[test]
fn invalid_config_and_corrupt_identity_maps_fail_closed() {
  let f = Fixture::new();
  let (tx, _) = broadcast::channel(8);
  for url in [
    "http://localhost",
    "https://user:password@localhost",
    "https://localhost/?token=secret",
  ] {
    let mut config = f.config.clone();
    config.ha_url = url.into();
    assert!(HaStore::new(&config, tx.clone()).is_err());
  }
  let mut config = f.config.clone();
  config.ha_poll_seconds = 0;
  assert!(HaStore::new(&config, tx.clone()).is_err());
  config = f.config.clone();
  config.ha_todo_entity = "sensor.not_a_list".into();
  assert!(HaStore::new(&config, tx.clone()).is_err());
  std::fs::write(f.dir.join("invalid-ca.pem"), "not a certificate").unwrap();
  config = f.config.clone();
  config.ha_ca_file = Some(f.dir.join("invalid-ca.pem"));
  assert!(HaStore::new(&config, tx.clone()).is_err());
  let mut map = HaStore::new(&f.config, tx.clone()).unwrap().metadata;
  map.next_id = 2;
  let metadata = Metadata {
    id: 1,
    creation: None,
    completion: None,
    legacy: None,
    completed: false,
  };
  map.items.insert("a".into(), metadata.clone());
  map.items.insert("b".into(), metadata);
  std::fs::write(
    &f.config.ha_metadata_file,
    serde_json::to_string(&map).unwrap(),
  )
  .unwrap();
  assert!(
    HaStore::new(&f.config, tx.clone()).is_err(),
    "duplicate persisted IDs must fail closed"
  );
  std::fs::write(&f.config.ha_metadata_file, "{}").unwrap();
  assert!(HaStore::new(&f.config, tx).is_err());
}
