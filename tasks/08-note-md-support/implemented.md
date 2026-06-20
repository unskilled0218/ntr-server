# Implemented: Note.md support, side by side with todo.txt (branch `note-md-support`)

Implements `plans/note-md-support.md` (brooks-review 100/100). Adds one userless,
free-form Markdown note (`Note.md`) beside the todo.txt: persisted server-side with
the same atomic write-then-swap discipline as the task store, exposed as
`GET`/`PUT /api/note`, and shown in a two-pane WebUI (`note | todo`).

## Changes

### Backend

- `src/atomicfile.rs` (new) — `atomicfile::write(path, data)`: the temp-file +
  rename atomic write, extracted from `store.rs` so the task and note stores share
  one durability routine instead of duplicating it. Leaf module (no deps), so the
  note store calling it creates no note→task-store coupling.
- `src/note.rs` (new) — `NoteStore { load, content, replace }`: caches the file in
  memory, treats a missing file as empty, and `replace` persists via
  `atomicfile::write` before swapping into memory (write-then-swap, mirroring
  `Store`). Three unit tests: missing-file-is-empty, replace-persists-and-reloads,
  replace-overwrites.
- `src/store.rs` — `write_file` now delegates to `atomicfile::write(path,
  &render(entries))`; the inline temp/rename is gone. Behavior unchanged.
- `src/lib.rs` — declares `atomicfile` and `note` modules.
- `src/config.rs` — `note_file: PathBuf` field (env `NOTE_FILE`, default `Note.md`),
  default, and env read, following the existing `file`/`TODO_FILE` pattern.
- `src/api.rs` — `AppState` gains `note: Arc<RwLock<NoteStore>>` (and the
  `AppState::new` arg); `GET`/`PUT /api/note` routes next to `/api/raw`; `get_note`
  returns `text/markdown; charset=utf-8`, `put_note` replaces from a text body
  (204); `From<io::Error> for AppError` (→ `Internal`) lets `put_note` use `?`,
  keeping one rule (an I/O failure is a 500, the same path as `StoreError::Io`).
- `src/main.rs` — loads the `NoteStore` and passes it to `AppState`; the startup
  log line now shows both the todo and note paths.

### Frontend

- `web/index.html` — page wrapped in `.panes`; left `.note-pane` (a `<textarea>` +
  Save button + status line), right `.todo-pane` holding the existing add form,
  filters, and list verbatim. The `<dialog>` moved to a direct child of `<main>`.
- `web/app.js`
  - `throwIfNotOk(res)` extracted from `api()` (shared not-ok `{error}` extraction);
    `api()` now delegates to it. Text-bodied `fetchNote`/`putNote` reuse it too.
  - `refreshNote(cleanStatus)` derives the Save button's `disabled`, the status
    text, and clean-vs-dirty from the single `value !== savedNote` check; all three
    callers (load, input, save) go through it (single source — the plan's review fix).
  - `setNoteStatus`, `loadNote`, `saveNote`; note els (`noteInput`/`noteSave`/
    `noteStatus`); `input`/`click` wiring and `loadNote()` at startup. No existing
    task-list, add-form, filter, sort, or `loadLabels` logic touched.
- `web/style.css` — `main` widened to `1100px`; `.panes` flex layout with a
  `@media (max-width: 720px)` stack; `.note-input` (full-width monospace textarea),
  `.note-bar`, `.note-status`(`.is-error`); the accent-button rule extended to
  `.note-bar button` with a muted `:disabled` state.

### Docs

- `README.md` — `NOTE_FILE` config row, `/api/note` API rows, a "Note (Note.md)"
  section, and a rewritten WebUI intro describing the two-pane note/todo layout.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` — all green:
  17 unit tests (incl. 3 new `NoteStore`) + 8 API tests (incl. new
  `note_get_and_replace`). Web assets revalidated via `include_str!`.
- Manual smoke test against `cargo run` (temp todo/note files): `/api/health` ok;
  `GET /api/note` returns `text/markdown` and is empty by default; `PUT /api/note`
  returns 204 and writes the exact text to disk; a re-GET returns it; served `/`
  contains `.panes` and `/static/app.js` contains `loadNote`.
- No JS test harness (deliberate no-build-step decision, consistent with prior
  WebUI features); note pane dirty tracking / load / save verified per plan §12.
