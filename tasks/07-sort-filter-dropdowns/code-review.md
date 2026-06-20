# Brooks-Lint Review — Code (REVIEW.md)

**Mode:** PR Review
**Scope:** code — `web/app.js`, `web/index.html`, `web/style.css` (commit e320364);
`README.md` wording
**Health Score:** 99/100
**Trend:** 100 → 99 (−1) — plan re-review to code review

The implementation matches the 100/100 plan exactly: `STATUSES` is the single
source for the status→`completed` mapping, the sort re-renders from the cached
`lastTasks` without refetching, `populateFilterSelect` writes back `state[kind]`
so the dropdown and the query can never disagree, and the add-form populator is
untouched. One Suggestion, constraint-bound, no code change required.

## Findings

### 🟢 Suggestion

**Coverage Illusion — the filter/sort behavior ships without an automated test**
Symptom: The invariants that make the change correct — `cmpMissingLast` sinking
absent keys to the bottom, `sortTasks` not mutating the fetched array, a sort
change re-rendering without a refetch, and `populateFilterSelect` clearing
`state[kind]` when a label vanishes — are verified by reading the code, not by a
test. The new `buildQuery`, `SORTS` comparators, and `render`/`refresh` split have
no mechanical coverage.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: A future change that, say, drops the `[...tasks]` copy in `sortTasks`
(mutating cached state) or removes the `state[kind]` write-back would not be caught
by CI.
Remedy: Accept as constraint-bound — the repo has no JS test harness by the
deliberate no-build-step decision, consistent with `add-task-selects`,
`project-context-dropdowns`, `label-add-modal`, and `dropdown-menu-cleanup`. The
Rust suite is unaffected and green (`fmt`/`build`/`test`/`clippy`), and the feature
was smoke-tested manually (combined `completed`+`project` query, served assets).
No change required.

Considered and dismissed:

- **Knowledge Duplication / Accidental Complexity — two label-select populators.**
  `populateLabelSelect` and `populateFilterSelect` share a "rebuild a kind-keyed
  select, preserve the previous value" shape, but their head options (add entry +
  `None` + staged union vs. a single `All <kind>s`) and their follow-ups
  (`priorLabel[kind]` vs. `state[kind]`) differ. A shared helper would either need
  add-entry/empty-label flag arguments (R1 Flag Arguments) or would extract only a
  one-line `names.sort()` loop — over-abstraction worse than the trivial overlap.
  The genuinely shared knowledge (the label set, the `KINDS` config) is already
  centralised. No finding.
- **`state[kind]` write inside `populateFilterSelect`** is an intentional side
  effect (keeping the dropdown and query in sync) and runs only via `loadLabels`,
  documented at the call site. Not Feature Envy. No finding.

## Summary

The code scores 99/100, over the 95 gate, with no actionable findings: the only
Suggestion is the constraint-bound absence of a JS test harness. Per REVIEW.md,
there is nothing to implement from this review — the change is recorded and
complete.
