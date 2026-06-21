# 16 - Live sync (watch files and update live)

## Goal

When one client changes a task or the note, every other open client reflects the
change live, without a manual reload. The motivating case: a user edits from their
phone and wants their desktop to update on its own.

A second, related source of change is the file itself: the todo.txt and Note.md are
plain files that another program (or the user's editor / a file-sync app) may rewrite
out of band. Those edits must propagate too.

## Design in one sentence

The file is the single source of truth; a change reaches other clients over one broadcast
channel — announced by the API handler when the server makes the change, and by a file
poller when something else does — and delivered to browsers as one Server-Sent Events
stream.

## Why this shape

- **One mechanism, two triggers — because a change has two origins one cannot see.** A
  task/note change is either made *through this server* (an API handler) or made *to the
  file behind its back* (an editor, a file-sync app). These need two triggers, not one,
  and the reason is concrete: a handler updates memory and disk *together*, so a poller
  that fires only when disk and memory **differ** can never observe an API change — by the
  time it reads, they already match. So the handler must announce its own writes; the
  poller exists for exactly the changes the handler cannot announce (out-of-band edits,
  where disk and memory do differ). They share one channel, one event format, and one
  `notify` — the *mechanism* has a single home — and they never double-fire: an API write
  leaves disk == memory, so the poller stays silent for it.
  > Note: an earlier draft proposed a poller-*only* design, on the theory that "every
  > change lands in the file, so one poller catches them all". That is wrong for the
  > primary use case (phone edits via the app -> desktop updates): the handler keeps disk
  > and memory in lockstep, so the poller sees no diff and never fires. End-to-end testing
  > caught it; hence the two-trigger design above.
- **SSE, not WebSocket.** The traffic is one-directional (server -> client signal) and
  the existing mutations already flow over plain HTTP. SSE is the minimal fit: it is a
  GET that streams `text/event-stream`, auto-reconnects in the browser, and needs no
  new client library. A WebSocket would add a second protocol and a handshake for a
  one-way notification we can express as a stream.
- **Signal, don't ship state.** An event carries only *which resource changed*
  (`tasks` or `note`), and the client re-GETs that resource through the endpoints it
  already uses. The alternative — pushing the new content down the event — would
  duplicate the read path and force the event format to track every DTO change. The
  signal stays a stable, tiny contract.
- **Poll-and-compare, not an OS file watcher.** The files are tiny and frequently live
  on bind mounts / synced folders where inotify events are unreliable. A 1s poll that
  re-reads and compares is dependency-free, robust on every filesystem, and dead
  simple. The poller reloads only when the on-disk content actually differs from
  memory, so the server's *own* writes (which it just made) compare equal and never
  trigger a spurious broadcast or an id churn.

## Server changes

### `parse_entries` helper (src/store.rs)

Today the "turn todo.txt text into ids+entries" decision is written twice — once in
`Store::load`, once in `Store::replace_raw` — and `reload_from_disk` would be a third.
Extract it once:

```
fn parse_entries(content: &str, start_id: u64) -> (Vec<Entry>, u64)
```

It runs the `for line in content.lines() { if let Some(task) = Task::parse(line) { ... } }`
loop, assigning ids from `start_id` upward, and returns the entries plus the next free
id. `load` (start_id = 1), `replace_raw` (start_id = `next_id`), and the new
`reload_from_disk` all call it, so the parse/id rule lives in exactly one place.

### `Store::reload_from_disk(&mut self) -> io::Result<bool>` (src/store.rs)

The mirror of `save`: `save` writes memory -> disk, `reload_from_disk` pulls disk ->
memory. Re-read `self.path` (a missing file is empty, as in `load`), build entries with
`parse_entries(&content, self.next_id)`, and compare the canonical render of the parsed
entries against the current render. If they match, return `Ok(false)` and change nothing
— this is how the server recognises its own just-written file and avoids needless
reloads. If they differ, swap the new entries and `next_id` into memory and return
`Ok(true)`.

Reusing `render` for the comparison means a whitespace-only difference the parser
discards (e.g. a blank line) is correctly treated as no change. Like `replace_raw`,
loading from a file without ids rotates ids; clients re-fetch on the event, so they pick
up the new ids.

### `NoteStore::reload_from_disk(&mut self) -> io::Result<bool>` (src/note.rs)

The same mirror for the note: re-read `self.path` (missing = empty), and if the text
differs from `self.content`, swap it in and return `Ok(true)`, else `Ok(false)`. The
note has no structure, so the comparison is a plain string compare.

### Change events + broadcast channel (src/api.rs)

- `pub enum Change { Tasks, Note }` with `fn as_str(&self) -> &'static str` returning
  `"tasks"` / `"note"`. This is the entire wire contract for an event. The two strings
  are also read by the JS `change` handler, so the definition carries a comment naming
  the JS dispatch as the other half of the contract (and the JS carries the mirror
  comment) — the same cross-reference style the codebase uses for `VIEWS` <-> nav
  buttons, so a rename on one side points you straight at the other.
- `AppState` gains `pub tx: broadcast::Sender<Change>` (tokio). `AppState::new` creates
  it with a small capacity (16) and keeps only the sender; receivers are made on demand
  (one when a client connects to the SSE stream). `AppState::new`'s signature is
  unchanged, so existing callers (incl. tests) keep working.
- `AppState::notify(&self, change: Change)` does `let _ = self.tx.send(change)` — a send
  with no subscribers is fine (it returns `Err`, which we ignore). Called from both
  triggers: each mutation handler after a successful write, and the poller for out-of-band
  edits (see "Why this shape").

`Change` derives `Clone, Copy` (broadcast requires `Clone`).

### Announce on every mutation (src/api.rs)

After a successful write, each handler calls `state.notify(...)`:
`create/update/complete/uncomplete/delete/put_raw` -> `Change::Tasks`; `put_note` ->
`Change::Note`. One line per handler, co-located with the mutation it announces. This is
the instant path: the acting client sees its change in the HTTP response, every other
client gets the event.

### SSE endpoint `GET /api/events` (src/api.rs)

Subscribe to `state.tx` and stream each `Change` as an SSE event named `change` with the
`Change::as_str()` payload. The stream is built with `futures_util::stream::unfold` over
the `broadcast::Receiver`:

- `Ok(change)` -> yield `Event::default().event("change").data(change.as_str())`.
- `Lagged(_)` (a slow client missed events) -> skip and keep going; the client re-syncs
  on its next event or on reconnect, so a dropped signal only delays, never corrupts.
- `Closed` -> end the stream.

Wrap it in `Sse::new(stream).keep_alive(KeepAlive::default())` so idle connections (and
the proxies between them) stay open. axum sets the `text/event-stream` content type.
Route added before the CORS layer alongside the other routes.

### Dependency

Add `futures-util = "0.3"` to `[dependencies]`. It is already in `Cargo.lock` as a
transitive dependency of axum, so this only surfaces it for direct use (`stream::unfold`)
— no new crate is downloaded. (Build is offline-cached; flag if the registry is needed.)

### Wire up the poller (src/main.rs, new src/watch.rs)

- New module `watch` with `pub fn spawn(state: AppState, interval: Duration)`. It
  `tokio::spawn`s a loop: `tokio::time::interval(interval).tick().await`, then for the
  store and the note take the write lock, call `reload_from_disk()`, and on `Ok(true)`
  call `state.notify(...)`. A poisoned lock or an I/O error is skipped (logged to stderr);
  the next tick retries. The poller owns no state of its own — `reload_from_disk` is the
  one place that decides "did this actually change", so the loop stays trivial.
- `main` builds the state, hands a clone to `watch::spawn(state.clone(),
  Duration::from_secs(1))`, then moves the original into `api::router(state)` as today.
  `AppState` is already `Clone`.

API-driven updates are announced instantly (the handler path), so the common
phone-edits-via-the-app case is immediate. The 1s interval bounds only the latency of
*out-of-band* file edits — the changes the handler path cannot see. For a personal todo
app that is well within "live".

## Client changes (web/app.js)

- `onTasksChanged()` = `await refresh(); await loadLabels();` — the same pair the add
  form already runs after a successful POST, named once so the event handler and the
  form share it.
- `maybeReloadNote()` reloads the note **only when it is safe to overwrite the editor**:
  skip while `savingNote`, while `noteDirty()`, or while the editor is focused
  (`document.activeElement === els.noteInput`). The desktop sitting idle reloads; the
  device actually being typed on keeps its text and lets its own autosave win. This is
  the one rule that prevents a remote update from stealing the caret or clobbering an
  unsaved local edit, so it lives in one function.
- `connectEvents()` opens `new EventSource("/api/events")` and on a `change` event
  dispatches by `e.data`: `"tasks"` -> `onTasksChanged()`, `"note"` -> `maybeReloadNote()`.
  These two literals are the client half of the `Change::as_str()` wire contract, so the
  dispatch carries a comment pointing back at `Change` in src/api.rs (which mirrors it),
  keeping the two ends discoverable from each other.
  The browser reconnects EventSource automatically; on a *re*-connection (not the first
  open) it calls both resync functions, because events that fired during the gap were
  missed. The first open is skipped since the initial `refresh()/loadLabels()/loadNote()`
  at startup already loaded everything.
- Call `connectEvents()` once at the end of init, after the existing initial loads.

No HTML/CSS changes: live updates reuse the existing list and note rendering.

## Tests

- **store.rs unit:** write a todo file out of band, `reload_from_disk()` returns `true`
  and the new tasks are visible; a second call returns `false` (no change); after a
  normal `add`, `reload_from_disk()` returns `false` (memory already matches disk — the
  own-write case).
- **note.rs unit:** out-of-band write -> `reload_from_disk()` true and content updated;
  second call false; after `replace`, false.
- **api.rs integration:** `GET /api/events` returns `200` with content type
  `text/event-stream` (check status + header via `oneshot`; the body is an open stream,
  so it is not consumed). Confirms the route and SSE wiring without depending on timing.

The poller loop itself (timing) is left untested directly; its only logic is "call
`reload_from_disk`, notify if it changed", and both halves are covered above.

## Out of scope

- Per-field / per-task diffing: clients re-fetch the whole resource on a signal. The
  lists are small and this keeps the event contract tiny.
- Conflict resolution for two clients editing the *note* at once: the dirty/focus guard
  protects the active editor; last-write-wins otherwise, unchanged from today.
- Auth on the event stream: it inherits the server's existing userless, loopback-by-
  default posture (same as every other route).
