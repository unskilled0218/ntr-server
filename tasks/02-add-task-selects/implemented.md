# Implemented: structured add controls (branch `add-task-selects`)

Frontend-only feature. Adds select-style controls under the task input so
priority, due date, and project/context labels can be set by picking options
instead of typing todo.txt syntax. No server, API, or `Cargo.toml` change.

## What changed

- `web/index.html`
  - The add form is now two rows inside `#add-form`: `.add-row` (the existing
    text input + Add button) and `.add-fields` (the new controls).
  - Three labelled controls: a Priority `<select>`, a Due `<input type="date">`,
    and a multi-select labelled "Projects & contexts" (`#add-tags`).
  - The placeholder no longer advertises raw todo.txt syntax for the structured
    fields.

- `web/app.js`
  - `els` cache extended with `priority`, `due`, `tags`.
  - `initPriorityOptions()` — fills the priority select with a blank
    placeholder then `A`..`Z` (generated, not hand-written in HTML).
  - `loadTagOptions()` — fetches the unfiltered task list, collects distinct
    `+project`/`@context` tokens, sorts them, and rebuilds the multi-select
    while preserving any still-valid selection. Best-effort: a fetch failure
    surfaces via `showError` and leaves the form usable.
  - `selectedTagTokens()` — selected option values.
  - `composeText()` — pure function building the todo.txt line: priority first
    (spec requirement), then description, then selected tokens, then `due:`.
  - `resetAddForm()` — clears all controls after a successful add.
  - Submit handler now requires a non-empty description, submits
    `composeText()`, resets the form, refreshes, and reloads tag options.
  - `editTask` also reloads tag options after a successful edit (edits can
    introduce a new label). Init runs `initPriorityOptions()` then
    `loadTagOptions()`.

- `web/style.css`
  - `#add-form` is now a column; `.add-row` keeps the input+button row;
    `.add-fields` is a wrapping row of labelled controls styled with the
    existing muted/border variables. No new color tokens.

- `README.md` — added a short "WebUI" section describing the structured add
  controls and the empty-until-first-label behavior.

## Decisions (from plan + review)

- "Date" maps to the `due:` tag; the creation date is auto-stamped by the
  server, so it is not exposed.
- The label control is named "Projects & contexts" (not "Tags") to match
  todo.txt vocabulary (review Suggestion, R6).
- `composeText` is kept pure/side-effect-free so it stays testable if a
  frontend harness is ever added (review Suggestion, Coverage Illusion).
- Tag options come from one unfiltered list fetch, reloaded only after add/edit
  (the mutations that can introduce a label) — off the toggle/delete path.

## Verification

- `cargo fmt --check` clean; `cargo clippy --all-targets` clean.
- `cargo test`: 14 unit + 7 integration tests pass (backend unchanged; the
  build re-embeds the web assets via `include_str!`).
- Manual/runtime smoke test: served `/static/app.js` and `/` contain the new
  controls; `POST /api/tasks` with a `composeText`-shaped line
  (`(A) call bank +work @desk due:2026-06-30`) parses to priority `A`,
  `due:2026-06-30`, project `work`, context `desk`.
- No automated frontend tests exist (no JS harness; out of scope by the
  no-build-step constraint).
