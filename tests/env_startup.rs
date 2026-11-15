//! Real binary startup in private working directories; no global environment mutation or live HA.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
  fn new() -> Self {
    let root = std::env::temp_dir().join(format!(
      "ntr-env-startup-{}-{}",
      std::process::id(),
      NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    Self(root)
  }
  fn start(&self, cwd: &Path, variables: &[(&str, &str)]) -> (String, String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_my-ntr-server"))
      .current_dir(cwd)
      .env_clear()
      .env("HOST", "127.0.0.1")
      .env("PORT", "0")
      .envs(variables.iter().copied())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
      let mut line = String::new();
      let result = BufReader::new(stdout).read_line(&mut line);
      let _ = tx.send((line, result));
    });
    let first = rx.recv_timeout(Duration::from_secs(10));
    let _ = child.kill();
    let output = child.wait_with_output().unwrap();
    reader.join().unwrap();
    let (line, read) = first.expect("binary must either report readiness or exit promptly");
    read.unwrap();
    let ready = line.contains("my-ntr-server listening");
    (line, String::from_utf8(output.stderr).unwrap(), ready)
  }
  fn assert_no_stores(&self) {
    for name in ["todo.txt", "Note.md", "labels.json", "ha-metadata.json"] {
      assert!(
        !self.0.join(name).exists(),
        "unexpected resource write: {name}"
      );
    }
  }
}
impl Drop for Fixture {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

#[test]
fn working_directory_dotenv_selects_ha_without_shell_exports() {
  let f = Fixture::new();
  std::fs::write(f.0.join("token"), "disposable-test-token").unwrap();
  std::fs::write(f.0.join(".env"), "TASK_BACKEND=homeassistant\nHA_URL=https://127.0.0.1:1\nHA_TODO_ENTITY=todo.test\nHA_TOKEN_FILE=./token\n").unwrap();
  let (out, error, ready) = f.start(&f.0, &[]);
  assert!(ready, "startup failed: {error}");
  assert!(out.contains("task backend: homeassistant"));
  assert!(!format!("{out}{error}").contains("disposable-test-token"));
  f.assert_no_stores();
}

#[test]
fn process_backend_overrides_file_and_missing_dotenv_preserves_default() {
  let f = Fixture::new();
  let (out, error, ready) = f.start(&f.0, &[]);
  assert!(ready, "{error}");
  assert!(out.contains("task backend: file"));
  std::fs::write(
    f.0.join(".env"),
    "TASK_BACKEND=homeassistant\nHA_URL=https://file.invalid\n",
  )
  .unwrap();
  let (out, error, ready) = f.start(&f.0, &[("TASK_BACKEND", "file")]);
  assert!(ready, "{error}");
  assert!(out.contains("task backend: file"));
  f.assert_no_stores();
}

#[test]
fn parent_dotenv_is_not_searched() {
  let f = Fixture::new();
  std::fs::write(
    f.0.join(".env"),
    "TASK_BACKEND=parent-value-must-not-load\n",
  )
  .unwrap();
  let child = f.0.join("nested");
  std::fs::create_dir(&child).unwrap();
  let (out, error, ready) = f.start(&child, &[]);
  assert!(ready, "{error}");
  assert!(out.contains("task backend: file"));
}

#[test]
fn malformed_or_unreadable_dotenv_fails_before_any_store_write() {
  let f = Fixture::new();
  std::fs::write(
    f.0.join(".env"),
    "TASK_BACKEND=file\nHA_URL=\"sentinel-private-value\n",
  )
  .unwrap();
  let (out, error, ready) = f.start(&f.0, &[]);
  assert!(!ready);
  assert!(error.contains("Could not load .env"));
  assert!(!format!("{out}{error}").contains("sentinel-private-value"));
  f.assert_no_stores();
  std::fs::remove_file(f.0.join(".env")).unwrap();
  std::fs::create_dir(f.0.join(".env")).unwrap();
  let (_, error, ready) = f.start(&f.0, &[]);
  assert!(!ready);
  assert!(error.contains("Could not load .env"));
  f.assert_no_stores();
}

#[test]
fn private_setup_is_ignored_but_example_is_trackable() {
  let output = Command::new("git")
    .args(["check-ignore", "--no-index", "--stdin"])
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .and_then(|mut child| {
      use std::io::Write;
      child.stdin.take().unwrap().write_all(
        b".env\n.env.local\n.env.example\nsecrets/ha-token\ncertificates/homeassistant.lanCA.crt\n",
      )?;
      child.wait_with_output()
    })
    .unwrap();
  let ignored = String::from_utf8(output.stdout).unwrap();
  assert!(ignored.lines().any(|p| p == ".env"));
  assert!(ignored.lines().any(|p| p == ".env.local"));
  assert!(ignored.contains("secrets/ha-token"));
  assert!(ignored.contains("certificates/homeassistant.lanCA.crt"));
  assert!(!ignored.contains(".env.example"));
  let example =
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".env.example")).unwrap();
  for key in [
    "TASK_BACKEND=",
    "HA_URL=",
    "HA_TOKEN_FILE=",
    "HA_TODO_ENTITY=",
    "HA_CA_FILE=",
    "HA_METADATA_FILE=",
    "HA_POLL_SECONDS=",
  ] {
    assert!(example.contains(key), "missing example key: {key}");
  }
}
