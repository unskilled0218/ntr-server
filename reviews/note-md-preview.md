# Brooks-Lint Review

**Mode:** PR Review
**Scope:** `plans/note-md-preview.md` (design plan for the Note.md Markdown-preview feature)
**Health Score:** 100/100 (99/100 first pass, one Suggestion adopted)

A well-decomposed, security-first renderer plan that stays inside the repo's no-build
constraint; the one first-pass Suggestion (list/fence line-patterns written in several
places) was folded back in.

---

## Findings

### 🟢 Suggestion — adopted into the plan (first pass, now resolved)

**Knowledge Duplication — the "what marks a list / fence line" patterns were repeated**
Symptom: The list-item regex `/^\s*([-*+]|\d+\.)\s+/` appeared in both `blockKind` (to
classify) and `takeList` (to strip the marker); the ordered-item regex
`/^\s*\d+\.\s+/` was written twice inside `takeList` (the `ordered` flag and the loop
condition); and the fence pattern `/^```/` appeared in both `blockKind` and `takeFence`.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY: Don't Repeat Yourself (the
*decision* "what syntax marks a list/fence", not the characters, is the duplicated
knowledge); Fowler — Refactoring, Duplicate Code.
Consequence: Extending the marker syntax (e.g. accepting `•` bullets, or `)` ordered
delimiters) would require finding and changing the same pattern in three to four places;
a missed copy means `blockKind` and `takeList` disagree about what a list is, producing
mis-parsed output that the absent JS test harness would not catch.
Remedy: Hoist the three line-patterns to named module constants
(`LIST_ITEM`, `ORDERED_ITEM`, `FENCE`) and reference them everywhere — one home for each
"what marks this block" decision, mirroring how `blockKind` is already the single source
for *classification*. (The constants carry no `g` flag, so reusing them with `.test`
across the loop is stateless and safe.) **Adopted:** sections 3.3–3.4 now define and use
the three constants.

---

## Remaining (100/100)

No further findings. Considered and dismissed:

- **Cognitive Overload — a hand-written Markdown parser.** Decomposed into small,
  single-purpose pure functions (`escapeHtml`, `safeHref`, `renderInline`, `formatText`,
  `blockKind`, the `TAKE` table, and per-block `takeX` consumers); no function exceeds
  ~10 lines or nests past two levels, and the main loop is a flat table dispatch. Below
  the R1 thresholds. No finding.
- **Accidental Complexity — the `TAKE` dispatch table.** Each entry has a real caller
  (it replaces a switch and keeps `renderMarkdown` flat); not speculative generality. No
  finding.
- **Dependency Disorder / Domain Distortion — where rendering lives.** Section 9 keeps
  rendering client-side so the server stays the canonical *raw* store, preserving the
  existing "raw store vs. presentation" separation; no new endpoint, dependency, or tier
  split. Conceptually consistent. No finding.
- **Security (XSS).** Not a decay-risk code per se, but reviewed: every user run is
  `escapeHtml`-escaped before becoming HTML, link targets pass `safeHref` (javascript:
  → `#`), links carry `rel=noopener`. Safe by construction. No finding.
- **Coverage Illusion — the renderer ships without an automated test.** This is the
  weakest spot (a pure parser is highly testable in principle), but the repo has no JS
  test harness by its deliberate no-build decision, and adding one would itself break the
  constraint the whole feature is designed to respect (section 9). Mitigated by the pure
  structure, the single `blockKind`/marker-constant sources, and the per-construct manual
  checklist (section 7). Constraint-bound, consistent with every prior WebUI feature —
  not a deduction.

## Summary

The plan is ready to implement. The single actionable finding — list/fence line-patterns
duplicated across the classifier and the block consumers — is resolved by hoisting them
to three named constants, giving each "what marks this block" decision one home. Score
100/100 after that change.
