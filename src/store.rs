//! Persistent task store backed by a single todo.txt file.
//!
//! Tasks are assigned process-stable ids at load time (from a monotonic
//! counter) and ids are never reused while the process runs. The file stays a
//! pure todo.txt document; ids live only in memory as an index over it. Every
//! mutation writes the whole file atomically (temp file + rename) and only
//! swaps in the new in-memory state after the write succeeds, so the file and
//! memory never diverge.

use crate::task::Task;
use chrono::NaiveDate;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A task together with its process-stable id.
#[derive(Debug, Clone)]
pub struct Entry {
  pub id: u64,
  pub task: Task,
}

/// Optional filters for [`Store::list`].
#[derive(Debug, Default)]
pub struct Filter {
  pub completed: Option<bool>,
  pub priority: Option<char>,
  pub project: Option<String>,
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

    let mut entries = Vec::new();
    let mut next_id = 1;
    for line in content.lines() {
      if let Some(task) = Task::parse(line) {
        entries.push(Entry { id: next_id, task });
        next_id += 1;
      }
    }
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
        if let Some(p) = filter.priority {
          if e.task.priority != Some(p) {
            return false;
          }
        }
        if filter.project.is_some() || filter.context.is_some() {
          let d = e.task.derived();
          if let Some(pr) = &filter.project {
            if !d.projects.iter().any(|x| x == pr) {
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
    entries.push(Entry { id, task });
    self.save(entries, self.next_id + 1)?;
    Ok(id)
  }

  /// Replace a task's content from a raw todo.txt line.
  pub fn update(&mut self, id: u64, text: &str) -> Result<(), StoreError> {
    let task =
      Task::parse(text).ok_or_else(|| StoreError::Invalid("task text is empty".to_string()))?;
    let mut entries = self.entries.clone();
    let entry = entries
      .iter_mut()
      .find(|e| e.id == id)
      .ok_or(StoreError::NotFound)?;
    entry.task = task;
    self.save(entries, self.next_id)
  }

  /// Mark a task complete.
  pub fn complete(&mut self, id: u64, today: NaiveDate) -> Result<(), StoreError> {
    let mut entries = self.entries.clone();
    let entry = entries
      .iter_mut()
      .find(|e| e.id == id)
      .ok_or(StoreError::NotFound)?;
    entry.task.complete(today);
    self.save(entries, self.next_id)
  }

  /// Mark a task incomplete.
  pub fn uncomplete(&mut self, id: u64) -> Result<(), StoreError> {
    let mut entries = self.entries.clone();
    let entry = entries
      .iter_mut()
      .find(|e| e.id == id)
      .ok_or(StoreError::NotFound)?;
    entry.task.uncomplete();
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
    let mut out = String::new();
    for e in &self.entries {
      out.push_str(&e.task.to_string());
      out.push('\n');
    }
    out
  }

  /// Replace the entire file from raw todo.txt text. All ids are reassigned
  /// from the running counter (old ids become invalid); this is the one bulk
  /// operation that rotates ids.
  pub fn replace_raw(&mut self, text: &str) -> Result<(), StoreError> {
    let mut entries = Vec::new();
    let mut next_id = self.next_id;
    for line in text.lines() {
      if let Some(task) = Task::parse(line) {
        entries.push(Entry { id: next_id, task });
        next_id += 1;
      }
    }
    self.save(entries, next_id)
  }

  /// Persist `entries` to disk, then swap them into memory on success.
  fn save(&mut self, entries: Vec<Entry>, next_id: u64) -> Result<(), StoreError> {
    write_file(&self.path, &entries)?;
    self.entries = entries;
    self.next_id = next_id;
    Ok(())
  }
}

/// Atomically write entries: write to a sibling temp file, then rename over the
/// target so a crash never leaves a partially written todo.txt.
fn write_file(path: &Path, entries: &[Entry]) -> io::Result<()> {
  let mut data = String::new();
  for e in entries {
    data.push_str(&e.task.to_string());
    data.push('\n');
  }

  let mut tmp = path.as_os_str().to_owned();
  tmp.push(".tmp");
  let tmp = PathBuf::from(tmp);

  fs::write(&tmp, data)?;
  fs::rename(&tmp, path)?;
  Ok(())
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
  fn filter_by_project_and_completion() {
    let mut s = temp_store("filter");
    s.add("a +home", true, today()).unwrap();
    s.add("b +work", true, today()).unwrap();
    let id = s.add("c +home", true, today()).unwrap();
    s.complete(id, today()).unwrap();

    let home = Filter {
      project: Some("home".to_string()),
      ..Filter::default()
    };
    assert_eq!(s.list(&home).len(), 2);

    let active_home = Filter {
      project: Some("home".to_string()),
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
    assert_eq!(all[0].task.priority, Some('A'));
    assert!(all[1].task.completed);
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
}
