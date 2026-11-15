use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
  fn new() -> Self {
    let path = std::env::temp_dir().join(format!(
      "ntr-dotenv-{}-{}",
      std::process::id(),
      NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    Self(path)
  }
  fn file(&self) -> PathBuf {
    self.0.join(".env")
  }
  fn load(&self, env: &[(&str, &str)]) -> Result<Config, ConfigLoadError> {
    Config::from_env_file(
      &self.file(),
      env
        .iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v)))
        .collect(),
    )
  }
}
impl Drop for Fixture {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

#[test]
fn absent_dotenv_preserves_environment_and_defaults() {
  let f = Fixture::new();
  let config = f.load(&[("PORT", "9000")]).unwrap();
  assert_eq!(config.port, 9000);
  assert_eq!(config.task_backend, "file");
  assert_eq!(config.note_file, PathBuf::from("Note.md"));
}

#[test]
fn parses_comments_quotes_and_paths_without_mutating_process_environment() {
  let f = Fixture::new();
  let before = std::env::var_os("NTR_DOTENV_TEST_SENTINEL");
  std::fs::write(f.file(), "# Private token contents stay in another file\nTASK_BACKEND=homeassistant\nHA_URL=\"https://ha.test:8123\" # comment\nHA_TODO_ENTITY=todo.test\nHA_TOKEN_FILE='secrets/token with spaces'\nHA_CA_FILE=./certificates/ca.crt\nHA_METADATA_FILE=./ha-metadata.json\nHA_POLL_SECONDS=12\nNTR_DOTENV_TEST_SENTINEL=not-exported\n").unwrap();
  let config = f.load(&[]).unwrap();
  assert_eq!(config.task_backend, "homeassistant");
  assert_eq!(config.ha_url, "https://ha.test:8123");
  assert_eq!(config.ha_todo_entity, "todo.test");
  assert_eq!(
    config.ha_token_file,
    PathBuf::from("secrets/token with spaces")
  );
  assert_eq!(
    config.ha_ca_file,
    Some(PathBuf::from("./certificates/ca.crt"))
  );
  assert_eq!(config.ha_metadata_file, PathBuf::from("./ha-metadata.json"));
  assert_eq!(config.ha_poll_seconds, 12);
  assert_eq!(std::env::var_os("NTR_DOTENV_TEST_SENTINEL"), before);
}

#[test]
fn process_values_win_even_when_empty_and_file_duplicates_keep_first() {
  let f = Fixture::new();
  std::fs::write(f.file(), "TASK_BACKEND=homeassistant\nHA_URL=https://file.test\nHA_URL=https://duplicate.test\nPORT=9000\n").unwrap();
  let config = f
    .load(&[("TASK_BACKEND", "file"), ("HA_URL", ""), ("PORT", "1234")])
    .unwrap();
  assert_eq!(config.task_backend, "file");
  assert_eq!(config.ha_url, "");
  assert_eq!(config.port, 1234);
  assert_eq!(f.load(&[]).unwrap().ha_url, "https://file.test");
}

#[test]
fn malformed_and_non_regular_files_fail_without_echoing_values() {
  let f = Fixture::new();
  std::fs::write(
    f.file(),
    "TASK_BACKEND=homeassistant\nHA_URL=\"private-sentinel\n",
  )
  .unwrap();
  let error = match f.load(&[("TASK_BACKEND", "file")]) {
    Err(error) => error,
    Ok(_) => panic!("malformed dotenv must fail even with overrides"),
  };
  assert!(!format!("{error} {error:?}").contains("private-sentinel"));
  std::fs::remove_file(f.file()).unwrap();
  std::fs::create_dir(f.file()).unwrap();
  assert!(
    f.load(&[]).is_err(),
    "directory is a deterministic read failure, even as root"
  );
}

#[test]
fn invalid_utf8_file_is_rejected() {
  let f = Fixture::new();
  std::fs::write(f.file(), b"HA_URL=\xff\n").unwrap();
  assert!(f.load(&[]).is_err());
}
