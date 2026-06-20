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
