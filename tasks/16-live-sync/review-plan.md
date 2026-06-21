# 16 - Live sync — code-review fix plan

Addresses the two findings from the code review (review-implemented.md). Both are small.

## Fix 1 (🟡 Coverage Illusion) — test that a mutation broadcasts a `change` event

The behavior that already regressed once (poller-only design emitted nothing for API
writes) is only manually verified. Add an automated guard.

- `tests/api.rs`: add a helper `app_with_state(tag) -> (Router, AppState)` that builds the
  store/note/config exactly as `app` does but returns the `AppState` too, then have the
  existing `app` delegate to it (`app` = `app_with_state(tag).0`) so the construction rule
  stays in one place (no duplication introduced by the fix).
- New test `mutation_broadcasts_event`:
  - `let (app, state) = app_with_state("events-broadcast");`
  - `let mut rx = state.tx.subscribe();` (subscribe *before* mutating)
  - POST `/api/tasks` through the router; assert `rx.try_recv()` yields `Change::Tasks`
    (via `matches!`, since `Change` is `Copy` but not `PartialEq`).
  - PUT `/api/note`; assert `rx.try_recv()` yields `Change::Note`.
- `Change` and `AppState.tx` are already `pub`, so no production change is needed. Import
  `Change` and `tokio::sync::broadcast` error type as needed in the test.

This locks in "a successful mutation announces the matching `Change`" — exactly the wiring
whose absence broke the feature.

## Fix 2 (🟢 Knowledge Duplication) — collapse the two watcher arms

The "reload, notify on change, log on error" decision is written twice in
`reload_and_notify` (once per resource), differing only in lock, `Change` variant, and log
label.

- In `src/watch.rs`, define a private trait `trait ReloadFromDisk { fn reload_from_disk(&mut self) -> std::io::Result<bool>; }`
  and impl it for `Store` and `NoteStore` by forwarding to their inherent methods (the
  inherent methods stay; the trait just lets the helper be generic).
- Replace the two match blocks with one generic helper:
  ```
  fn poll<T: ReloadFromDisk>(lock: &RwLock<T>, state: &AppState, change: Change, label: &str)
  ```
  which does the `write()` / `reload_from_disk()` / `notify` / `eprintln!` logic once,
  using `label` in the messages. `reload_and_notify` becomes two calls:
  `poll(&state.store, state, Change::Tasks, "tasks"); poll(&state.note, state, Change::Note, "note");`
- The policy (how a change is announced, how errors/poisoned locks are logged) now lives
  in exactly one place.

## Verification

- `cargo test` (expect 31 passing: +1 new integration test), `cargo fmt --check`,
  `cargo clippy --all-targets` all clean.
- No behavior change for clients; the existing end-to-end checks already cover delivery.

## Out of scope

- Unit-testing the poller's *timing* (interval firing): still left to the manual e2e
  check; the new test covers the notify wiring, and `reload_from_disk` is unit-tested.
