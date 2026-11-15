//! Server-only HA transport and serialized snapshot/metadata coordinator.

use crate::api::{AppError, Change, Notification};
use crate::atomicfile;
use crate::config::Config;
use crate::store::Entry;
use crate::task::Task;
use crate::task_backend::Mutation;
use chrono::{DateTime, Local, NaiveDate};
use reqwest::{header, Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::broadcast;

const MAX_ID: u64 = 9_007_199_254_740_991;
const MAX_RESPONSE: usize = 4 * 1024 * 1024;
// HA TodoListEntityFeature: CREATE=1, DELETE=2, UPDATE=4, DATE=16, DATETIME=32.
const WRITE_FEATURES: u64 = 1 | 2 | 4;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Item {
  uid: String,
  summary: String,
  status: String,
  #[serde(default)]
  due: Option<String>,
  #[serde(default)]
  description: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
  id: u64,
  creation: Option<String>,
  completion: Option<String>,
  legacy: Option<char>,
  completed: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Sidecar {
  schema: u8,
  source: String,
  next_id: u64,
  items: BTreeMap<String, Metadata>,
}

#[derive(Clone, Copy)]
enum WriteTarget {
  Create,
  Update(u64),
  Delete(u64),
}

struct WriteIntent {
  task: Task,
  target: WriteTarget,
}

struct SubmittedWrite {
  intent: WriteIntent,
  uid: Option<String>,
  summary: String,
  due: Option<String>,
}

pub struct HaStore {
  client: Client,
  base: Url,
  entity: String,
  path: PathBuf,
  metadata: Sidecar,
  baseline: Option<BTreeMap<String, Item>>,
  continuity: bool,
  tx: broadcast::Sender<Notification>,
}

fn invalid(message: &str) -> AppError {
  AppError::BadRequest(message.into())
}
fn upstream() -> AppError {
  AppError::Upstream("Home Assistant returned invalid or unsupported task data".into())
}
fn metadata_error() -> AppError {
  AppError::Internal("HA metadata could not be read or saved; check HA_METADATA_FILE".into())
}
fn uncertain() -> AppError {
  AppError::Conflict("HA write outcome is uncertain; reload before making another change. Do not retry automatically.".into())
}
fn date(value: &str) -> bool {
  NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok() && value.len() == 10
}

impl HaStore {
  pub fn new(config: &Config, tx: broadcast::Sender<Notification>) -> Result<Self, AppError> {
    let mut base =
      Url::parse(&config.ha_url).map_err(|_| invalid("HA_URL must be an HTTPS base URL"))?;
    if base.scheme() != "https"
      || base.host_str().is_none()
      || !base.username().is_empty()
      || base.password().is_some()
      || base.query().is_some()
      || base.fragment().is_some()
    {
      return Err(invalid(
        "HA_URL must use HTTPS without credentials, query, or fragment",
      ));
    }
    if !base.path().ends_with('/') {
      base.set_path(&format!("{}/", base.path()));
    }
    let entity = config.ha_todo_entity.clone();
    if !entity.starts_with("todo.")
      || entity.len() <= 5
      || !entity[5..]
        .bytes()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    {
      return Err(invalid(
        "HA_TODO_ENTITY must be an existing todo.* entity ID",
      ));
    }
    if !(1..=3600).contains(&config.ha_poll_seconds) {
      return Err(invalid("HA_POLL_SECONDS must be between 1 and 3600"));
    }
    let token = std::fs::read_to_string(&config.ha_token_file)
      .map_err(|_| invalid("HA_TOKEN_FILE must be a readable token file"))?;
    if token.trim().is_empty() {
      return Err(invalid("HA_TOKEN_FILE is empty"));
    }
    let mut authorization = header::HeaderValue::from_str(&format!("Bearer {}", token.trim()))
      .map_err(|_| invalid("HA_TOKEN_FILE contains an invalid token"))?;
    authorization.set_sensitive(true);
    let mut headers = header::HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
    // A read uses two HA requests; keep their combined deadline below the client's 30s.
    let mut builder = Client::builder()
      .default_headers(headers)
      .redirect(reqwest::redirect::Policy::none())
      .timeout(Duration::from_secs(10));
    if let Some(path) = &config.ha_ca_file {
      let pem = std::fs::read(path)
        .map_err(|_| invalid("HA_CA_FILE must be a readable PEM certificate"))?;
      let certificates = reqwest::Certificate::from_pem_bundle(&pem)
        .map_err(|_| invalid("HA_CA_FILE contains invalid PEM certificates"))?;
      if certificates.is_empty() {
        return Err(invalid("HA_CA_FILE contains no certificates"));
      }
      for certificate in certificates {
        builder = builder.add_root_certificate(certificate);
      }
    }
    let client = builder
      .build()
      .map_err(|_| invalid("Could not initialize HA HTTPS client"))?;
    let source = format!("{}|{}", base, entity);
    let metadata = match std::fs::read_to_string(&config.ha_metadata_file) {
      Ok(text) => serde_json::from_str::<Sidecar>(&text).map_err(|_| metadata_error())?,
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => Sidecar {
        schema: 1,
        source: source.clone(),
        next_id: 1,
        items: BTreeMap::new(),
      },
      Err(_) => return Err(metadata_error()),
    };
    let mut ids = BTreeSet::new();
    if metadata.schema != 1
      || metadata.source != source
      || metadata.next_id == 0
      || metadata.next_id > MAX_ID
      || metadata.items.iter().any(|(uid, m)| {
        uid.is_empty()
          || m.id == 0
          || m.id >= metadata.next_id
          || !ids.insert(m.id)
          || m.creation.as_ref().is_some_and(|d| !date(d))
          || m.completion.as_ref().is_some_and(|d| !date(d))
          || m.legacy.is_some_and(|c| !c.is_ascii_uppercase())
      })
    {
      return Err(metadata_error());
    }
    Ok(Self {
      client,
      base,
      entity,
      path: config.ha_metadata_file.clone(),
      metadata,
      baseline: None,
      continuity: false,
      tx,
    })
  }

  async fn request(&self, path: &str, data: Option<Value>) -> Result<Value, AppError> {
    let url = self.base.join(path).map_err(|_| upstream())?;
    let request = if let Some(data) = data {
      self.client.post(url).json(&data)
    } else {
      self.client.get(url)
    };
    let mut response = request.send().await.map_err(|e| {
      if e.is_timeout() {
        AppError::Timeout
      } else {
        AppError::Unavailable(
          "Home Assistant connection failed; check URL, CA trust, and availability".into(),
        )
      }
    })?;
    if !response.status().is_success() {
      return Err(if response.status().as_u16() == 503 {
        AppError::Unavailable("Home Assistant is unavailable".into())
      } else {
        AppError::Upstream(
          "Home Assistant rejected the request; check token, entity, and supported actions".into(),
        )
      });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| {
      if e.is_timeout() {
        AppError::Timeout
      } else {
        upstream()
      }
    })? {
      if bytes.len() + chunk.len() > MAX_RESPONSE {
        return Err(upstream());
      }
      bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| upstream())
  }

  async fn features(&self) -> Result<u64, AppError> {
    let state = self
      .request(&format!("api/states/{}", self.entity), None)
      .await?;
    if state["state"] == "unavailable" || state["state"] == "unknown" {
      return Err(AppError::Unavailable(
        "HA to-do entity is unavailable".into(),
      ));
    }
    state["attributes"]["supported_features"]
      .as_u64()
      .ok_or_else(upstream)
  }

  async fn fetch(&mut self) -> Result<BTreeMap<String, Item>, AppError> {
    let result = self.fetch_inner().await;
    if result.is_err() {
      self.continuity = false;
    }
    result
  }

  async fn fetch_inner(&self) -> Result<BTreeMap<String, Item>, AppError> {
    self.features().await?;
    let response = self
      .request(
        "api/services/todo/get_items?return_response",
        Some(json!({"entity_id": self.entity, "status": ["needs_action", "completed"]})),
      )
      .await?;
    let items: Vec<Item> =
      serde_json::from_value(response["service_response"][&self.entity]["items"].clone())
        .map_err(|_| upstream())?;
    let mut snapshot = BTreeMap::new();
    for item in items {
      if item.uid.is_empty()
        || item.summary.trim().is_empty()
        || item.summary.contains(['\r', '\n'])
        || !matches!(item.status.as_str(), "needs_action" | "completed")
        || item
          .summary
          .split_whitespace()
          .any(|t| t.starts_with("due:"))
        || item
          .due
          .as_ref()
          .is_some_and(|d| !date(d) && DateTime::parse_from_rfc3339(d).is_err())
      {
        return Err(upstream());
      }
      let probe = Task {
        completed: item.status == "completed",
        legacy_prefix: None,
        completion_date: None,
        creation_date: None,
        description: item.summary.clone(),
      };
      if Task::parse(&probe.to_string()) != Some(probe) {
        return Err(upstream());
      }
      if snapshot.insert(item.uid.clone(), item).is_some() {
        return Err(upstream());
      }
    }
    Ok(snapshot)
  }

  fn notify(&self, origin: Option<String>) {
    let _ = self.tx.send(Notification {
      change: Change::Tasks,
      origin,
    });
  }

  /// A canceled request can have committed upstream; force an unfiltered recovery rescan.
  pub fn interrupted(&mut self) {
    self.continuity = false;
    self.notify(None);
  }

  fn save(&mut self, next: Sidecar) -> Result<(), AppError> {
    let text = serde_json::to_string_pretty(&next).map_err(|_| metadata_error())?;
    atomicfile::write(&self.path, &text).map_err(|_| metadata_error())?;
    self.metadata = next;
    Ok(())
  }

  fn observe(&mut self, snapshot: &BTreeMap<String, Item>) -> Result<Vec<Entry>, AppError> {
    let interrupted = !self.continuity && self.baseline.is_some();
    let mut next = self.metadata.clone();
    for (uid, item) in snapshot {
      if !next.items.contains_key(uid) {
        if next.next_id >= MAX_ID {
          return Err(metadata_error());
        }
        let id = next.next_id;
        next.next_id += 1;
        next.items.insert(
          uid.clone(),
          Metadata {
            id,
            creation: None,
            completion: None,
            legacy: None,
            completed: item.status == "completed",
          },
        );
      }
      let m = next.items.get_mut(uid).expect("metadata inserted");
      let completed = item.status == "completed";
      if !self.continuity || m.completed != completed || !completed {
        m.completion = None;
      }
      m.completed = completed;
    }
    self.save(next)?;
    self.continuity = true;
    if interrupted {
      self.notify(None);
    }
    Ok(self.entries(snapshot))
  }

  fn entries(&self, snapshot: &BTreeMap<String, Item>) -> Vec<Entry> {
    snapshot
      .iter()
      .map(|(uid, item)| {
        let m = &self.metadata.items[uid];
        let completed = item.status == "completed";
        let completion_date = if completed {
          m.completion
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        } else {
          None
        };
        let creation_date = if !completed || completion_date.is_some() {
          m.creation
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        } else {
          None
        };
        let mut description = item.summary.clone();
        if let Some(due) = &item.due {
          description.push_str(&format!(" due:{due}"));
        }
        Entry {
          id: m.id,
          task: Task {
            completed,
            legacy_prefix: m.legacy,
            completion_date,
            creation_date,
            description,
          },
        }
      })
      .collect()
  }

  pub async fn list(&mut self) -> Result<Vec<Entry>, AppError> {
    let snapshot = self.fetch().await?;
    // Reads project fresh metadata but never consume unpublished snapshot changes.
    self.observe(&snapshot)
  }

  pub async fn poll(&mut self) -> Result<(), AppError> {
    let snapshot = self.fetch().await?;
    self.observe(&snapshot)?;
    self.publish(snapshot, None);
    Ok(())
  }

  fn publish(&mut self, snapshot: BTreeMap<String, Item>, origin: Option<String>) {
    if self.baseline.as_ref() != Some(&snapshot) {
      self.notify(origin);
    }
    self.baseline = Some(snapshot);
  }

  async fn service(&self, name: &str, mut data: Value) -> Result<(), AppError> {
    data["entity_id"] = json!(self.entity);
    self
      .request(&format!("api/services/todo/{name}"), Some(data))
      .await?;
    Ok(())
  }

  pub async fn mutate(
    &mut self,
    mutation: Mutation,
    add_creation: bool,
    origin: Option<String>,
  ) -> Result<Option<Entry>, AppError> {
    let before = self.fetch().await?;
    let entries = self.observe(&before)?;
    self.publish(before.clone(), None);
    let features = self.features().await?;
    if features & WRITE_FEATURES != WRITE_FEATURES {
      return Err(invalid(
        "HA list must support creating, updating, and deleting tasks",
      ));
    }
    let today = Local::now().date_naive();
    let intent = match mutation {
      Mutation::Create(text) => {
        let mut task = parse_text(&text)?;
        if add_creation && !task.completed && task.creation_date.is_none() {
          task.creation_date = Some(today);
        }
        WriteIntent {
          task,
          target: WriteTarget::Create,
        }
      }
      other => {
        let deleting = matches!(&other, Mutation::Delete(_));
        let id = match &other {
          Mutation::Update(id, _)
          | Mutation::Complete(id)
          | Mutation::Reopen(id)
          | Mutation::Delete(id) => *id,
          _ => unreachable!(),
        };
        let entry = entries
          .into_iter()
          .find(|e| e.id == id)
          .ok_or(AppError::NotFound)?;
        let task = match other {
          Mutation::Update(_, text) => parse_text(&text)?,
          Mutation::Complete(_) => {
            let mut t = entry.task;
            t.complete(today);
            t
          }
          Mutation::Reopen(_) => {
            let mut t = entry.task;
            t.uncomplete();
            t
          }
          Mutation::Delete(_) => entry.task,
          _ => unreachable!(),
        };
        WriteIntent {
          task,
          target: if deleting {
            WriteTarget::Delete(id)
          } else {
            WriteTarget::Update(id)
          },
        }
      }
    };
    self.write_task(before, intent, features, origin).await
  }

  async fn write_task(
    &mut self,
    before: BTreeMap<String, Item>,
    intent: WriteIntent,
    features: u64,
    origin: Option<String>,
  ) -> Result<Option<Entry>, AppError> {
    let task = &intent.task;
    let creating = matches!(intent.target, WriteTarget::Create);
    let deleting = matches!(intent.target, WriteTarget::Delete(_));
    let id = match intent.target {
      WriteTarget::Create => None,
      WriteTarget::Update(id) | WriteTarget::Delete(id) => Some(id),
    };
    let uid = id.and_then(|id| {
      self
        .metadata
        .items
        .iter()
        .find(|(_, m)| m.id == id)
        .map(|(uid, _)| uid.clone())
    });
    let (summary, due) = summary_due(task)?;
    let probe = Task {
      completed: false,
      legacy_prefix: None,
      completion_date: None,
      creation_date: None,
      description: summary.clone(),
    };
    if !deleting && Task::parse(&probe.to_string()) != Some(probe) {
      return Err(invalid(
        "HA summary cannot start with todo.txt lifecycle or legacy-prefix syntax",
      ));
    }
    let old = uid.as_ref().and_then(|u| before.get(u));
    let due_changed = old.and_then(|i| i.due.as_ref()) != due.as_ref();
    if !deleting && due_changed {
      let needed = if due.as_ref().is_some_and(|d| !date(d))
        || (due.is_none() && old.and_then(|i| i.due.as_ref()).is_some_and(|d| !date(d)))
      {
        32
      } else {
        16
      };
      if features & needed == 0 {
        return Err(invalid("HA list does not support this due value"));
      }
    }
    let status = if task.completed {
      "completed"
    } else {
      "needs_action"
    };
    let mut data = json!({});
    let service = if deleting {
      "remove_item"
    } else if creating {
      "add_item"
    } else {
      "update_item"
    };
    if creating {
      data["item"] = json!(summary);
    } else {
      data["item"] = json!(uid.as_ref().ok_or(AppError::NotFound)?);
    }
    if !creating && !deleting {
      if old.is_some_and(|i| i.summary != summary) {
        data["rename"] = json!(summary);
      }
      if old.is_some_and(|i| i.status != status) {
        data["status"] = json!(status);
      }
    }
    if !deleting && due_changed {
      if let Some(due) = &due {
        data[if date(due) {
          "due_date"
        } else {
          "due_datetime"
        }] = json!(due);
      } else {
        data[if old.and_then(|i| i.due.as_ref()).is_some_and(|d| !date(d)) {
          "due_datetime"
        } else {
          "due_date"
        }] = Value::Null;
      }
    }
    if let Some(uid) = &uid {
      // HA resolves item by UID OR summary in list order. Refuse an ambiguous address.
      if before.values().any(|i| &i.uid != uid && &i.summary == uid) {
        return Err(invalid(
          "HA item UID collides with another summary; rename that item in HA first",
        ));
      }
    }
    let needs_write = creating || deleting || data.as_object().is_some_and(|d| d.len() > 1);
    if let Err(error) = if needs_write {
      self.service(service, data).await
    } else {
      Ok(())
    } {
      self.continuity = false;
      self.notify(None);
      return Err(match error {
        AppError::Timeout => AppError::Timeout,
        _ => uncertain(),
      });
    }
    let result = self
      .finish_write(
        before,
        SubmittedWrite {
          intent,
          uid,
          summary,
          due,
        },
        origin,
      )
      .await;
    if result.is_err() {
      self.continuity = false;
      self.notify(None);
    }
    result.map_err(|_| uncertain())
  }

  async fn finish_write(
    &mut self,
    before: BTreeMap<String, Item>,
    submitted: SubmittedWrite,
    origin: Option<String>,
  ) -> Result<Option<Entry>, AppError> {
    let SubmittedWrite {
      intent,
      uid,
      summary,
      due,
    } = submitted;
    let task = intent.task;
    let creating = matches!(intent.target, WriteTarget::Create);
    let deleting = matches!(intent.target, WriteTarget::Delete(_));
    let mut after = self.fetch().await?;
    let uid = if creating {
      let candidates: Vec<_> = after
        .values()
        .filter(|i| {
          !before.contains_key(&i.uid)
            && i.summary == summary
            && same_due(&i.due, &due)
            && i.status == "needs_action"
        })
        .map(|i| i.uid.clone())
        .collect();
      if candidates.len() != 1 {
        return Err(uncertain());
      }
      candidates[0].clone()
    } else {
      uid.ok_or(AppError::NotFound)?
    };
    if creating && task.completed {
      if after.values().any(|i| i.uid != uid && i.summary == uid) {
        return Err(uncertain());
      }
      self
        .service("update_item", json!({"item": uid, "status": "completed"}))
        .await?;
      after = self.fetch().await?;
    }
    let mut expected = before;
    if deleting {
      expected.remove(&uid);
      if after.contains_key(&uid) {
        return Err(uncertain());
      }
    } else {
      let description = expected.get(&uid).and_then(|i| i.description.clone());
      expected.insert(
        uid.clone(),
        Item {
          uid: uid.clone(),
          summary,
          due,
          description,
          status: if task.completed {
            "completed".into()
          } else {
            "needs_action".into()
          },
        },
      );
    }
    self.observe(&after)?;
    if !deleting {
      let observed = after.get(&uid).ok_or_else(uncertain)?;
      let intended = &expected[&uid];
      if observed.summary != intended.summary
        || observed.status != intended.status
        || !same_due(&observed.due, &intended.due)
      {
        self.publish(after, None);
        return Err(uncertain());
      }
      let mut next = self.metadata.clone();
      let m = next.items.get_mut(&uid).ok_or_else(metadata_error)?;
      if task.creation_date.is_some() {
        m.creation = task.creation_date.map(|d| d.to_string());
      }
      m.completion = task.completion_date.map(|d| d.to_string());
      m.legacy = task.legacy_prefix;
      self.save(next)?;
    }
    let own = if snapshots_match(&after, &expected) {
      origin
    } else {
      None
    };
    if self.baseline.as_ref() == Some(&after) {
      self.notify(own.clone());
    }
    self.publish(after.clone(), own);
    Ok(if deleting {
      None
    } else {
      self
        .entries(&after)
        .into_iter()
        .find(|e| e.id == self.metadata.items[&uid].id)
    })
  }
}

fn same_due(a: &Option<String>, b: &Option<String>) -> bool {
  if a == b {
    return true;
  }
  match (a, b) {
    (Some(a), Some(b)) => match (
      DateTime::parse_from_rfc3339(a),
      DateTime::parse_from_rfc3339(b),
    ) {
      (Ok(a), Ok(b)) => a == b,
      _ => false,
    },
    _ => false,
  }
}

fn snapshots_match(a: &BTreeMap<String, Item>, b: &BTreeMap<String, Item>) -> bool {
  a.len() == b.len()
    && a.iter().all(|(uid, i)| {
      b.get(uid).is_some_and(|j| {
        i.summary == j.summary
          && i.status == j.status
          && i.description == j.description
          && same_due(&i.due, &j.due)
      })
    })
}

fn parse_text(text: &str) -> Result<Task, AppError> {
  if text.contains(['\r', '\n']) {
    return Err(invalid("Task text must be a single line"));
  }
  Task::parse(text).ok_or_else(|| invalid("Task text is empty"))
}

fn summary_due(task: &Task) -> Result<(String, Option<String>), AppError> {
  let mut due = None;
  let mut summary = task.description.clone();
  for token in task.description.split_whitespace() {
    if let Some(value) = token.strip_prefix("due:") {
      if due.is_some() || (!date(value) && DateTime::parse_from_rfc3339(value).is_err()) {
        return Err(invalid("Use one valid due date or offset-bearing datetime"));
      }
      due = Some(value.to_string());
      let (start, _) = task
        .description
        .match_indices(token)
        .find(|(start, _)| *start == 0 || task.description[..*start].ends_with(char::is_whitespace))
        .ok_or_else(|| invalid("Invalid due token"))?;
      let end = start + token.len();
      let start = if start > 0 && task.description.as_bytes()[start - 1] == b' ' {
        start - 1
      } else {
        start
      };
      summary.replace_range(start..end, "");
    }
  }
  if summary.trim().is_empty() {
    return Err(invalid("Task summary is empty"));
  }
  Ok((summary.trim().to_string(), due))
}

#[cfg(test)]
#[path = "homeassistant_tests.rs"]
mod tests;
