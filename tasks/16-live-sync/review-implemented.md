# 16 - Live sync — review fixes implemented

Applied both code-review findings (code-review.md) per review-plan.md.

## Fix 1 — Coverage Illusion (🟡 -> resolved)
- tests/api.rs: added `app_with_state(tag) -> (Router, AppState)`; `app` now delegates to
  it (construction rule stays single-homed).
- New test `mutation_broadcasts_event`: subscribes to state.tx, then asserts
  POST /api/tasks -> Change::Tasks and PUT /api/note -> Change::Note. Guards the exact
  wiring that regressed once (poller-only emitted nothing for API writes).

## Fix 2 — Knowledge Duplication (🟢 -> resolved)
- src/watch.rs: added private `ReloadFromDisk` trait (impl for Store and NoteStore,
  forwarding to their inherent methods) and a generic `poll<T: ReloadFromDisk>` helper.
  `reload_and_notify` is now two `poll(...)` calls; the reload/notify/log policy has one
  home.

## Verification
- cargo test: 30 pass (20 lib + 10 integration, incl. mutation_broadcasts_event).
- cargo fmt --check clean; cargo clippy --all-targets clean.

Re-review: 100/100.
