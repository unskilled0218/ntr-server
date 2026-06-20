# Brooks-Lint Review

**Mode:** PR Review
**Scope:** plans/dropdown-menu-cleanup.md (design plan; web/* frontend feature)
**Health Score:** 100/100 (re-review after the section-2 fix; first pass was 99/100)
**Trend:** 99 → 100 (+1) after one iteration

The plan is small, single-purpose, and ready to implement: relabel the empty
placeholder of all three add-form dropdowns to `None`, move the project/context
add entry to the top of its option list, and delete the now-dead `plural` field.
The one first-pass Suggestion (a duplicated literal) has been folded in.

---

## First pass (99/100)

### 🟢 Suggestion — resolved

**Knowledge Duplication — the empty-option label "None" as a bare literal in two places**
Symptom: The draft set the placeholder text with `new Option("None", "")` in both
`initPriorityOptions` and `populateLabelSelect`. The same product decision — what
the no-selection option is called — was expressed as a literal in two call sites,
where the prior code had sourced the project/context caption from a single
`KINDS[kind].plural` field.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY: Don't Repeat Yourself
(a configuration value repeated as a literal in multiple places).
Consequence: A later wording change (`—`, `Any`, or a localized string) would have
to be made in two spots and could silently drift, reintroducing exactly the kind
of caption inconsistency this change set out to remove.
Remedy applied: Section 2 now defines `EMPTY_LABEL = "None"` once at module scope
(beside `ADD_VALUE`); both call sites use `new Option(EMPTY_LABEL, "")`, and
section 4's file list calls it out. Single source restored.

## Remaining (100/100)

No further findings. Notes considered and dismissed:

- **Coverage Illusion (not counted):** the plan ships no automated test for the
  reordering/relabeling. Constraint-bound — the repo has no JS test harness by the
  deliberate no-build-step decision, consistent with `add-task-selects`,
  `project-context-dropdowns`, and `label-add-modal`. Section 5 documents manual
  verification plus the green Rust gates (`fmt`/`build`/`test`/`clippy`). This is
  the established pattern for WebUI-only changes and is not a deduction.
- **Accidental Complexity:** the change *removes* a dead field (`plural`) and adds
  no abstraction or extension point — it reduces complexity. No finding.
- **Change Propagation:** the `plural` removal was verified by grep to have a
  single reader (the placeholder text being replaced); `singular` and `prefix`
  remain in use. No unrelated module is touched. No finding.

---

## Summary

After one iteration the plan clears the 95 gate at 100/100. The empty-option
wording now has a single source (`EMPTY_LABEL`), the add-entry reordering is
reasoned to be safe because `populateLabelSelect` always sets `select.value`
explicitly (so the first-option default never fires), and the empty-state
*semantics* (`value=""`) are preserved throughout — only the visible label and the
option order change. Proceed to implementation.

---

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
