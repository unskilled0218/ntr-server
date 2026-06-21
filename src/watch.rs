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
use std::time::Duration;

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

/// One poll pass: reload the store and the note, announcing each that changed. Split out
/// so the loop stays trivial; `reload_from_disk` is the one place that judges "changed".
fn reload_and_notify(state: &AppState) {
  match state.store.write() {
    Ok(mut store) => match store.reload_from_disk() {
      Ok(true) => state.notify(Change::Tasks),
      Ok(false) => {}
      Err(e) => eprintln!("watch: reloading tasks failed: {e}"),
    },
    Err(_) => eprintln!("watch: task store lock poisoned"),
  }
  match state.note.write() {
    Ok(mut note) => match note.reload_from_disk() {
      Ok(true) => state.notify(Change::Note),
      Ok(false) => {}
      Err(e) => eprintln!("watch: reloading note failed: {e}"),
    },
    Err(_) => eprintln!("watch: note store lock poisoned"),
  }
}
