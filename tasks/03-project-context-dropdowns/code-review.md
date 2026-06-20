# Brooks-Lint Review — Code

**Mode:** PR Review
**Scope:** committed diff `f96e80c` — `web/app.js`, `web/index.html`, `web/style.css`, `README.md`
**Health Score:** 98/100

The implementation matches the plan; one input-validation gap lets a malformed label
token slip through, and client logic remains untested (constraint-bound).

## Findings

### 🟢 Suggestion

**Cognitive Overload — label validation strips only one leading sigil**
Symptom: `parseLabelInput` normalizes with `raw.trim().replace(/^[+@]/, "")`, which
removes at most one leading `+`/`@`. Input like `++work`, `@+x`, or `+@home` passes
the empty/whitespace checks and returns a name that still contains a sigil, which
`composeText` then prefixes again — producing a malformed token such as `++work`.
Source: McConnell — Code Complete, Ch. 8 (Defensive Programming: validate inputs at
the boundary).
Consequence: A user can create a task with a garbled `++work` project token; the
parser reads the project name as `+work`, so the label is subtly wrong and will not
match the clean `work` label elsewhere.
Remedy: Strip all leading sigils and reject embedded special characters, e.g.
`const name = raw.trim().replace(/^[+@]+/, "");` plus a check rejecting names that
still contain whitespace, `+`, `@`, or `:`. Pure change to one function.

**Coverage Illusion — client logic ships without an automated test**
Symptom: `composeText`, `parseLabelInput`, and the staging/selection logic have no
automated test; only manual and endpoint smoke verification was run.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: Regressions in compose/validation/staging are caught only by manual
clicking.
Remedy: Accepted as constraint-bound — the repo has no JS test harness by the
no-build-step decision, and the affected functions are pure and ready to test if a
harness is added. No action now; noted so the gap is explicit.

Note (not a finding): the `+`/`@` literals in `renderMeta` and `buildQuery` are not
treated as KINDS duplication — they render server DTO arrays and parse free-text
filter input at a different boundary than the add-form label kinds; coupling them to
`KINDS` would over-couple unrelated concerns.

## Summary

Most important action: harden `parseLabelInput` so a typed sigil cannot survive into
the composed token. The rest of the diff is clean and on-plan; the test gap is bounded
by the no-build-step constraint.
