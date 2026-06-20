# Brooks-Lint Review

**Mode:** PR Review
**Scope:** plans/sort-filter-dropdowns.md (design plan; web/* frontend feature)
**Health Score:** 100/100 (re-review after the section-2/5 fix; first pass was 99/100)
**Trend:** 99 → 100 (+1) after one iteration

The plan is single-purpose and ready to implement: replace the filter buttons +
free-text tag input with Status/Project/Context dropdowns, add a client-side Sort
dropdown, and reuse the existing label-gathering and query-param machinery with no
server change. The one first-pass Suggestion (a status decision split across two
places) has been folded in.


## First pass (99/100)

### 🟢 Suggestion — resolved

**Knowledge Duplication — status value strings decided in two places**
Symptom: The draft enumerated the status values in a `STATUSES` table
(`all`/`active`/`completed`) but then re-interpreted two of them by bare literal
inside `buildQuery` (`if (state.status === "active") ... "false"`,
`if (state.status === "completed") ... "true"`). The single decision "what each
status means as a `completed` query param" lived partly in the table and partly in
`buildQuery`.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY: Don't Repeat Yourself
(a configuration value / mapping expressed as literals in more than one place).
Consequence: Renaming a status value, adding a fourth status, or changing its
query mapping would require coordinated edits in both spots and could silently
drift — and it diverges from the sibling `SORTS` config, which already pairs each
label with its behavior (the comparator) in one entry.
Remedy applied: Section 2/5 now make `STATUSES` a keyed map whose entry carries the
query meaning beside the label — `{ all: {label, completed:null}, active:
{label, completed:false}, completed: {label, completed:true} }` — and `buildQuery`
reads `STATUSES[state.status].completed` instead of re-deciding. This gives status
the same label-plus-behavior shape as `SORTS` (conceptual integrity) and a single
source for the mapping.

## Remaining (100/100)

No further findings. Notes considered and dismissed:

- **Accidental Complexity (not counted):** the plan keeps two label-select
  populators (`populateLabelSelect` for the add form, `populateFilterSelect` for
  the filters) that look similar. Dismissed — section 4/9 reason that merging them
  would require add-entry / staged / empty-label flag arguments (R1 Flag Arguments,
  worse than two short single-purpose functions), and the genuinely shared
  knowledge — the label *set* and the `KINDS` config — is already centralised in
  `loadLabels`/`KINDS`. Not duplicated knowledge.
- **Knowledge Duplication, empty-option wording (dismissed):** the add form's
  `None` (`EMPTY_LABEL`) and the filter's `All projects`/`All contexts` are
  distinct concepts (no-label-on-a-new-task vs. no-filter), not the same decision
  expressed twice. The `All ${singular}s` form reuses the existing `singular`
  field rather than reviving the removed `plural` field.
- **Coverage Illusion (not counted):** the feature ships no automated test for the
  filter/sort behavior. Constraint-bound — the repo has no JS test harness by the
  deliberate no-build-step decision, consistent with `add-task-selects`,
  `project-context-dropdowns`, `label-add-modal`, and `dropdown-menu-cleanup`.
  Section 8 documents manual verification plus the green Rust gates
  (`fmt`/`build`/`test`/`clippy`). This is the established pattern for WebUI-only
  changes and is not a deduction.
- **Change Propagation:** WebUI-only; no Rust/API/store/test change, no new
  imports, no cross-module edits. The sort is client-side and the filters reuse the
  server's existing `completed`/`project`/`context` params. No finding.


## Summary

After one iteration the plan clears the 95 gate at 100/100. The status mapping now
has a single source (`STATUSES` carries the `completed` meaning beside the label,
mirroring `SORTS`), the sort never refetches (the `render`/`refresh` split over a
cached `lastTasks`), and the filter dropdown can never disagree with the query
(`populateFilterSelect` writes back `state[kind]` when a label vanishes). Proceed
to implementation.

