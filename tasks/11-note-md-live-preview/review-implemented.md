# Implemented: code-review fix (R3 — name the split-mode predicate)

Built per `review-plan.md`, addressing the single 🟢 finding in `code-review.md`.

## What changed

- **`web/app.js`** — added `const isSplit = (view) => view.input && view.preview;` beside
  `NOTE_MODES`. The `is-split` class toggle in `setNoteMode` and the live-re-render gate
  in the `input` handler now both call `isSplit(...)` instead of repeating the
  `view.input && view.preview` expression. Pure extraction — no behaviour change.

No other file touched; renderer, HTML, CSS, README unchanged.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` (17 + 8) — all green.
  `src/` untouched; `cargo build` re-embedded the asset.
- Runtime smoke: served `app.js` carries `isSplit` (definition + two call sites); Live
  still splits and live-renders; `PUT /api/note` → 204.

## brooks-review outcome

Re-review (`code-review.md`, pass 2): 100/100, no findings — the split-mode decision now
has one home alongside `NOTE_MODES`.
