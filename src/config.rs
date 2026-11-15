//! Server configuration, read from environment variables.

use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
  /// Path to the todo.txt file (env `TODO_FILE`, default `todo.txt`).
  pub file: PathBuf,
  /// Path to the Markdown note file (env `NOTE_FILE`, default `Note.md`).
  pub note_file: PathBuf,
  /// Path to the remembered labels file (env `LABELS_FILE`, default `labels.json`).
  pub labels_file: PathBuf,
  /// Bind host (env `HOST`, default `127.0.0.1`).
  pub host: String,
  /// Bind port (env `PORT`, default `8080`).
  pub port: u16,
  /// Stamp a creation date on new incomplete tasks that lack one
  /// (env `TODO_ADD_CREATION_DATE`, default `true`).
  pub add_creation_date: bool,
  /// Selected task backend: `file` or `homeassistant`.
  pub task_backend: String,
  pub ha_url: String,
  pub ha_token_file: PathBuf,
  pub ha_todo_entity: String,
  pub ha_ca_file: Option<PathBuf>,
  pub ha_metadata_file: PathBuf,
  pub ha_poll_seconds: u64,
}

impl Default for Config {
  fn default() -> Self {
    Config {
      file: PathBuf::from("todo.txt"),
      note_file: PathBuf::from("Note.md"),
      labels_file: PathBuf::from("labels.json"),
      host: "127.0.0.1".to_string(),
      port: 8080,
      add_creation_date: true,
      task_backend: "file".into(),
      ha_url: String::new(),
      ha_token_file: PathBuf::new(),
      ha_todo_entity: String::new(),
      ha_ca_file: None,
      ha_metadata_file: PathBuf::from("ha-metadata.json"),
      ha_poll_seconds: 10,
    }
  }
}

impl Config {
  /// Build configuration from environment variables, falling back to defaults.
  pub fn from_env() -> Self {
    let defaults = Config::default();
    Config {
      file: std::env::var_os("TODO_FILE")
        .map(PathBuf::from)
        .unwrap_or(defaults.file),
      note_file: std::env::var_os("NOTE_FILE")
        .map(PathBuf::from)
        .unwrap_or(defaults.note_file),
      labels_file: std::env::var_os("LABELS_FILE")
        .map(PathBuf::from)
        .unwrap_or(defaults.labels_file),
      host: std::env::var("HOST").unwrap_or(defaults.host),
      port: std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(defaults.port),
      add_creation_date: std::env::var("TODO_ADD_CREATION_DATE")
        .ok()
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(defaults.add_creation_date),
      task_backend: std::env::var("TASK_BACKEND").unwrap_or(defaults.task_backend),
      ha_url: std::env::var("HA_URL").unwrap_or_default(),
      ha_token_file: std::env::var_os("HA_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_default(),
      ha_todo_entity: std::env::var("HA_TODO_ENTITY").unwrap_or_default(),
      ha_ca_file: std::env::var_os("HA_CA_FILE").map(PathBuf::from),
      ha_metadata_file: std::env::var_os("HA_METADATA_FILE")
        .map(PathBuf::from)
        .unwrap_or(defaults.ha_metadata_file),
      ha_poll_seconds: std::env::var("HA_POLL_SECONDS")
        .ok()
        .map(|s| s.parse().unwrap_or(0))
        .unwrap_or(defaults.ha_poll_seconds),
    }
  }
}
