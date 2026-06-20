# Brooks-Lint Review

**Mode:** PR Review
**Scope:** code — branch changes vs main (commit 3d86405): web/app.js, web/index.html, web/style.css, README.md
**Health Score:** 97/100
**Trend:** 100 → 97 (−3) vs last code review (task 12)

A faithful implementation of the plan with the character-preservation invariant verified; three small consistency/DRY findings where the new code drifts from idioms the codebase already established.

---

## Findings

### 🟢 Suggestion

**R4 Accidental Complexity — `highlightLine` reintroduces the if-ladder the prior renderer replaced with a dispatch table**
Symptom: `highlightLine` (web/app.js:619) is a 7-branch `if (kind === "blank") … if (kind === "code") …` ladder. The renderer it replaced (task 10) deliberately used a `TAKE` table keyed by `blockKind` "so renderMarkdown stays a flat dispatch" — the same keyed-dispatch idiom tasks 11–12 used for `NOTE_MODES`. The new code abandons that established pattern.
Source: Fowler — Refactoring, "Replace Conditional with … " / Switch Statements; Brooks — The Mythical Man-Month, Ch. 4 Conceptual Integrity (consistency with existing idioms).
Consequence: Adding or renaming a line kind now means editing a conditional chain instead of a table entry, and a reader who learned the `TAKE`/`NOTE_MODES` pattern elsewhere in this file meets a different shape for the same job — small friction that compounds as the file's dispatch sites diverge.
Remedy: Key a `LINE` table `{ blank, code, hr, heading, quote, list, para }` of `(line) => html` renderers and have `highlightMarkdown` call `LINE[kind](line)`, mirroring `TAKE`. The kinds `highlightMarkdown` produces are exactly those seven, so the table is total.

**R3 Knowledge Duplication — `formatInline`'s four emphasis branches repeat the same wrap shape**
Symptom: In `formatInline` (web/app.js:597–600) the bold (`**`, `__`) and italic (`*`, `_`) replacements are four structurally identical callbacks: `(_, t) => ${mark(M)}<span class="${C}">${t}</span>${mark(M)}`, differing only in the marker string and class.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY; Fowler — Refactoring, Duplicate Code.
Consequence: A change to how an emphasis run is wrapped (e.g. adding a title attribute or a data hook) has to be made in four places; one missed copy drifts silently. The shared decision has no single home.
Remedy: Factor `const emph = (marker, cls) => (_, t) => \`${mark(marker)}<span class="${cls}">${t}</span>${mark(marker)}\`;` and pass `emph("**","md-strong")`, `emph("__","md-strong")`, `emph("*","md-em")`, `emph("_","md-em")`. The link branch stays as-is (different shape).

**R4 Accidental Complexity — redundant `line.startsWith(">")` guard in `blockKind`**
Symptom: `blockKind` (web/app.js:579) reads `if (QUOTE_MARKER.test(line) && line.startsWith(">"))`. `QUOTE_MARKER` is `/^(>\s?)(.*)$/`, anchored at `^>`, so `.test(line)` is already true only when the line starts with `>`. The `&& line.startsWith(">")` can never change the result.
Source: Fowler — Refactoring, Dead Code; McConnell — Code Complete, Ch. 19 (no redundant conditions).
Consequence: A reader pauses to work out what case the extra guard covers (none), and a future edit to `QUOTE_MARKER` might "fix" the guard rather than the regex. Minor, but it is a small false signal in the one classifier the highlighter depends on.
Remedy: Drop `&& line.startsWith(">")`; the `QUOTE_MARKER.test(line)` test is sufficient.

---

## Summary

The implementation matches the plan and the core is sound: a Node round-trip of the pure highlighter confirms textContent equals the source for 12 varied inputs (the invariant saving and caret math depend on), and the XSS check is clean. The three findings are all low-severity consistency/DRY drift from idioms already in this file — restore the keyed dispatch table for `highlightLine` (matching `TAKE`/`NOTE_MODES`), factor the repeated emphasis wrap in `formatInline`, and drop a redundant `blockKind` guard. None affects behaviour or the verified invariant. Per REVIEW.md these are now addressed in a fix pass (review-plan.md → review-implemented.md). 97/100.
