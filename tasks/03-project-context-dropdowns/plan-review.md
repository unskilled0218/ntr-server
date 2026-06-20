# Brooks-Lint Review

**Mode:** PR Review
**Scope:** `plans/project-context-dropdowns.md` (design plan; reviewed for decay risks the described implementation would introduce)
**Health Score:** 98/100

A well-scoped, frontend-only plan that defends its boundaries explicitly; the only
risks are a per-kind metadata that could scatter and the (constraint-bound) absence
of client-side tests.


## Findings

### 🟢 Suggestion

**Knowledge Duplication — per-kind metadata risks being expressed in several places**
Symptom: The plan encodes "project vs context" facts in multiple spots: the `+`/`@`
prefixes in `composeText` (section 6), the `data-kind` attribute and the visible
"Projects"/"Contexts" labels and "+ Add project"/"+ Add context" button text in the
HTML (sections 3, 7), the modal title string (section 7), and the toggle text
"Select projects"/`Projects (n)` (section 3). The mapping kind → {prefix, singular,
plural} is implied in four+ locations rather than stated once.
Source: Hunt & Thomas — The Pragmatic Programmer (DRY: Don't Repeat Yourself);
Ousterhout — A Philosophy of Software Design (Ch. 5: Information Leakage).
Consequence: Adding or renaming a kind (or changing how a prefix is rendered) means
hunting through HTML and JS for every place the kind's words/prefix appear; the
copies can drift (e.g. a label says "context" while a button says "tag").
Remedy: Define one per-kind descriptor consumed everywhere, e.g.
`const KINDS = { project: { prefix: "+", singular: "project", plural: "Projects" }, context: { prefix: "@", singular: "context", plural: "Contexts" } };`
and have `setupDropdown`, `composeText`, the toggle text, and the modal title read
from it. `composeText` then emits `${KINDS[kind].prefix}${name}` in a single loop
over kinds instead of one hardcoded line per kind.

**Coverage Illusion — new client logic ships without an automated test**
Symptom: The plan adds non-trivial client behavior (selection state, label staging,
the union `known = server ∪ staged`, name validation, `composeText`) but section 10
verifies it only manually, because the repo has no JS test harness.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1 (untested behavior).
Consequence: Regressions in compose/validation/staging are caught only by manual
clicking; the staging-and-reset logic in particular is easy to break silently.
Remedy: Accepted as constraint-bound — adding a JS test harness is outside this
feature's no-build-step scope, and `composeText`/validation are kept pure so they
are ready to test if a harness is ever introduced. No action required now; noted so
the gap is explicit rather than hidden.


## Summary

The plan is sound and tightly scoped: it justifies the custom dropdown over a native
`<select>`, confines the A-C rule to the picker (not the parser), and resolves the
"where does a new label live" question with documented client-side staging that
keeps todo.txt canonical. The one actionable improvement is to centralize per-kind
metadata so prefixes and labels cannot drift across HTML and JS; the test gap is
real but bounded by the deliberate no-build-step constraint. No Critical or Warning
findings.

