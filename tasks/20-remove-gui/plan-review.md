# Brooks-Lint Review — plan.md

**Mode:** PR Review
**Scope:** tasks/20-remove-gui/plan.md (plan to remove the browser WebUI, leaving an API-only server)

## Round 1 — Score 99/100

A clean subtractive plan: it deletes a whole surface (the frontend) and its routes, removing
code rather than adding it, so R2/R3/R4 mostly move in the healthy direction. Scope is drawn
precisely and the keep/remove boundary is argued.

- 🟢 Accidental Complexity (R4 — Speculative Generality): once the frontend is gone, the
  retained live-update machinery (`watch.rs` + broadcast channel + `GET /api/events`) has no
  in-repo consumer left; it survives purely as a documented HTTP affordance.
  Source: Fowler — Refactoring (Speculative Generality) / Ousterhout — A Philosophy of Software
  Design, Ch. 3.
  Consequence: machinery with no first-party caller can rot unnoticed — nothing in the tree
  exercises the end-to-end path, so a future regression in the broadcast/SSE wiring would only
  surface for an external subscriber.
  Remedy: none required for this task — the plan makes the retention deliberate (it is part of
  the published API in the README table and `tests/api.rs` still covers the route + broadcast),
  which is exactly the "stable public API with intentionally supported behavior" the R4 guard
  says not to flag. Recorded as a 🟢 so the decision is on the record, not as a fix to make.

Considered, not flagged: keeping `CorsLayer::permissive()` (the API is now the only surface —
correct, and the plan justifies it); the doc-comment re-point of the `Change` enum away from
`web/app.js` (keeps the wire-contract note truthful after the consumer moves out of the repo);
version bump 0.1.0 -> 0.2.0 (matches "Update version", appropriate for a removed surface).

## Verdict

99/100, above the >95 gate. No plan changes needed — implementing.
