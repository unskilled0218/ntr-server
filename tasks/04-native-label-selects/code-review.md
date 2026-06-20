# Code review (REVIEW.md step 1 — `/brooks-review code`)

**Mode:** PR Review
**Scope:** code: `web/app.js`, `web/index.html`, `web/style.css` (commit
`65ffb99`, diff vs `main`)
**Health Score:** 98/100

A net-deletion implementation faithful to the approved plan; the new code is
short and clear, with the same two constraint-bound Suggestions and no actionable
defect.

## Findings

### 🟢 Suggestion

**Knowledge Duplication — caption string in HTML and JS**
Symptom: the field caption ("Projects"/"Contexts") is in the `<label>` text in
`web/index.html` and again as the placeholder option text via `KINDS[kind].plural`
in `populateLabelSelect`.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY.
Consequence: a caption rename must touch both spots or they drift.
Remedy: accept — this is exactly how the existing Priority control works and is
the consistency the feature exists to create; one short display string per kind,
not logic. No change.

**Coverage Illusion — frontend logic without an automated test**
Symptom: `populateLabelSelect`/`composeText`/`loadLabels` changed with no test in
the diff.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: a regression in compose/populate would not be caught by CI.
Remedy: accept as constraint-bound — no JS harness by the no-build-step decision;
`composeText` kept pure and ready to test; the change is a net deletion that
reduces behavioral surface. No change.

## Summary

The implementation matches `plans/native-label-selects.md` exactly: custom
widget/modal removed, native single-select dropdowns added, README synced.
`populateLabelSelect`'s value-restore logic was checked against the
placeholder / still-present / vanished cases and is correct — no latent bug like
the prior round's sigil-stripping issue. Both findings are the same
constraint-bound Suggestions as the plan review; neither is actionable, so
REVIEW.md's fix-plan/implement steps have nothing to do. Code clears the 95 gate
at 98/100.
