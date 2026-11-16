//! Persistent task store backed by a single todo.txt file.
//!
//! Tasks are assigned process-stable ids at load time (from a monotonic
//! counter) and ids are never reused while the process runs. The file stays a
//! pure todo.txt document; ids live only in memory as an index over it. Every
//! mutation writes the whole file atomically (temp file + rename) and only
//! swaps in the new in-memory state after the write succeeds, so the file and
//! memory never diverge.

use crate::atomicfile;
use crate::task::Task;
use chrono::NaiveDate;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A task together with its process-stable id.
#[derive(Debug, Clone)]
pub struct Entry {
  pub id: u64,
  pub list_id: Option<String>,
  pub task: Task,
}

/// Optional filters for [`Store::list`].
#[derive(Debug, Default)]
pub struct Filter {
  pub completed: Option<bool>,
  pub list: Option<String>,
  pub context: Option<String>,
}

/// Errors a store operation can produce.
#[derive(Debug)]
pub enum StoreError {
  NotFound,
  Invalid(String),
  Io(io::Error),
}

impl From<io::Error> for StoreError {
  fn from(e: io::Error) -> Self {
    StoreError::Io(e)
  }
}

pub struct Store {
  path: PathBuf,
  entries: Vec<Entry>,
  next_id: u64,
}

impl Store {
  /// Load a store from `path`. A missing file is treated as an empty store.
  pub fn load(path: impl Into<PathBuf>) -> io::Result<Store> {
    let path = path.into();
    let content = match fs::read_to_string(&path) {
      Ok(c) => c,
      Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
      Err(e) => return Err(e),
    };

    let (entries, next_id) = parse_entries(&content, 1);
    Ok(Store {
      path,
      entries,
      next_id,
    })
  }

  /// List tasks in file order, applying any provided filters.
  pub fn list(&self, filter: &Filter) -> Vec<&Entry> {
    self
      .entries
      .iter()
      .filter(|e| {
        if let Some(c) = filter.completed {
          if e.task.completed != c {
            return false;
          }
        }
        if filter.list.is_some() || filter.context.is_some() {
          let d = e.task.derived();
          if let Some(pr) = &filter.list {
            if !d.lists.iter().any(|x| x == pr) {
              return false;
            }
          }
          if let Some(cx) = &filter.context {
            if !d.contexts.iter().any(|x| x == cx) {
              return false;
            }
          }
        }
        true
      })
      .collect()
  }

  /// Fetch a single task by id.
  pub fn get(&self, id: u64) -> Option<&Entry> {
    self.entries.iter().find(|e| e.id == id)
  }

  /// Add a new task from a raw todo.txt line. When `add_creation_date` is set
  /// and the line is incomplete with no creation date, `today` is stamped.
  pub fn add(
    &mut self,
    text: &str,
    add_creation_date: bool,
    today: NaiveDate,
  ) -> Result<u64, StoreError> {
    let mut task =
      Task::parse(text).ok_or_else(|| StoreError::Invalid("task text is empty".to_string()))?;
    if add_creation_date && !task.completed && task.creation_date.is_none() {
      task.creation_date = Some(today);
    }

    let id = self.next_id;
    let mut entries = self.entries.clone();
    entries.push(Entry {
      id,
      list_id: None,
      task,
    });
    self.save(entries, self.next_id + 1)?;
    Ok(id)
  }

  /// Replace a task's content from a raw todo.txt line.
  pub fn update(&mut self, id: u64, text: &str) -> Result<(), StoreError> {
    let task =
      Task::parse(text).ok_or_else(|| StoreError::Invalid("task text is empty".to_string()))?;
    self.mutate(id, move |t| *t = task)
  }

  /// Mark a task complete.
  pub fn complete(&mut self, id: u64, today: NaiveDate) -> Result<(), StoreError> {
    self.mutate(id, |t| t.complete(today))
  }

  /// Mark a task incomplete.
  pub fn uncomplete(&mut self, id: u64) -> Result<(), StoreError> {
    self.mutate(id, |t| t.uncomplete())
  }

  /// Apply `f` to the task with `id` under the write-then-swap discipline:
  /// mutate a clone, persist it, and only then swap it into memory.
  fn mutate(&mut self, id: u64, f: impl FnOnce(&mut Task)) -> Result<(), StoreError> {
    let mut entries = self.entries.clone();
    let entry = entries
      .iter_mut()
      .find(|e| e.id == id)
      .ok_or(StoreError::NotFound)?;
    f(&mut entry.task);
    self.save(entries, self.next_id)
  }

