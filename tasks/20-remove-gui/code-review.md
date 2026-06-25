# Brooks-Lint Review — code (remove-gui vs main)

**Mode:** PR Review
**Scope:** branch changes vs main — `src/api.rs`, `src/lib.rs`, `src/main.rs`, `Cargo.toml`,
`README.md`, deletion of `web/` (1,777 deletions / 206 insertions). Generated/lock file
`Cargo.lock` skipped (tool output).

## Score 99/100

A subtractive change: it removes a whole surface and its wiring, so R1/R3/R4/R5 all move in
the healthy direction (fewer routes, fewer imports, less code). The keep/remove line is drawn
on the right seam — the browser frontend goes, the `/api/*` surface (incl. SSE) stays.

- 🟢 Accidental Complexity (R4 — Speculative Generality): with the frontend gone, the retained
  live-update machinery (`watch.rs` + broadcast channel + `GET /api/events`) has no first-party
  consumer in the repo; it survives as a documented HTTP affordance.
  Source: Fowler — Refactoring (Speculative Generality).
  Consequence: an end-to-end path with no in-tree caller can rot unnoticed.
  Remedy: none for this task — it is intentionally retained as published API (README "Live
  updates"; `tests/api.rs::events_stream_opens` and `mutation_broadcasts_event` still cover the
  route and the broadcast), exactly the "intentionally supported behavior" the R4 guard says
  not to flag. Logged so the decision is on record.

Quick Test Check: production behavior was *removed*, not added, and the removed routes never had
tests, so there is no Coverage Illusion — the 10 existing `/api/*` tests pass unchanged and
remain the regression gate. No test findings.

Considered, not flagged: the `Change` doc-comment re-point away from `web/app.js` (keeps the
wire-contract note truthful); `CorsLayer::permissive()` retained (correct — the API is now the
only surface); R2 — the change touches 5 files but all for the one purpose of removing the GUI
(coordinated removal, not shotgun surgery). Out of scope: `docker-compose.yml` is unchanged by
this branch, so its contents are not in review scope.

## Verdict

99/100, above the >95 gate. The single 🟢 is an accepted, deliberate retention, not a defect.
No code fixes required — see review-plan.md.
