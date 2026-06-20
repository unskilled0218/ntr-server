# Implemented: Auto-save Note.md (branch `note-md-autosave`)

Built per `plans/note-md-autosave.md` (brooks-review 100/100,
`reviews/note-md-autosave.md`). The WebUI note pane now saves itself; the explicit
Save button is gone. Backend untouched — autosave is a client that calls the
existing `PUT /api/note` more often.

## What changed

- **`web/app.js`** — replaced the Save-button flow (`refreshNote`, `saveNote`, the
  button lookup/wiring) with debounced autosave:
  - `NOTE_SAVE_DELAY_MS = 800` — the one named debounce length.
  - `noteDirty()` — single definition of "textarea differs from last persisted text",
    called by all three save paths (the adopted brooks-review R3 fix).
  - `scheduleNoteSave()` / `flushNote()` — per-keystroke debounce; `flushNote`
    single-flights on `savingNote`, persists when dirty, sets the status line, and on
    completion re-checks `noteDirty()` to re-arm for edits typed during the PUT.
  - `flushNoteBeacon()` — best-effort `keepalive` PUT on `pagehide`.
  - `putNote(text, { keepalive })` — gained the optional keepalive flag (off by
    default) so the page-close flush can outlive the document.
  - `loadNote()` — now just resets the status to blank (no button to toggle).
  - Wiring: `input` → "Editing…" + `scheduleNoteSave`; `blur` → `flushNote`;
    `window` `pagehide` → `flushNoteBeacon`.
- **`web/index.html`** — removed `<button id="note-save">`; `.note-bar` keeps only the
  status span.
- **`web/style.css`** — removed the two now-dead note-button rules (`.note-bar button`
  from the accent selector list, and `.note-bar button:disabled`).
- **`README.md`** — WebUI note-pane paragraph rewritten from the Save button to
  autosave (debounce + blur + page-close, status states Editing…/Saving…/Saved/error,
  last-write-wins).

Status line lifecycle: `""` (loaded) → `Editing…` (typing) → `Saving…` (PUT) →
`Saved`, or red `Could not save note: …` on failure (retried on the next edit).

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` (8 integration tests
  incl. `note_get_and_replace`, plus the `NoteStore` unit tests) — all green. `src/`
  unchanged, so the suite is a pure regression guard.
- Manual smoke against a freshly built binary: the served page has no `note-save`
  button; `app.js` ships `flushNoteBeacon`/`NOTE_SAVE_DELAY_MS`; `style.css` no longer
  carries `.note-bar button`; `PUT /api/note` → 204 and `GET /api/note` round-trips the
  text. The in-browser debounce/blur/pagehide behaviour is verified by reading the code
  (no JS test harness — the repo's standing no-build-step constraint).

## Notes / non-goals (unchanged from the plan)

- No backend change; `PUT /api/note` was already atomic and tested.
- Task add/edit stay explicit — only the note pane autosaves.
- Still last-write-wins across tabs (same as `PUT /api/raw`); no version history, no
  conflict detection, no Markdown preview, no `Ctrl/Cmd+S`.
- `keepalive` bodies are capped (~64 KB); larger notes still save on the normal
  debounce/blur paths, the beacon only covers the last unsaved edit on close.
