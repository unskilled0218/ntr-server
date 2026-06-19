# Brooks-Lint Review

**Mode:** PR Review
**Scope:** plans/add-task-selects.md (design plan for structured add controls)
**Health Score:** 98/100

A tightly scoped, frontend-only plan that keeps the API contract untouched and centralizes line construction; the only open items are minor naming/vocabulary and the (constraint-bound) absence of a test for the new compose logic.

---

## Findings

### Green Suggestion

**Domain Model Distortion — "Tags" control flattens two distinct todo.txt concepts**
Symptom: Section 1/3 group `+project` and `@context` labels under a single control named "Tags", while the actual `key:value` tag the plan does handle (`due:`) lives in a separate date control. todo.txt treats projects, contexts, and key:value tags as three distinct concepts.
Source: Evans — Domain-Driven Design (Ubiquitous Language); Ousterhout — A Philosophy of Software Design (naming that reflects the domain).
Consequence: A user familiar with todo.txt may misread the control, and a future need to filter or style projects vs contexts separately starts from a label that has already merged them. Low impact — it is a UI label, not a data model change; the description field still records them verbatim.
Remedy: Label the control "Projects & contexts" (or keep two small selects). Cheap to adjust at implementation time; flagged so the wording is a deliberate choice.

**Coverage Illusion — New compose logic has no automated test**
Symptom: Section 9 states the new `composeText`/tag-loading logic is verified only manually; the repo's test suite is Rust API/unit tests that this feature does not touch.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: A regression in line composition (e.g., priority not emitted first, or a missing space) would not be caught by `cargo test`. Mitigated by the small surface area and the no-build-step constraint, which rules out a JS test harness for v1.
Remedy: Accept for v1 given the constraint, but keep `composeText` a pure, side-effect-free function so it stays trivially testable if a frontend harness is ever added. State that intent in the plan.

---

## Step 7 — Quick Test Check

Frontend/docs change with no production (Rust) logic touched and no test files in scope; the backend suite is unaffected. The only test-relevant note is the Coverage Illusion suggestion above (new client logic is manually verified). No mock or obscurity signals apply.

---

## Summary

The plan resolves the obvious decay traps up front: no API/contract change (no Hyrum's-Law surface added), a single `composeText` source of truth (no duplication), tag tokens carrying their own `+`/`@` prefix (no re-prefixing logic), and the one extra fetch explicitly bounded and documented so it is not an accidental N+1. The two residual items are a UI naming choice and a constraint-bound test gap, both Suggestions. Score 98/100 — over the 95 gate; cleared to implement.

---

# Brooks-Lint Review — Code (REVIEW.md step 1)

**Mode:** PR Review
**Scope:** branch changes vs main (`main...HEAD`) — web/app.js, web/index.html, web/style.css (docs excluded from logic review)
**Health Score:** 98/100

The implementation matches the approved plan: a single pure `composeText`, tag tokens carrying their own prefix (no re-prefixing), and no API change. Two minor Suggestions remain.

## Findings

### Green Suggestion

**Cognitive Overload — ASCII literals for the A..Z priority range**
Symptom: `initPriorityOptions()` loops `for (let code = 65; code <= 90; ...)` using the raw char codes for `A` and `Z`.
Source: McConnell — Code Complete, Ch. 12 (avoid magic numbers); Hunt & Thomas — The Pragmatic Programmer (DRY/clarity).
Consequence: A reader must recognize 65/90 as ASCII `A`/`Z`; a typo in either bound silently shifts the option set. Low impact, isolated to one short loop.
Remedy: Derive the bounds from the characters themselves — `"A".charCodeAt(0)` / `"Z".charCodeAt(0)` — so the intent is self-evident.

**Coverage Illusion — Client compose logic has no automated test**
Symptom: `composeText` and `loadTagOptions` add new behavior with no test changes in the diff.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: A composition regression (priority not leading, missing space, wrong tag prefix) would not be caught by `cargo test`. Mitigated: `composeText` is pure and small, and the repo has no JS test harness by the deliberate no-build-step constraint. Consistent with the plan-review classification of this item as a Suggestion.
Remedy: Accept for v1; revisit if a frontend harness is introduced. `composeText` is already pure, so it is ready to test in place.

## Step 7 — Quick Test Check

New behavior is client-side JS; the backend (Rust) suite is unchanged and still green (14 unit + 7 integration). No JS test harness exists. Signal 1 surfaces the Coverage Illusion suggestion above; no mock/obscurity signals apply.

## Summary

No correctness, propagation, or duplication issues: line construction is centralized in one pure function, tag options derive from the API's existing `projects`/`contexts` arrays (no client-side parsing), and the contract is untouched. The magic-number nit will be fixed; the test-coverage item is accepted as constraint-bound. Score 98/100 — over the 95 gate.
