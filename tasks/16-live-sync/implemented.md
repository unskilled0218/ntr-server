# 16 - Live sync — implemented

Live cross-client updates: when a task or the note changes, every other open client
reflects it without a manual reload. A change is announced over one broadcast channel and
delivered to browsers as a Server-Sent Events stream.

## Two triggers, one mechanism

A change has two origins that one cannot see, so there are two notify call sites feeding
one channel:

1. **API mutations** — each mutating handler calls `state.notify(...)` after a successful
   write. Required because a handler updates memory and disk together; the poller (which
   fires only when disk != memory) can never observe an API change.
2. **Out-of-band file edits** — a 1s file poller reloads from disk and notifies only when
   on-disk content actually differs from memory. Covers edits made to the file by an
   editor / sync app — exactly what the handler path cannot see.

They never double-fire: an API write leaves disk == memory, so the poller stays silent.

## Files changed

- `src/store.rs` — extracted `parse_entries(content, start_id)` (one home for the
  parse/id rule, shared by `load`, `replace_raw`, and the new `reload_from_disk`); added
  `Store::reload_from_disk()` returning whether the disk differed from memory.
- `src/note.rs` — added `NoteStore::reload_from_disk()` (plain text compare).
- `src/api.rs` — `Change { Tasks, Note }` enum (wire contract, cross-referenced with the
  JS); `AppState.tx: broadcast::Sender<Change>` + `AppState::notify`; `notify` calls in
  all seven mutating handlers; `GET /api/events` SSE endpoint built with
  `futures_util::stream::unfold` over a broadcast receiver, with keep-alive.
- `src/watch.rs` — new module: `spawn(state, interval)` polling loop that reloads each
  store and notifies on a real change.
- `src/main.rs` — `watch::spawn(state.clone(), 1s)` before serving.
- `src/lib.rs` — registered `watch` module.
- `Cargo.toml` — added `futures-util` (already in the lock via axum) and tokio `time` +
  `sync` features.
- `web/app.js` — `onTasksChanged()`, `maybeReloadNote()` (reloads only when the editor is
  idle/clean/unfocused, so a remote update never steals the caret or clobbers a local
  edit), `connectEvents()` (EventSource with reconnect-resync), wired at init.
- `tests/api.rs` — `events_stream_opens` (200 + text/event-stream).
- `src/store.rs` / `src/note.rs` — unit tests for `reload_from_disk` (external edit
  detected; own write and whitespace-only diff are no-ops).

## Verification

- `cargo test` — 29 pass (20 lib unit + 9 integration). `cargo fmt --check` clean,
  `cargo clippy --all-targets` clean.
- End-to-end against a running server: an API POST (task) and PUT (note) each emitted one
  `change` event to an idle SSE client; a direct out-of-band file write emitted one more.
  Three events total, no duplicates; the reloaded store/note reflected the external edit.

## Note on the plan

The plan went through a mid-implementation correction: an interim "poller-only" design
(plan round 2) was proved wrong by end-to-end testing — the poller cannot see API
mutations because the handler keeps disk == memory. The two-trigger design above is the
corrected, verified result (plan round 3). See plan-review.md.
