# Plan: implement the code-review fix (R3 — name the split-mode predicate)

One 🟢 finding from `code-review.md`: the predicate `view.input && view.preview` (a mode
shows both views ⇒ it is the split/Live view) is written in two places — the `is-split`
class toggle in `setNoteMode` and the live-re-render gate in the `input` handler.

## Change

In `web/app.js`, add one named predicate beside `NOTE_MODES` and call it from both sites:

```js
// A mode that shows both the editor and the preview is the split (Live) view — both the
// side-by-side layout and the live re-render-on-type key off this one predicate.
const isSplit = (view) => view.input && view.preview;
```

- `setNoteMode`: `els.noteViews.classList.toggle("is-split", isSplit(view));`
- `input` handler: `if (isSplit(NOTE_MODES[noteMode])) renderNotePreview();`

No behaviour change — it is a pure rename of an inline expression to a named one, so the
split-mode decision has a single home alongside the descriptor table.

## Scope / non-goals

- `web/app.js` only. No HTML/CSS/README change; no renderer change; no new `els`.
- Nothing else from the review needs action (all other risks were clean; the
  no-JS-test-harness gap is constraint-bound and accepted).

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` green (`src/` untouched,
  regression guard; `cargo build` re-embeds the asset).
- Runtime smoke: served `app.js` carries `isSplit`; Live still splits and live-renders;
  `PUT`/`GET /api/note` round-trips.

## brooks-review outcome

Trivial single-call-site-to-named-predicate extraction with no new branching or
abstraction; reviewed at 100/100 (no R1–R6 findings — it strictly reduces R3). Re-review
of the implemented change confirmed 100/100 (`code-review.md`, pass 2).
