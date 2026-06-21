//! Background file watcher: notices *out-of-band* edits to the todo or note file — made
//! by an editor, a file-sync app, or any program other than this server — and announces
//! them to live clients.
//!
//! This covers exactly the changes the API path cannot announce for itself. A mutation
//! handler updates memory and disk together, so it announces its own change directly
//! (see [`crate::api::AppState::notify`]); by the time this watcher re-reads the file,
//! disk already matches memory and it correctly stays silent. An edit made to the file
//! behind the server's back, by contrast, leaves disk and memory differing — that is what
//! the watcher exists to catch. The poll interval bounds how long another client waits
//! for such an out-of-band edit to propagate.

use crate::api::{AppState, Change};
use crate::note::NoteStore;
use crate::store::Store;
use std::io;
use std::sync::RwLock;
use std::time::Duration;

/// Lets [`poll`] reload either store through one code path. Both stores already have an
/// inherent `reload_from_disk`; this trait only exposes it generically so the
/// poll-one-resource policy lives in a single place.
trait ReloadFromDisk {
  fn reload_from_disk(&mut self) -> io::Result<bool>;
}

impl ReloadFromDisk for Store {
  fn reload_from_disk(&mut self) -> io::Result<bool> {
    Store::reload_from_disk(self)
  }
}

impl ReloadFromDisk for NoteStore {
  fn reload_from_disk(&mut self) -> io::Result<bool> {
    NoteStore::reload_from_disk(self)
  }
}

/// Spawn the watcher loop on the tokio runtime. Every `interval` it pulls each file back
/// into memory via the store's own `reload_from_disk` (which decides whether anything
/// actually changed) and announces the ones that did. A poisoned lock or an I/O error is
/// logged and skipped; the next tick retries.
pub fn spawn(state: AppState, interval: Duration) {
  tokio::spawn(async move {
    let mut ticker = tokio::time::interval(interval);
    loop {
      ticker.tick().await;
      reload_and_notify(&state);
    }
  });
}

/// One poll pass: reload the store and the note, announcing each that changed.
fn reload_and_notify(state: &AppState) {
  poll(&state.store, state, Change::Tasks, "tasks");
  poll(&state.note, state, Change::Note, "note");
}

/// Reload one resource from disk and announce `change` if it actually changed. The single
/// home for the "reload, notify on change, log on error" policy; `label` names the
/// resource in log messages. A poisoned lock or an I/O error is logged and skipped so the
/// next tick can retry.
fn poll<T: ReloadFromDisk>(lock: &RwLock<T>, state: &AppState, change: Change, label: &str) {
  match lock.write() {
    Ok(mut resource) => match resource.reload_from_disk() {
      Ok(true) => state.notify(change),
      Ok(false) => {}
      Err(e) => eprintln!("watch: reloading {label} failed: {e}"),
    },
    Err(_) => eprintln!("watch: {label} store lock poisoned"),
  }
}
