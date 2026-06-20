# Brooks-Lint Review

**Mode:** PR Review
**Scope:** tasks/12-note-live-only/plan.md (implementation plan — collapse note view to Live-only)
**Health Score:** 99/100 (first pass) → 100/100 (after adopting the R4 suggestion — see Re-review)
**Trend:** 100 → 99 (−1) vs last plan review (task 11 re-review)

A clean net-deletion plan that removes the mode layer faithfully and addresses load/keystroke render, dangling references, and the narrow-screen layout; one small dead-structure leftover is the only smell.

---

## Findings

### 🟢 Suggestion

**R4 Accidental Complexity — `.note-head` becomes a single-child flex wrapper**
Symptom: Section 4 leaves `<div class="note-head"><h1>note</h1></div>` in the markup and keeps `.note-head { display: flex; justify-content: space-between; align-items: baseline; gap: .75rem; }` (style.css:53). After the tab group is deleted, the flex container wraps a lone `<h1>` — `justify-content: space-between` and `gap` now govern a single child and have no visible effect. The plan acknowledges this (section 4) and keeps it to minimize the diff.
Source: A Philosophy of Software Design — accidental complexity / structure that no longer earns its place; Refactoring — Lazy Element.
Consequence: A future reader sees a flex header and assumes it still positions two things, then has to diff history to learn the second child was removed. Low cost, but it is exactly the kind of "why is this here?" residue that accumulates into noise across many such simplifications.
Remedy: Optional and safe to defer — either inline the title (drop the wrapper, let `<h1>` sit directly in the pane) or delete the `.note-head` rule and keep a plain `<div>`. If kept for minimal-diff reasons (a legitimate call), add a one-line comment on the rule noting it now holds only the title, so the intent is explicit rather than inferred. Not blocking.

---

## Summary

The plan is a faithful simplification: it deletes the `noteMode`/`NOTE_MODES`/`isSplit`/`setNoteMode` layer and the tab markup, promotes the `is-split` layout to the base rule, and correctly closes the two gaps a deletion of this kind creates — render-on-load (so the preview is populated before the first keystroke) and a grep sweep for dangling references to the removed elements. The renderer, autosave, store, and API stay untouched, and the per-keystroke-render and XSS tradeoffs carry over verbatim from task 11. The only smell is the now-vestigial `.note-head` flex wrapper, a 🟢 the plan already flags and consciously defers. Score 99/100 — over threshold; proceed to implementation.

---

## Re-review (after adopting the R4 suggestion)

**Health Score:** 100/100

The plan now removes the `.note-head` wrapper outright (section 4: the `<h1>` sits directly in the pane) and deletes the `.note-head` CSS rule (section 5), so no single-child flex residue survives — the R4 finding is resolved at the source rather than deferred. Re-scanning all six decay risks against the revised plan: Change Propagation (one feature, `src/` untouched), Cognitive Overload (net deletion of the mode branching), Knowledge Duplication (both render call sites delegate to the one `renderNotePreview` helper), Accidental Complexity (the vestigial wrapper is now gone), Dependency Disorder (no new imports), and Domain Model Distortion (naming unchanged) are all clean. No new findings. The only standing item is the documented, constraint-bound absence of a JS test harness — unchanged from tasks 10–11 and not a defect in this plan. Cleared for implementation.
