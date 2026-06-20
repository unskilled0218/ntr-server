# Implemented: project/context dropdowns and A-C priority (branch `project-context-dropdowns`)

Frontend-only change, as planned. No Rust/API/store/Cargo changes.

## What changed

- **Priority A-C** (`web/app.js`, `initPriorityOptions`): the picker now offers
  a placeholder plus `A`, `B`, `C` only. The `Task` parser is unchanged and
  still accepts/displays `(A)`-`(Z)`, so files and raw edits with higher
  priorities are preserved.
- **Separate project/context dropdowns** (`web/index.html`, `web/app.js`,
  `web/style.css`): the single combined `#add-tags` multi-select was replaced by
  two custom dropdown widgets (toggle button + popover panel of checkbox rows).
  Multi-select is preserved; the toggle text shows the selected count. Only one
  panel is open at a time; outside-click and `Escape` close them.
- **Add button + modal** (`web/index.html` `<dialog id="label-modal">`,
  `web/app.js`): each panel has an "+ Add project" / "+ Add context" button that
  opens a shared native `<dialog>`. A submitted name is trimmed, has any leading
  `+`/`@` stripped, and is rejected (with an inline modal error) if empty or
  containing whitespace. A valid name is staged client-side, selected, and shown
  checked immediately; it persists once a task using it is saved.
- **`KINDS` descriptor** (`web/app.js`): per-kind prefix/singular/plural live in
  one place; `setupDropdown`, the toggle/add-button text, the modal title, and
  `composeText` all read from it (the brooks-review suggestion).
- **`composeText`** now emits selected labels by iterating `KINDS`
  (`+name`/`@name`); priority still leads the line.
- **`README.md`**: WebUI section updated (A-C picker, separate dropdowns,
  add-via-modal, staged-until-used note, and the parser-not-narrowed note).

## Files touched

- `web/index.html` — two `.dropdown` widgets + shared `<dialog>`.
- `web/app.js` — `KINDS`, `labels` state, `setupDropdown`, `initDropdowns`,
  modal open/validate/confirm, `loadLabels` (replacing `loadTagOptions`),
  `composeText`, reset/submit wiring, A-C priority.
- `web/style.css` — `.dropdown*` and `dialog#label-modal` / `::backdrop` /
  `.modal-*` styles; removed the old `.add-tags-field` rules.
- `README.md` — WebUI paragraph.
- `plans/`, `reviews/`, `implemented/` per `agents/AGENTS.md`.

## Verification

- `cargo build`, `cargo test` (7 API + unit tests), `cargo clippy --all-targets`,
  `cargo fmt --check` all green (backend unchanged).
- Asset/endpoint smoke test against a running binary: `/` serves both
  `.dropdown` widgets and the `#label-modal` dialog; `/static/app.js` contains
  `KINDS`/`setupDropdown`; `/static/style.css` contains the dropdown styles; a
  `POST /api/tasks` with `(C) ship it +rust @home` round-trips to
  `(C) 2026-06-19 ship it +rust @home` with `projects=["rust"]`,
  `contexts=["home"]`.
- No JS test harness in the repo (no-build-step constraint); `composeText` and
  `parseLabelInput` are kept pure for future testability.
