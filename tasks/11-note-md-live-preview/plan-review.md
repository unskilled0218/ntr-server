# brooks-review: plan (`tasks/11-note-md-live-preview/plan.md`)

Two passes. First pass 99/100 with one 🟢 R3 suggestion; adopted; re-review 100/100.

## Pass 1 — 99/100

**🟢 Suggestion — Knowledge Duplication (R3): "which views a mode shows" encoded twice**
- Symptom: `setNoteMode` derived `showInput = mode === "edit" || mode === "live"` and
  `showPreview = mode === "preview" || mode === "live"`, while the same mode set was also
  the keys of `NOTE_TABS` and re-tested as `mode === "live"` for the split toggle and the
  live-render gate — four sites encoding the same mode knowledge.
- Source: Hunt & Thomas — The Pragmatic Programmer (DRY); Fowler — Refactoring (Duplicate
  Code).
- Consequence: adding/renaming a mode means editing the map plus two boolean expressions
  plus the `is-split`/live tests, and a missed site silently desyncs the tab highlight
  from the visible views. Same smell class task 10 fixed with the marker constants.
- Remedy: a single `NOTE_MODES` descriptor table pairing each mode with its `tab` and its
  `input`/`preview` visibility; `setNoteMode`, the `input` handler, and the wiring all
  read from it (split = `input && preview`).

## Adopted

Plan section 3.1 replaced the `NOTE_TABS` map with the `NOTE_MODES` descriptor table
(`{ tab, input, preview }` per mode). Sections 3.3 (toggle), 3.4 (live render gate), and
3.5 (wiring) now read visibility and the tab from that one table; sections 10 and 11
updated to match.

## Pass 2 — 100/100 (re-review)

No findings. R1 clear (all functions ≤ ~10 lines, nesting ≤ 2, descriptive names); R2
additive/single-pane; R3 resolved (one table owns the mode set); R4 no speculative
generality (renderer reused verbatim, three modes = three real behaviours); R5/R6 no new
deps, domain-faithful naming. Standing constraint-bound note: no JS test harness for the
client renderer (no-build decision, as task 10) — renderer reused unchanged, server half
stays covered by the Rust suite.

**Result: 100/100 — approved for implementation.**
