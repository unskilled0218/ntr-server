//! Task operations shared by the file store and the HA adapter.

use crate::api::{AppError, Change, Notification};
use crate::config::Config;
use crate::homeassistant::HaStore;
use crate::store::{Entry, Filter, Store};
use chrono::Local;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tokio::time::timeout;

#[derive(Clone)]
pub enum TaskBackend {
  File(Arc<RwLock<Store>>),
  HomeAssistant(Arc<Mutex<HaStore>>),
}

pub enum Mutation {
  Create(String),
  Update(u64, String),
  Complete(u64),
  Reopen(u64),
  Delete(u64),
}

impl TaskBackend {
  pub fn file(store: Store) -> Self {
    Self::File(Arc::new(RwLock::new(store)))
  }

  pub fn from_config(
    config: &Config,
    tx: broadcast::Sender<Notification>,
  ) -> Result<Self, AppError> {
    match config.task_backend.as_str() {
      "file" => Ok(Self::file(Store::load(&config.file)?)),
      "homeassistant" => Ok(Self::HomeAssistant(Arc::new(Mutex::new(HaStore::new(
        config, tx,
      )?)))),
      _ => Err(AppError::BadRequest(
        "TASK_BACKEND must be file or homeassistant".into(),
      )),
    }
  }

  pub fn is_ha(&self) -> bool {
    matches!(self, Self::HomeAssistant(_))
  }

  pub async fn list(&self, filter: &Filter) -> Result<Vec<Entry>, AppError> {
    let entries = match self {
      Self::File(store) => {
        return Ok(
          store
            .read()
            .map_err(|_| poisoned())?
            .list(filter)
            .into_iter()
            .cloned()
            .collect(),
        )
      }
      Self::HomeAssistant(store) => {
        let mut store = timeout(Duration::from_secs(5), store.lock())
          .await
          .map_err(|_| busy())?;
        match timeout(Duration::from_secs(20), store.list()).await {
          Ok(result) => result?,
          Err(_) => {
            store.interrupted();
            return Err(AppError::Timeout);
          }
        }
      }
    };
    Ok(
      entries
        .into_iter()
        .filter(|e| {
          let d = e.task.derived();
          filter.completed.is_none_or(|c| c == e.task.completed)
            && filter
              .project
              .as_ref()
              .is_none_or(|p| d.projects.contains(p))
            && filter
              .context
              .as_ref()
              .is_none_or(|c| d.contexts.contains(c))
        })
        .collect(),
    )
  }

  pub async fn get(&self, id: u64) -> Result<Entry, AppError> {
    self
      .list(&Filter::default())
      .await?
      .into_iter()
      .find(|e| e.id == id)
      .ok_or(AppError::NotFound)
  }

  pub async fn mutate(
    &self,
    mutation: Mutation,
    config: &Config,
    tx: &broadcast::Sender<Notification>,
    origin: Option<String>,
  ) -> Result<Option<Entry>, AppError> {
    match self {
      Self::HomeAssistant(store) => {
        let mut store = timeout(Duration::from_secs(5), store.lock())
          .await
          .map_err(|_| busy())?;
        match timeout(
          Duration::from_secs(20),
          store.mutate(mutation, config.add_creation_date, origin),
        )
        .await
        {
          Ok(result) => result,
          Err(_) => {
            store.interrupted();
            Err(AppError::Timeout)
          }
        }
      }
      Self::File(store) => {
        let mut store = store.write().map_err(|_| poisoned())?;
        let today = Local::now().date_naive();
        let id = match mutation {
          Mutation::Create(text) => Some(store.add(&text, config.add_creation_date, today)?),
          Mutation::Update(id, text) => {
            store.update(id, &text)?;
            Some(id)
          }
          Mutation::Complete(id) => {
            store.complete(id, today)?;
            Some(id)
          }
          Mutation::Reopen(id) => {
            store.uncomplete(id)?;
            Some(id)
          }
          Mutation::Delete(id) => {
            store.delete(id)?;
            None
          }
        };
        let result = id.and_then(|id| store.get(id).cloned());
        let _ = tx.send(Notification {
          change: Change::Tasks,
          origin,
        });
        Ok(result)
      }
    }
  }

  pub async fn raw(&self) -> Result<String, AppError> {
    let mut text = String::new();
    for entry in self.list(&Filter::default()).await? {
      text.push_str(&entry.task.to_string());
      text.push('\n');
    }
    Ok(text)
  }

  pub fn replace_raw(&self, text: &str) -> Result<(), AppError> {
    match self {
      Self::File(store) => Ok(store.write().map_err(|_| poisoned())?.replace_raw(text)?),
      Self::HomeAssistant(_) => Err(AppError::BadRequest(
        "HA tasks cannot be replaced from a local task file".into(),
      )),
    }
  }
}

fn poisoned() -> AppError {
  AppError::Internal("task store lock poisoned".into())
}

fn busy() -> AppError {
  AppError::Unavailable(
    "HA task coordinator is busy; no new write was issued. Reload before retrying.".into(),
  )
}
