# Brooks-Lint Review

**Mode:** PR Review
**Scope:** plans/label-add-modal.md (design plan; web/* frontend feature)
**Health Score:** 99/100 (re-review after section-2 fix; first pass was 94/100)
**Trend:** 94 → 99 (+5) after one iteration

The plan is sound and ready to implement: the one Warning from the first pass —
a value-collision in the sentinel-option design — has been resolved, leaving only
a constraint-bound test suggestion.

---

## First pass (94/100)

### 🟡 Warning — resolved

**Cognitive Overload — in-band magic sentinel value can collide with a real label**
Symptom: The draft reserved `ADD_VALUE = "__add__"` as the sentinel option's
`value` and keyed `onLabelSelectChange` off `value === ADD_VALUE`, arguing
collision was impossible. That was wrong: `+__add__` is a valid todo.txt project
(underscores allowed; no whitespace and no `+ @ :` in the bare name, so
`parseLabelInput` accepts it). Once a task used it, `populateLabelSelect` would
render a real option whose value equalled the sentinel's, so selecting that real
label would open the modal and the label would be unselectable.
Source: McConnell — Code Complete, Ch. 12 (in-band/magic values); Ousterhout —
A Philosophy of Software Design, Ch. 4 (design special cases out rather than
reason around them).
Consequence: A correctness invariant ("no real label has this value") that the
data model does not guarantee, with a confusing failure mode.
Remedy applied: Section 2 now recognises the sentinel out-of-band via a
`data-add` flag — the handler branches on `selectedOptions[0].dataset.add`, not
the value — so no label name can collide. The value is additionally set to a
validator-forbidden `+add` as defence-in-depth, and the incorrect justification
was replaced with the explicit invariant. Sections 5 and 6 and the Risks section
were updated to match.

## Remaining (99/100)

### 🟢 Suggestion

**Coverage Illusion — new client logic ships without an automated test**
Symptom: `parseLabelInput`, `onLabelSelectChange`, and the staging union are
verified manually only (section 10).
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: Validation/sentinel regressions are not caught mechanically.
Remedy: Accept as constraint-bound — the repo has no JS test harness by the
deliberate no-build-step decision, consistent with `add-task-selects` and
`project-context-dropdowns`. `parseLabelInput` is kept pure (section 7) so it is
ready to test if a harness is added. No change required; documented to avoid a
false "incomplete" reading.

---

## Summary

After one iteration the plan clears the 95 gate at 99/100. The sentinel design is
now collision-proof by construction (out-of-band `data-add` flag), and the rest of
the plan — frontend-only scope, the documented staging trade-off, the shared
`<dialog>`, and the single `KINDS` source of truth — is consistent with the
established pattern. Proceed to implementation.