  /// Delete a task by id. Other ids are unaffected.
  pub fn delete(&mut self, id: u64) -> Result<(), StoreError> {
    if !self.entries.iter().any(|e| e.id == id) {
      return Err(StoreError::NotFound);
    }
    let entries: Vec<Entry> = self
      .entries
      .iter()
      .filter(|e| e.id != id)
      .cloned()
      .collect();
    self.save(entries, self.next_id)
  }

  /// The whole store rendered as todo.txt text.
  pub fn raw(&self) -> String {
    render(&self.entries)
  }

  /// Replace the entire file from raw todo.txt text. All ids are reassigned
  /// from the running counter (old ids become invalid); this is the one bulk
  /// operation that rotates ids.
  pub fn replace_raw(&mut self, text: &str) -> Result<(), StoreError> {
    let (entries, next_id) = parse_entries(text, self.next_id);
    self.save(entries, next_id)
  }

  /// Pull the file back into memory: the mirror of [`save`](Store::save), which
  /// pushes memory to disk. Re-reads `self.path` (a missing file is empty, as in
  /// [`load`](Store::load)) and compares the canonical render of what is on disk
  /// against the current render. Returns `Ok(false)` and changes nothing when they
  /// match — so the store's own just-written file (and any whitespace-only edit the
  /// parser discards) is correctly seen as "no change". When they differ this is an
  /// out-of-band edit: the new entries are swapped in (ids rotate, as in
  /// [`replace_raw`](Store::replace_raw)) and `Ok(true)` is returned.
  pub fn reload_from_disk(&mut self) -> io::Result<bool> {
    let content = match fs::read_to_string(&self.path) {
      Ok(c) => c,
      Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
      Err(e) => return Err(e),
    };
    let (entries, next_id) = parse_entries(&content, self.next_id);
    if render(&entries) == render(&self.entries) {
      return Ok(false);
    }
    self.entries = entries;
    self.next_id = next_id;
    Ok(true)
  }

  /// Persist `entries` to disk, then swap them into memory on success.
  fn save(&mut self, entries: Vec<Entry>, next_id: u64) -> Result<(), StoreError> {
    write_file(&self.path, &entries)?;
    self.entries = entries;
    self.next_id = next_id;
    Ok(())
  }
}

/// Parse todo.txt text into entries, assigning ids from `start_id` upward (blank
/// and unparseable lines are skipped). Returns the entries and the next free id.
/// The single home for "how a todo.txt becomes ids+entries", shared by
/// [`load`](Store::load), [`replace_raw`](Store::replace_raw), and
/// [`reload_from_disk`](Store::reload_from_disk).
fn parse_entries(content: &str, start_id: u64) -> (Vec<Entry>, u64) {
  let mut entries = Vec::new();
  let mut next_id = start_id;
  for line in content.lines() {
    if let Some(task) = Task::parse(line) {
      entries.push(Entry {
        id: next_id,
        list_id: None,
        task,
      });
      next_id += 1;
    }
  }
  (entries, next_id)
}

/// Render entries as todo.txt text (one task per line, trailing newline each).
fn render(entries: &[Entry]) -> String {
  let mut out = String::new();
  for e in entries {
    out.push_str(&e.task.to_string());
    out.push('\n');
  }
  out
}

