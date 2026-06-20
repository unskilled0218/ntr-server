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

---

# Code Review (committed implementation, `web/*` @ commit aa7ed4c)

**Mode:** PR Review
**Scope:** `web/app.js`, `web/index.html`, `web/style.css` (the preview feature as built)
**Health Score:** 100/100 (99/100 first pass, one Suggestion fixed)

The implementation matches the 100/100 plan; reviewing it as shipped code caught one
marker pattern the plan's R3 fix had missed — the blockquote prefix — now also a
constant.

## Findings

### 🟢 Suggestion — fixed in this branch

**Knowledge Duplication — the blockquote marker `/^>\s?/` was still written twice**
Symptom: The plan's marker-constant extraction named `LIST_ITEM`, `ORDERED_ITEM`, and
`FENCE`, but the blockquote pattern `/^>\s?/` remained inline in both `blockKind` (to
classify a quote line) and `takeBlockquote` (to strip the `>` prefix) — the exact
classify-here / strip-there duplication the constants were introduced to remove.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY (the "what marks a blockquote line"
decision in two places); Fowler — Refactoring, Duplicate Code.
Consequence: Changing what counts as a quote line (e.g. requiring the space after `>`, or
allowing a leading indent) would need editing both copies; a missed copy desyncs the
classifier from the stripper, mis-rendering quotes — which the absent JS harness would
not catch.
Remedy: Add a `QUOTE_MARKER = /^>\s?/` constant and reference it from both sites, so the
blockquote marker joins the other three with a single home. **Fixed.**

## Remaining (100/100)

No further findings. Considered and dismissed:

- **The heading patterns `/^#{1,6}\s/` (classify) vs `/^(#{1,6})\s+(.*)$/` (capture).**
  Not the same expression — one detects, one parses with capture groups — so this is a
  detect-vs-parse pair, not duplicated knowledge (the same call made in the plan review).
  No finding.
- **Correctness — a link URL containing `)` is truncated** (`[x](https://e/Foo_(bar))`
  stops the href at the first `)`). A real bounded limitation of regex link parsing, not
  a security issue (the truncated target still passes `safeHref`), and balanced-paren
  handling would add parser complexity disproportionate to a reading-view subset. The
  README documents the renderer as a practical subset. Accepted, not a deduction.
- **Cognitive Overload / Accidental Complexity / Security.** Unchanged from the plan
  review: small pure functions, flat `TAKE` dispatch, escape-then-sanitise. No finding.
- **Coverage Illusion — renderer untested by CI.** Constraint-bound (no JS engine or
  harness available; the no-build decision the feature respects). The pure structure and
  the per-construct manual trace stand in; `src/` is untouched and stays covered. Not a
  deduction.

## Summary

The built feature is sound and matches the plan. The one issue — the blockquote marker
escaping the plan's R3 constant extraction — is fixed by adding `QUOTE_MARKER`, so all
four repeated block markers now have a single home. Score 100/100 after the fix.
