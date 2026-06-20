# Brooks-Lint Review — Code (REVIEW.md)

**Mode:** PR Review
**Scope:** code — `web/app.js`, `web/index.html`, `web/style.css` (commit b0a7937, ~120 lines)
**Health Score:** 98/100
**Trend:** 99 → 98 (−1) — plan re-review to code review

The implementation faithfully realises the 99/100 plan: short single-purpose
functions, the collision-proof `data-add` sentinel, and a pure validator. Two
Suggestions, neither requiring a code change.

## Findings

### 🟢 Suggestion

**Knowledge Duplication — "what labels exist" is derived two ways**
Symptom: `loadLabels` computes the option set as `server[kind] ∪ staged[kind]`,
while `confirmLabel` rebuilds it by scraping the live `<option>` values and
deleting the placeholder and `ADD_VALUE`. The same concept ("the known labels of
this kind") is expressed through two different sources.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY: Don't Repeat Yourself.
Consequence: If the `loadLabels` derivation later changes (e.g. server labels get
filtered), the DOM-scraping path in `confirmLabel` would silently diverge and could
re-render a stale set.
Remedy: Acceptable as-is — the rendered options are a correct local cache and
reusing them avoids a redundant fetch on every label add. If the two ever need to
share logic, cache the last server-derived set per kind and feed
`server[kind] ∪ staged[kind]` to both call sites. No change applied (over gate).

**Coverage Illusion — new client logic ships without an automated test**
Symptom: `parseLabelInput`, `onLabelSelectChange`, and `confirmLabel` are covered
by manual checks only.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: Validation/sentinel regressions are not caught mechanically.
Remedy: Accept as constraint-bound — no JS test harness exists by the deliberate
no-build-step decision (consistent with `add-task-selects` /
`project-context-dropdowns`). `parseLabelInput` is pure and ready to test if a
harness is added. No change required.

## Summary

The code scores 98/100, over the 95 gate, with no actionable findings: the one
DRY suggestion is a justified local-cache trade-off and the test suggestion is
constraint-bound. No fixes needed; the backend is untouched and green
(`cargo fmt`/`build`/`test`/`clippy`).
