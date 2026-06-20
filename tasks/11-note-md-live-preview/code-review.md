# brooks-review: code (branch `note-md-live-preview` vs main)

Scope: web/app.js, web/index.html, web/style.css, README.md (~75 lines). First pass
99/100 with one 🟢 R3 suggestion; planned + fixed; re-review 100/100.

## Pass 1 — 99/100

**🟢 Suggestion — Knowledge Duplication (R3): the "split/Live mode" predicate written twice**
- Symptom: `view.input && view.preview` appears in both `setNoteMode` (the `is-split`
  class toggle) and the `input` handler (the live-re-render gate) — the same derived fact
  "this mode shows both views, i.e. Live/split."
- Source: Hunt & Thomas — The Pragmatic Programmer (DRY).
- Consequence: "what is the split mode" lives in two expressions; a future change to that
  meaning could update one and leave the other behind, desyncing the layout from the
  render gate — the same risk the `NOTE_MODES` table removed for the mode set.
- Remedy: name the predicate once (`const isSplit = (view) => view.input && view.preview;`)
  and call it from both sites.

Other risks clean: R1 (functions ≤ ~10 lines, nesting ≤ 2, descriptive names); R2
(additive, single pane); R4 (no speculative generality, renderer reused verbatim); R5/R6
(no new deps, domain-faithful naming). No-JS-test-harness gap constraint-bound, accepted
(renderer unchanged, `src/` untouched, Rust suite green) — not scored.

## Adopted

Added `const isSplit = (view) => view.input && view.preview;` beside `NOTE_MODES`;
`setNoteMode`'s `is-split` toggle and the `input`-handler render gate both call it. The
predicate now has one home.

## Pass 2 — 100/100 (re-review)

No findings. The split-mode decision is expressed once; everything else from pass 1 was
already clean. `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` green;
runtime smoke (served `isSplit`, Live tab, note round-trip) confirmed.

**Result: 100/100.**
