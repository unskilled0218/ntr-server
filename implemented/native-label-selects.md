# Implemented: native project/context selects (branch `native-label-selects`)

Implements `plans/native-label-selects.md`. The add form's Projects and Contexts
controls are now native single-select `<select>` dropdowns matching the Priority
control; the custom popover widgets and the add-label modal were removed. Net
deletion of code. Frontend-only — no Rust, API, or store change.

## What changed

### `web/index.html`
- Replaced the two `.dropdown` widgets (toggle button + checkbox panel + add
  button) with two `<label>`-wrapped selects, `<select id="add-project">` and
  `<select id="add-context">`, modeled on the existing Priority `<label>`.
- Deleted the `<dialog id="label-modal">` block.

### `web/app.js`
- `KINDS` trimmed to `{ prefix, plural }` per kind (`singular` fed only the
  removed modal title).
- Removed the `labels` registry (`known`/`selected`/`staged` sets), the
  `dropdowns` registry, `setupDropdown`, `closeAllDropdowns`, `initDropdowns`,
  the document-level click/`Escape` panel listeners, and the modal functions and
  state (`openLabelModal`, `parseLabelInput`, `confirmLabel`, `modalKind`) and the
  modal `els` references and event wiring.
- Added `els.project` / `els.context`, keyed by kind name so the loops read
  `els[kind]`.
- Added `populateLabelSelect(kind, names)`: rebuilds the option list as a blank
  placeholder (caption text, like Priority) plus the sorted names, preserving the
  current pick only while it still exists.
- Simplified `loadLabels` to collect the server label sets (unchanged source: one
  `GET /api/tasks`) and call `populateLabelSelect` per kind.
- `composeText` now reads `els[kind].value` (one label per kind) instead of
  iterating a `selected` set; stays pure.
- `resetAddForm` resets both selects with `value = ""`.
- Bootstrap drops `initDropdowns()`.

### `web/style.css`
- Deleted the `.dropdown*` block and the `dialog#label-modal` / `::backdrop` /
  `#label-form` / `#label-input` / `.modal-error` / `.modal-actions` rules. The
  new selects reuse the existing `.add-fields select` styling — no new CSS.

### `README.md`
- Rewrote the WebUI paragraph: four alike native controls; project/context are
  single-select, populated from existing file labels, no Add modal. Documented
  that both the `A`-`C` priority limit and the one-label-each limit are UI-only —
  the data model, raw edit, and `PUT /api/raw` still accept and display any
  priority and any number of labels.

## Decisions carried from the plan
- Single-select is a picker-only restriction (mirrors `A`-`C`): the Rust
  parser/`Display` and `renderMeta` are untouched, so a task with many labels
  still loads, round-trips, and lists every label.
- No Add modal: a native `<select>` cannot host the button, and labels become
  options the moment a task carries them — consistent with todo.txt having no
  free-floating label store.

## Verification
- `cargo fmt --check` — clean.
- `cargo build` — succeeds; `include_str!` revalidates the embedded web assets.
- `cargo clippy --all-targets` — no warnings.
- `cargo test` — 7 passed, 0 failed (unchanged backend).
- `grep` confirms no orphaned `dropdown`/`modal`/`staged`/`labels[`/`singular`
  references remain in `web/*`.
- Manual checklist (plan section 9) to be run via `cargo run` against
  `http://127.0.0.1:8080`.

## brooks-review
Plan scored 98/100 (`reviews/native-label-selects.md`), over the 95 gate, with
two constraint-bound Suggestions (Knowledge Duplication mirroring the Priority
pattern; Coverage Illusion bounded by the no-JS-harness repo decision). No plan
change was required.
