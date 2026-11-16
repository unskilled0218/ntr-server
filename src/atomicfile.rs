//! Atomic whole-file writes shared by the task and note stores.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Write `data` to `path` atomically: write a sibling temp file, then rename it
/// over the target, so a crash or concurrent reader never sees a partially
/// written file.
pub fn write(path: &Path, data: &str) -> io::Result<()> {
  let mut tmp = path.as_os_str().to_owned();
  tmp.push(".tmp");
  let tmp = PathBuf::from(tmp);

  fs::write(&tmp, data)?;
  fs::rename(&tmp, path)?;
  Ok(())
}

/// Persist an HA sidecar before admitting dependent upstream writes. Atomic
/// replacement alone is not durable: both the file and its directory must sync.
pub(crate) fn write_durable(path: &Path, data: &str) -> io::Result<()> {
  #[cfg(unix)]
  {
    write_durable_with(path, data, |_| Ok(()))
  }
  #[cfg(not(unix))]
  {
    let _ = (path, data);
    Err(io::Error::new(
      io::ErrorKind::Unsupported,
      "HA metadata durability requires Unix file and directory synchronization",
    ))
  }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DurableStage {
  Write,
  SyncFile,
  Rename,
  SyncDirectory,
}

#[cfg(unix)]
fn parent_directory(path: &Path) -> &Path {
  path
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
    .unwrap_or_else(|| Path::new("."))
}

// The callback is a private, per-call fault seam. Production always permits each
// step; tests can fail at a boundary without environment or permission changes.
#[cfg(unix)]
pub(crate) fn write_durable_with(
  path: &Path,
  data: &str,
  mut before: impl FnMut(DurableStage) -> io::Result<()>,
) -> io::Result<()> {
  use std::io::Write;
  let mut tmp = path.as_os_str().to_owned();
  tmp.push(".tmp");
  let tmp = PathBuf::from(tmp);

  before(DurableStage::Write)?;
  let mut file = fs::File::create(&tmp)?;
  file.write_all(data.as_bytes())?;
  before(DurableStage::SyncFile)?;
  file.sync_all()?;
  before(DurableStage::Rename)?;
  fs::rename(&tmp, path)?;
  before(DurableStage::SyncDirectory)?;
  fs::File::open(parent_directory(path))?.sync_all()
}

#[cfg(all(test, unix))]
mod tests {
  use super::*;
  use std::sync::atomic::{AtomicU64, Ordering};

  static NEXT: AtomicU64 = AtomicU64::new(1);

  #[test]
  fn durable_write_orders_syncs_and_preserves_destination_until_rename() {
    let dir = std::env::temp_dir().join(format!(
      "ntr-durable-{}-{}",
      std::process::id(),
      NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("metadata.json");
    let stages = [
      DurableStage::Write,
      DurableStage::SyncFile,
      DurableStage::Rename,
      DurableStage::SyncDirectory,
    ];
    for (index, failed) in stages.iter().enumerate() {
      fs::write(&path, "previous").unwrap();
      let mut observed = Vec::new();
      let result = write_durable_with(&path, "next", |stage| {
        observed.push(stage);
        if stage == *failed {
          Err(io::Error::other("injected persistence failure"))
        } else {
          Ok(())
        }
      });
      assert!(result.is_err(), "{failed:?}");
      assert_eq!(observed, stages[..=index]);
      assert_eq!(
        fs::read_to_string(&path).unwrap(),
        if *failed == DurableStage::SyncDirectory {
          "next"
        } else {
          "previous"
        },
        "only a post-rename failure may expose the new destination"
      );
    }
    let mut observed = Vec::new();
    write_durable_with(&path, "complete", |stage| {
      observed.push(stage);
      Ok(())
    })
    .unwrap();
    assert_eq!(observed, stages);
    assert_eq!(fs::read_to_string(path).unwrap(), "complete");
    fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn relative_sidecar_uses_current_directory_without_changing_process_state() {
    assert_eq!(
      parent_directory(Path::new("ha-metadata.json")),
      Path::new(".")
    );
    assert_eq!(
      parent_directory(Path::new("data/metadata.json")),
      Path::new("data")
    );
  }
}
