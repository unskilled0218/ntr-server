# Brooks-Lint Review — Code (REVIEW.md)

**Mode:** PR Review
**Scope:** code — `web/app.js` (commit d05e1bb, ~38 lines); `README.md` wording
**Health Score:** 99/100
**Trend:** 100 → 99 (−1) — plan re-review to code review

The implementation matches the 100/100 plan exactly: `EMPTY_LABEL` is the single
source for the empty option's text, the add entry is emitted first with `value`
set explicitly afterwards, and the dead `plural` field is gone. One Suggestion,
constraint-bound, no code change required.

## Findings

### 🟢 Suggestion

**Coverage Illusion — the reordering and relabeling ship without an automated test**
Symptom: The invariant that makes the reorder safe — `populateLabelSelect` always
sets `select.value` so the top-pinned add entry never becomes the default — is
verified by reading the code, not by a test. `initPriorityOptions`,
`populateLabelSelect`, and the `composeText`/`resetAddForm` empty-state paths have
no mechanical coverage.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: A future change that drops the explicit `select.value` assignment, or
re-adds a caption-as-placeholder, would not be caught by CI.
Remedy: Accept as constraint-bound — the repo has no JS test harness by the
deliberate no-build-step decision, consistent with `add-task-selects`,
`project-context-dropdowns`, and `label-add-modal`. The Rust suite is unaffected
and green (`fmt`/`build`/`test`/`clippy`). No change required.

Considered and dismissed: the empty-option construction `new Option(EMPTY_LABEL,
"")` appears in two functions, but that is shared-constant reuse in two genuinely
distinct flows (one-time priority init vs. per-reload label rebuild), not
duplicated knowledge — the wording decision now lives in exactly one place.

## Summary

The code scores 99/100, over the 95 gate, with no actionable findings: the only
Suggestion is the constraint-bound absence of a JS test harness. No fixes needed.
Per REVIEW.md, there is nothing to implement from this review — the change is
recorded and complete.
