//! Persistent free-form note backed by a single Markdown file.
//!
//! Mirrors [`Store`](crate::store::Store)'s write-then-swap discipline: the file
//! is the canonical store, its text is cached in memory, and every save writes
//! atomically (temp + rename, via [`atomicfile`](crate::atomicfile)) and only
//! swaps the new text into memory once the write succeeds — so the file and
//! memory never diverge. The note has no structure to parse; it is a single
//! block of text.

use crate::atomicfile;
use std::fs;
use std::io;
use std::path::PathBuf;

pub struct NoteStore {
  path: PathBuf,
  content: String,
}

impl NoteStore {
  /// Load the note from `path`. A missing file is treated as an empty note,
  /// exactly as [`Store::load`](crate::store::Store::load) treats a missing
  /// todo.txt.
  pub fn load(path: impl Into<PathBuf>) -> io::Result<NoteStore> {
    let path = path.into();
    let content = match fs::read_to_string(&path) {
      Ok(c) => c,
      Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
      Err(e) => return Err(e),
    };
    Ok(NoteStore { path, content })
  }

  /// The current note text.
  pub fn content(&self) -> &str {
    &self.content
  }

  /// Replace the whole note. Persists atomically, then swaps into memory.
  pub fn replace(&mut self, content: &str) -> io::Result<()> {
    atomicfile::write(&self.path, content)?;
    self.content = content.to_string();
    Ok(())
  }

  /// Pull the file back into memory: the mirror of [`replace`](NoteStore::replace),
  /// which pushes memory to disk. Re-reads `self.path` (a missing file is empty, as in
  /// [`load`](NoteStore::load)) and, since the note has no structure, compares the text
  /// directly. Returns `Ok(false)` and changes nothing when it matches — so the note's
  /// own just-written file is seen as "no change". When it differs (an out-of-band
  /// edit) the new text is swapped in and `Ok(true)` is returned.
  pub fn reload_from_disk(&mut self) -> io::Result<bool> {
    let content = match fs::read_to_string(&self.path) {
      Ok(c) => c,
      Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
      Err(e) => return Err(e),
    };
    if content == self.content {
      return Ok(false);
    }
    self.content = content;
    Ok(true)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn temp_path(tag: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ntr-note-{}-{}.md", tag, std::process::id()));
    let _ = fs::remove_file(&path);
    path
  }

  #[test]
  fn missing_file_is_empty() {
    let note = NoteStore::load(temp_path("missing")).unwrap();
    assert_eq!(note.content(), "");
  }

  #[test]
  fn replace_persists_and_reloads() {
    let path = temp_path("persist");
    {
      let mut note = NoteStore::load(&path).unwrap();
      note.replace("# Title\n\n- a note\n").unwrap();
      assert_eq!(note.content(), "# Title\n\n- a note\n");
    }
    let reloaded = NoteStore::load(&path).unwrap();
    assert_eq!(reloaded.content(), "# Title\n\n- a note\n");
  }

  #[test]
  fn replace_overwrites() {
    let path = temp_path("overwrite");
    let mut note = NoteStore::load(&path).unwrap();
    note.replace("first").unwrap();
    note.replace("second").unwrap();
    assert_eq!(note.content(), "second");
    assert_eq!(NoteStore::load(&path).unwrap().content(), "second");
  }

  #[test]
  fn reload_from_disk_picks_up_external_edits() {
    let path = temp_path("reload");
    let mut note = NoteStore::load(&path).unwrap();
    note.replace("mine\n").unwrap();
    // The note's own write is not a change.
    assert!(!note.reload_from_disk().unwrap());

    // Another program rewrites the file out of band.
    fs::write(&path, "# theirs\n").unwrap();
    assert!(note.reload_from_disk().unwrap());
    assert_eq!(note.content(), "# theirs\n");
    // Re-reading the same content reports no change.
    assert!(!note.reload_from_disk().unwrap());
  }
}
