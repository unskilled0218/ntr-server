# Brooks-Lint Review

**Mode:** PR Review
**Scope:** tasks/13-note-inplace-preview/plan.md (implementation plan — single contenteditable in-place live Markdown editor)
**Health Score:** 98/100 (first pass) → 100/100 (after adopting both suggestions — see Re-review)
**Trend:** 100 → 98 (−2) vs last plan review (task 12 re-review)

A well-reasoned plan for an inherently complex feature: the central caret/text invariant is stated once and every helper derives from it, and the task-10 renderer is removed rather than left dead; two coupling/readability refinements before implementation.

---

## Findings

### 🟢 Suggestion

**R3 Knowledge Duplication — the "one `\n` between lines" rule lives in three inverse functions**
Symptom: The decision that the logical text joins lines with exactly one `"\n"` is encoded in `readNoteText` (`.join("\n")`), in `getCaret` (`offset += line.textContent.length + 1`), and in `setCaret` (`pos -= len + 1`). The three are inverses that must agree; the `+ 1` is the same separator decision written twice more, with nothing naming the relationship. If the join ever changed, two arithmetic sites would silently drift.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY (a decision expressed in more than one place); Ousterhout — A Philosophy of Software Design, Information Leakage (one fact spread across modules).
Consequence: A future edit to the line model (e.g. preserving a trailing blank line, or a different separator) would have to be made in three places that don't reference each other; missing one yields off-by-one caret seating that is hard to trace because the text still reads back correctly.
Remedy: Name the invariant in one place — a short comment block on `readNoteText` stating "lines join with one `\n`; `getCaret`/`setCaret` are its inverses and must use the same `+1`" so the three sites are explicitly linked, or factor the per-line contribution (`textContent.length + 1`) behind a tiny named helper the two caret functions call. Cheap; do it while the three are being written together.

**R1 Cognitive Overload — `highlightInline` risks becoming a dense multi-pass function**
Symptom: Section 3.2 sketches `highlightInline` as a single function doing code-span splitting, then links, then bold, then italic, each re-emitting markers. Task 10 deliberately split that work into two functions (`renderInline` for the code-span split, `formatText` for emphasis/links); the plan collapses it to one, where the marker-keeping replacements are also more involved than task 10's marker-stripping ones.
Source: Fowler — Refactoring, Long Method; McConnell — Code Complete, Ch. 7 (one routine, one level of abstraction).
Consequence: The inline highlighter is the densest, most regex-heavy part of the change; as one function it mixes "separate code from non-code" with "format a non-code run," making the marker-preservation logic harder to read and hand-verify — exactly the code where a subtle bug (a dropped or doubled marker character) breaks the textContent-equals-source invariant the whole feature rests on.
Remedy: Mirror task 10's split — keep the code-span partition in `highlightInline` and put the emphasis/link wrapping in a separate `formatInline(run)` helper it maps over the non-code segments — so each function stays at one level of abstraction and the marker-keeping regexes are read in isolation. State this split in the plan so it is the intended shape, not an afterthought.

---

## Summary

The plan handles the hard parts of a single-surface contenteditable editor head-on: it pins everything to one invariant (a line's `textContent` joined by `\n` *is* the raw Markdown), which collapses saving, caret preservation, and XSS to trivial consequences; it intercepts Enter/paste and guards IME so the DOM stays canonical; and it deletes task 10's now-unused renderer instead of leaving it dead. The two 🟢s are refinements to make before coding — name the three-way newline invariant so the inverse caret functions can't drift (R3), and split the inline highlighter to keep its marker-preservation regexes readable (R1). Neither blocks; both are cheapest to fold in now. 98/100 — over threshold.

---

## Re-review (after adopting both suggestions)

**Health Score:** 100/100

The plan now (1) names the newline invariant in one place and routes `getCaret`/`setCaret` through a single per-line-length helper so the `+1` separator decision has one home, and (2) specifies the `highlightInline` (code-span partition) / `formatInline` (emphasis + links) split up front, mirroring task 10's two-function shape. Re-scanning the six risks against the revised plan: R2 (confined to the web note feature, `src/` untouched), R4 (the contenteditable complexity is essential to the chosen requirement, not speculative — simpler options were offered and declined, section 9; the old renderer is removed), R5 (no new dependencies; reuses `blockKind`/`escapeHtml`), and R6 (consistent note/line/caret/highlight vocabulary) are all clean. No further findings. The only standing item is the documented, constraint-bound absence of a JS test harness for the client editor — unchanged from tasks 10–12 and not a defect in this plan. Cleared for implementation.
