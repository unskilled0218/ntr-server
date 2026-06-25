# 20 - remove-gui — code-review fix plan

## Decision: no code fixes

code-review.md scored the branch 99/100. The only finding is a 🟢 (the live-update machinery —
`watch.rs` + broadcast + `GET /api/events` — now has no in-repo consumer). It is an accepted,
deliberate retention, not a defect:

- It is published API: documented in the README "Live updates" section and the API table.
- It is still tested: `tests/api.rs::events_stream_opens` (route opens, correct content type)
  and `mutation_broadcasts_event` (a mutation broadcasts the matching `Change`).
- The R4 guard explicitly says not to flag "a stable public API with intentionally supported
  behavior" — so changing it would be wrong for a task scoped to removing the *GUI*.

Removing the SSE/watch machinery would be scope creep beyond "Remove GUI part" and would delete
a working API feature. So the remedy is to leave it as-is and keep the decision on record.

## What would have been in scope but needs nothing

- All `/api/*` endpoints, doc comments, README, version bump: verified clean (fmt, clippy, 10
  tests, grep for residual GUI refs all pass in implemented.md).

## Verification (re-confirm, no code change)

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` stay green.
- No edits to ship, so nothing new to test.

## Out of scope

- `docker-compose.yml` (unchanged by this branch).
- Removing the live-update feature (API, not GUI).
