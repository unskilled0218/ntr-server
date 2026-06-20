# Brooks-Lint Review

**Mode:** PR Review
**Scope:** plans/native-label-selects.md (plan for the native project/context selects feature)
**Health Score:** 98/100
**Trend:** 98 → 98 (Stable at 98) over last runs (PR Review mode)

A net-deletion frontend plan that replaces two custom popover/modal widgets with
native single-select dropdowns matching Priority; well-scoped and risk-reducing,
with only two minor, constraint-bound suggestions.


## Findings

### 🟢 Suggestion

**Knowledge Duplication — placeholder label string lives in both HTML and JS**
Symptom: The word "Projects"/"Contexts" appears as static `<label>` text in
`web/index.html` and again as the placeholder `<option>` text via
`KINDS[kind].plural` in `populateLabelSelect` (`web/app.js`). The same display
decision is expressed in two files.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY: Don't Repeat Yourself.
Consequence: A future rename of the field caption must be made in two places or
the label and its placeholder drift apart.
Remedy: Accept as constraint-bound — this is exactly how the existing Priority
control already works (`<label>Priority</label>` plus `Option("Priority", "")`),
and the feature's stated goal is to be "same as Priority." Diverging here to
de-duplicate would break the very consistency the feature exists to create. The
duplication is a single short display string per kind, not logic, so drift risk
is trivial. No plan change.

**Coverage Illusion — frontend logic ships without an automated test**
Symptom: `composeText`, `populateLabelSelect`, and the reset/load flow are new or
changed client logic with no automated test in the diff.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: A regression in line composition or option population would not be
caught by CI.
Remedy: Accept as constraint-bound — the repo has no JS test harness by the
deliberate no-build-step decision (the test suite is Rust API/unit tests, which
this change does not touch). The plan keeps `composeText` pure (reads control
values, returns a string) so it is ready to test the moment a harness exists. The
change is a net deletion of hand-rolled popover/modal/focus code, lowering overall
behavioral surface rather than expanding it. Same posture as the prior two
features. No plan change.


## Summary

The plan is sound: it removes ~120 lines of bespoke widget/modal/focus logic in
favor of native `<select>` controls, keeps the change strictly frontend (no
API/Rust/store impact, verified via `include_str!`), and explicitly bounds the
single-select restriction as picker-only so no task loses label data. Both
findings are minor Suggestions accepted as constraint-bound (consistency with the
existing Priority control, and the no-JS-harness repo decision); neither
introduces decay, so the plan clears the 95 gate at 98/100 with no required
changes.