/// Render entries and write them durably, so a crash never leaves a partially
/// written todo.txt. The atomic temp-file + rename lives in [`atomicfile`], shared
/// with the note store.
fn write_file(path: &Path, entries: &[Entry]) -> io::Result<()> {
  atomicfile::write(path, &render(entries))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 19).unwrap()
  }

  fn temp_store(tag: &str) -> Store {
    let mut path = std::env::temp_dir();
    path.push(format!("ntr-store-{}-{}.txt", tag, std::process::id()));
    let _ = fs::remove_file(&path);
    Store::load(&path).unwrap()
  }

  #[test]
  fn add_and_list() {
    let mut s = temp_store("add");
    let id = s.add("buy milk", true, today()).unwrap();
    assert_eq!(id, 1);
    let all = s.list(&Filter::default());
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].task.creation_date, Some(today()));
  }

  #[test]
  fn filter_by_list_and_completion() {
    let mut s = temp_store("filter");
    s.add("a +home", true, today()).unwrap();
    s.add("b +work", true, today()).unwrap();
    let id = s.add("c +home", true, today()).unwrap();
    s.complete(id, today()).unwrap();

    let home = Filter {
      list: Some("home".to_string()),
      ..Filter::default()
    };
    assert_eq!(s.list(&home).len(), 2);

    let active_home = Filter {
      list: Some("home".to_string()),
      completed: Some(false),
      ..Filter::default()
    };
    assert_eq!(s.list(&active_home).len(), 1);
  }

  #[test]
  fn delete_keeps_other_ids_stable() {
    let mut s = temp_store("delete");
    let a = s.add("a", true, today()).unwrap();
    let b = s.add("b", true, today()).unwrap();
    let c = s.add("c", true, today()).unwrap();
    s.delete(b).unwrap();
    assert!(s.get(b).is_none());
    // a and c keep their original ids; no renumbering.
    assert!(s.get(a).is_some());
    assert!(s.get(c).is_some());
    assert_eq!(s.list(&Filter::default()).len(), 2);
  }

  #[test]
  fn persists_and_reloads_identically() {
    let mut path = std::env::temp_dir();
    path.push(format!("ntr-store-persist-{}.txt", std::process::id()));
    let _ = fs::remove_file(&path);

    {
      let mut s = Store::load(&path).unwrap();
      s.add("(A) write tests", true, today()).unwrap();
      let id = s.add("ship it", true, today()).unwrap();
      s.complete(id, today()).unwrap();
    }

    let reloaded = Store::load(&path).unwrap();
    let all = reloaded.list(&Filter::default());
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].task.legacy_prefix, Some('A'));
    assert!(all[1].task.completed);
  }

  #[test]
  fn legacy_lines_survive_every_store_write_and_reload() {
    let mut store = temp_store("legacy-writes");
    let fixtures = [
      "(A) 2026-06-01 call pri:A pri:B pri:A +work @phone due:2026-06-30 custom:yes",
      "x (Z) 2026-06-19 2026-06-01 call pri:B pri:A",
      "(A)call pri:A",
      "(a) call",
      "(AA) call",
      "call (A)",
    ];
    store.replace_raw(&fixtures.join("\n")).unwrap();
    let assert_disk = |store: &Store| {
      let reloaded = Store::load(store.path.clone()).unwrap();
      assert_eq!(reloaded.raw(), store.raw());
      assert_eq!(fs::read_to_string(&store.path).unwrap(), store.raw());
    };
    let ids: Vec<_> = store
      .list(&Filter::default())
      .iter()
      .map(|e| e.id)
      .collect();
    for (index, id) in ids.into_iter().enumerate() {
      let original = store.get(id).unwrap().task.to_string();
      store.update(id, &original).unwrap();
      assert_disk(&store);
      store.complete(id, today()).unwrap();
      assert_disk(&store);
      store.uncomplete(id).unwrap();
      assert_disk(&store);
      let expected = if index == 1 {
        "(Z) 2026-06-01 call pri:B pri:A"
      } else {
        fixtures[index]
      };
      assert_eq!(store.get(id).unwrap().task.to_string(), expected);
      let other = store.add("different task", false, today()).unwrap();
      store.update(other, "unrelated edit").unwrap();
      assert_disk(&store);
      assert_eq!(store.get(id).unwrap().task.to_string(), expected);
      store.delete(other).unwrap();
      assert_disk(&store);
    }
  }

  #[test]
  fn missing_id_is_not_found() {
    let mut s = temp_store("missing");
    assert!(matches!(s.update(999, "x"), Err(StoreError::NotFound)));
    assert!(matches!(s.delete(999), Err(StoreError::NotFound)));
  }

  #[test]
  fn empty_text_is_invalid() {
    let mut s = temp_store("empty");
    assert!(matches!(
      s.add("   ", true, today()),
      Err(StoreError::Invalid(_))
    ));
  }

  #[test]
  fn reload_from_disk_picks_up_external_edits() {
    let mut path = std::env::temp_dir();
    path.push(format!("ntr-store-reload-{}.txt", std::process::id()));
    let _ = fs::remove_file(&path);

    let mut s = Store::load(&path).unwrap();
    s.add("a", true, today()).unwrap();
    // The store's own write is not a change.
    assert!(!s.reload_from_disk().unwrap());

    // Another program rewrites the file out of band.
    fs::write(&path, "(A) one\ntwo +proj\n").unwrap();
    assert!(s.reload_from_disk().unwrap());
    let all = s.list(&Filter::default());
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].task.legacy_prefix, Some('A'));
    // Re-reading the same content reports no change.
    assert!(!s.reload_from_disk().unwrap());
  }

  #[test]
  fn reload_ignores_whitespace_only_difference() {
    let mut path = std::env::temp_dir();
    path.push(format!("ntr-store-reload-ws-{}.txt", std::process::id()));
    let _ = fs::remove_file(&path);

    let mut s = Store::load(&path).unwrap();
    s.add("only task", true, today()).unwrap();
    // A blank line the parser discards renders identically, so it is not a change.
    let mut text = s.raw();
    text.push('\n');
    fs::write(&path, text).unwrap();
    assert!(!s.reload_from_disk().unwrap());
  }
}
