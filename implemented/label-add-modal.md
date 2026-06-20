# Implemented: add-new-label option + modal (branch `label-add-modal`)

Frontend-only feature. Each of the add form's project/context native `<select>`
dropdowns now ends with an "add" entry; choosing it opens a modal with a single
text input for creating a new label. No server, API, or `Cargo.toml` change.

## What changed

- `web/index.html`
  - Added a shared `<dialog id="label-modal">` after the add form: a title, one
    name `<input>`, an inline error `<p>`, and Cancel/Add buttons. The four
    `.add-fields` controls are unchanged.

- `web/app.js`
  - `KINDS` gained a `singular` word per kind (used for the add-entry text and
    the modal title), keeping the per-kind vocabulary in one place.
  - `ADD_VALUE = "+add"` — the sentinel option's value. The sentinel is
    recognised by a `data-add` flag, **not** by this value, so a real label can
    never be mistaken for it; the value is a validator-forbidden token only as
    defence-in-depth.
  - `staged` (one `Set` per kind) holds labels created in the modal but not yet
    on any task; rendered options are `server-derived ∪ staged`.
  - `priorLabel` (per kind) remembers the current real selection so choosing the
    add entry can restore it instead of leaving the sentinel selected.
  - `populateLabelSelect` now appends the flagged add `<option>` last and keeps
    `priorLabel` in sync with the surviving selection.
  - `loadLabels` unions `staged` into the rendered names.
  - `parseLabelInput(raw)` — pure validator: strips leading `+`/`@`, rejects
    empty, whitespace, and any `+`/`@`/`:` in the bare name.
  - `onLabelSelectChange(kind)` — branches on `selectedOptions[0].dataset.add`:
    restores the prior value and opens the modal for the add entry, otherwise
    records the new selection.
  - `openLabelModal(kind)` / `confirmLabel()` — open the shared `<dialog>` and,
    on a valid name, stage + render + select the label; an invalid value shows an
    inline error and keeps the dialog open.
  - Wired the two selects' `change` events, the modal form `submit`
    (`preventDefault` + `confirmLabel`), and the Cancel button.
  - `resetAddForm` clears `priorLabel` alongside the selects.

- `web/style.css`
  - Added `dialog#label-modal`, `::backdrop`, `#label-form`, `.modal-actions`,
    and `.modal-error`, reusing the existing color variables.

- `README.md`
  - Extended the WebUI section: the add entry, the modal, and the
    staged-until-used label note.

## Design notes

- Native `<select>` popups cannot host a `<button>`, so the "add button at the
  bottom of the dropdown menu" is a sentinel `<option>` (the established
  native-select UI from commit `65ffb99` is preserved). See
  `plans/label-add-modal.md` section 2.
- Staged-but-unused labels vanish on reload, consistent with todo.txt having no
  standalone label store. See plan section 4.

## Verification

- `cargo fmt --check`, `cargo build`, `cargo test` (7 passed), and
  `cargo clippy --all-targets` are all green — the backend is untouched and the
  embedded web assets compile in.
- Ran the binary and confirmed `/`, `/static/app.js`, and `/static/style.css`
  serve the updated markup, script, and styles (sentinel, modal, and new
  functions present).
- Manual UI checks per plan section 10: add entry pinned last; choosing it opens
  the titled modal without leaving the select on the sentinel; a valid name
  stages, selects, and composes as `+name`/`@name`; empty/spaced/`+@:` input
  shows an inline error and keeps the modal open; Cancel/Escape leave the select
  unchanged.

## brooks-review

Plan reviewed in `reviews/label-add-modal.md`: 94/100 first pass (one Warning —
sentinel value collision), revised to an out-of-band `data-add` flag, 99/100 on
re-review (over the 95 gate).
