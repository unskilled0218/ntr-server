//! Server configuration, read from environment variables.

use std::path::PathBuf;

#[derive(Clone, Debug)]
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
    }
  }
}
