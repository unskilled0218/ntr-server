# Plan: Note.md support, side by side with todo.txt (branch `note-md-support`)

Add a single free-form Markdown scratchpad (`Note.md`) that lives beside the
todo.txt file, is persisted server-side through the same durable write discipline
as the task store, is exposed over the HTTP API, and is shown in the WebUI **side
by side** with the todo list: `Note.md | todo.txt` (note on the left, the existing
todo UI on the right).

## 1. Goal and scope

Requirements (from `agents/ADD_NEW_FEATURE.md`):

> - Add Note.md support
> - Side by side
>   - Note.md | todo.txt

Interpretation (documented here, consistent with the repo's existing philosophy):

- **Note.md support** = one userless, free-form Markdown document stored in a file
  beside the todo.txt (the same way the todo list is one userless todo.txt file).
  There is no per-task note and no note structure to parse — it is a single block
  of text. It is the canonical store; the server caches it in memory and writes it
  atomically, exactly like the task store.
- **Side by side: Note.md | todo.txt** = a two-pane WebUI. The left pane is a
  Markdown editor (a `<textarea>` over the raw `.md` text with an explicit **Save**
  button and a dirty/saved status line); the right pane is the existing todo UI
  (add form, filters, list) unchanged. On narrow viewports the two panes stack.

In scope:

- `src/atomicfile.rs` (new) — the temp-file + rename atomic write, extracted from
  `store.rs` so both the task store and the note store share one durability
  routine instead of duplicating it.
- `src/note.rs` (new) — `NoteStore`: load the file (missing = empty), expose its
  content, and `replace` it under the same write-then-swap discipline as `Store`.
- `src/config.rs` — a `note_file` field (env `NOTE_FILE`, default `Note.md`).
- `src/api.rs` — `AppState` gains the note store; new `GET /api/note` and
  `PUT /api/note` routes (the note analog of `GET`/`PUT /api/raw`); a
  `From<io::Error> for AppError` so the note handler can use `?`.
- `src/lib.rs` — declare the two new modules.
- `src/main.rs` — load the `NoteStore` and pass it into `AppState`.
- `web/index.html` — wrap the page in a two-pane layout; add the note pane markup.
- `web/app.js` — load/save the note, dirty tracking, a shared not-ok error helper.
- `web/style.css` — the two-pane layout (and its narrow-screen stack) and the note
  pane styling, reusing existing tokens/rules.
- `tests/api.rs` — give the test app a temp note file; add a `GET`/`PUT /api/note`
  round-trip test.
- `README.md` — document `NOTE_FILE`, the `/api/note` endpoints, and the note pane.

Out of scope (explicit non-goals):

- **No Markdown rendering / preview.** The pane edits the raw `.md` text, the same
  way a task is edited as raw todo.txt. Rendering Markdown to HTML would need a
  client-side Markdown library or a build step, against the repo's deliberate
  no-build, vanilla-embedded-asset design. A live preview pane is a separate
  feature.
- **No per-task notes and no note history/versioning.** One document, last write
  wins, mirroring `PUT /api/raw` for the task file (no optimistic-concurrency
  token). This matches the userless, single-file model of the whole server.
- **No autosave / no keyboard save shortcut.** Saving is an explicit button click,
  consistent with the rest of the UI (the Add button, the Edit prompt) — there is
  no debounce timer or `Ctrl/Cmd+S` handler to maintain.
- **No change to the task store, task parser, or any existing task route.** The
  only edit to `store.rs` is mechanical: extracting its private `write_file` body
  into the shared `atomicfile::write`; its public surface and behavior are
  unchanged.

## 2. Atomic write extraction (`src/atomicfile.rs`)

`store.rs` already writes durably: render to a sibling `.tmp` file, then rename
over the target so a crash never leaves a half-written file. The note store needs
the identical guarantee. Rather than copy the temp-name + rename sequence into a
second module (which would split one durability decision — the `.tmp` convention
and the write-then-rename ordering — across two files), extract it into a small
leaf module both stores call:

```rust
//! Atomic whole-file writes shared by the task and note stores.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Write `data` to `path` atomically: write a sibling temp file, then rename it
/// over the target, so a crash or concurrent reader never sees a partially
/// written file.
pub fn write(path: &Path, data: &str) -> io::Result<()> {
  let mut tmp = path.as_os_str().to_owned();
  tmp.push(".tmp");
  let tmp = PathBuf::from(tmp);

  fs::write(&tmp, data)?;
  fs::rename(&tmp, path)?;
  Ok(())
}
```

`store.rs`'s private `write_file` then becomes a one-line caller, so the store's
behavior is byte-for-byte unchanged and the only thing that moved is where the
temp/rename lives:

```rust
fn write_file(path: &Path, entries: &[Entry]) -> io::Result<()> {
  atomicfile::write(path, &render(entries))
}
```

This is a leaf module (no dependency on `store`, `task`, or anything else), so the
note store depending on it does not create a note→task-store coupling — both
stores depend on a shared filesystem utility, which is the correct direction.

## 3. The note store (`src/note.rs`)

`NoteStore` mirrors `Store`'s shape so the two read alike: the file is the
canonical store, the content is cached in memory, and `replace` persists before it
swaps the new content in (write-then-swap), so the file and memory never diverge.
There is nothing to parse, so it is just a path and a `String`:

```rust
//! Persistent free-form note backed by a single Markdown file.
//!
//! Mirrors `Store`'s write-then-swap discipline: the file is the canonical
//! store, its text is cached in memory, and every save writes atomically (temp +
//! rename, via `atomicfile`) and only swaps the new text into memory once the
//! write succeeds — so the file and memory never diverge. The note has no
//! structure to parse; it is a single block of text.

use crate::atomicfile;
use std::fs;
use std::io;
use std::path::PathBuf;

pub struct NoteStore {
  path: PathBuf,
  content: String,
}

impl NoteStore {
  /// Load the note from `path`. A missing file is treated as an empty note,
  /// exactly as `Store::load` treats a missing todo.txt.
  pub fn load(path: impl Into<PathBuf>) -> io::Result<NoteStore> {
    let path = path.into();
    let content = match fs::read_to_string(&path) {
      Ok(c) => c,
      Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
      Err(e) => return Err(e),
    };
    Ok(NoteStore { path, content })
  }

  /// The current note text.
  pub fn content(&self) -> &str {
    &self.content
  }

  /// Replace the whole note. Persists atomically, then swaps into memory.
  pub fn replace(&mut self, content: &str) -> io::Result<()> {
    atomicfile::write(&self.path, content)?;
    self.content = content.to_string();
    Ok(())
  }
}
```

Unit tests (parallel to `store.rs`'s, in a `#[cfg(test)] mod tests`):

- `missing_file_is_empty` — `load` on a nonexistent path yields `content() == ""`.
- `replace_persists_and_reloads` — `replace` then a fresh `load` of the same path
  returns the written text (proves the write hit disk, not just memory).
- `replace_overwrites` — a second `replace` fully replaces the first content.

## 4. Config (`src/config.rs`)

Add one field beside `file`, following the exact pattern of the existing
env-var reads:

- Struct: `pub note_file: PathBuf` — "Path to the Markdown note file (env
  `NOTE_FILE`, default `Note.md`)."
- `Default`: `note_file: PathBuf::from("Note.md")`.
- `from_env`: `note_file: std::env::var_os("NOTE_FILE").map(PathBuf::from)
  .unwrap_or(defaults.note_file)` — same shape as the existing `file` read.

## 5. API (`src/api.rs`)

`AppState` gains the note store alongside the task store, with the same
`Arc<RwLock<…>>` treatment:

```rust
pub struct AppState {
  pub store: Arc<RwLock<Store>>,
  pub note: Arc<RwLock<NoteStore>>,
  pub config: Arc<Config>,
}

impl AppState {
  pub fn new(store: Store, note: NoteStore, config: Config) -> Self {
    AppState {
      store: Arc::new(RwLock::new(store)),
      note: Arc::new(RwLock::new(note)),
      config: Arc::new(config),
    }
  }
}
```

Two routes, registered next to `/api/raw` because they are the note analog of the
whole-file raw task routes (GET returns the text, PUT replaces it):

```rust
.route("/api/note", get(get_note).put(put_note))
```

```rust
async fn get_note(State(state): State<AppState>) -> Result<Response, AppError> {
  let note = state.note.read().map_err(lock_err)?;
  let body = note.content().to_string();
  Ok(([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], body).into_response())
}

async fn put_note(State(state): State<AppState>, body: String) -> Result<StatusCode, AppError> {
  let mut note = state.note.write().map_err(lock_err)?;
  note.replace(&body)?;
  Ok(StatusCode::NO_CONTENT)
}
```

`put_note` uses `?` on the `io::Result` from `replace`; to make that compile,
add the missing-but-natural conversion (the task path already funnels
`StoreError::Io` to `Internal`, so this keeps one rule — an I/O failure is a 500):

```rust
impl From<io::Error> for AppError {
  fn from(e: io::Error) -> Self {
    AppError::Internal(e.to_string())
  }
}
```

The content type is `text/markdown; charset=utf-8` (the file *is* Markdown);
`get_raw` stays `text/plain`. PUT accepts a `text/plain`/`text/markdown` body as a
plain `String` extractor, exactly like `put_raw`. No JSON DTO — the note has no
fields. CORS, error mapping, and every existing route are untouched.

## 6. Main (`src/main.rs`)

Load the note store beside the task store and hand both to `AppState`:

```rust
let store = Store::load(&config.file)?;
let note = NoteStore::load(&config.note_file)?;
let state = api::AppState::new(store, note, config.clone());
```

The startup log line gains the note path so the operator sees both files:

```rust
println!(
  "my-ntr-server listening on http://{} (todo: {}, note: {})",
  addr,
  config.file.display(),
  config.note_file.display()
);
```

## 7. WebUI markup (`web/index.html`)

Wrap the existing content in a two-pane container. The note pane comes first
(`Note.md | todo.txt`); the todo pane holds the current markup verbatim. The
`<dialog>` stays a direct child of `<main>` (it is a top-layer modal, not part of
either column).

```html
<main>
  <div class="panes">
    <section class="pane note-pane">
      <h1>note</h1>
      <textarea
        id="note-input"
        class="note-input"
        aria-label="Note (Markdown)"
        placeholder="Notes (Markdown)…"
      ></textarea>
      <div class="note-bar">
        <button type="button" id="note-save" disabled>Save</button>
        <span id="note-status" class="note-status" role="status"></span>
      </div>
    </section>

    <section class="pane todo-pane">
      <h1>todo</h1>
      <!-- existing #add-form, #filters, #tasks, #empty, #error markup, verbatim -->
    </section>
  </div>

  <dialog id="label-modal"><!-- unchanged --></dialog>
</main>
```

The existing `<h1>todo</h1>` moves inside `.todo-pane`; the note pane gets a
parallel `<h1>note</h1>`. Nothing inside `#add-form`, `#filters`, `#tasks`, or the
modal changes — they keep their ids, so all existing `app.js` lookups still
resolve.

## 8. WebUI behavior (`web/app.js`)

### 8.1 Shared not-ok error helper

The note transfers raw text, not JSON, so it cannot go through `api()` (which
`JSON.stringify`s the body and `res.json()`s the reply). But both share one thing:
on a non-2xx response, read the API's `{ "error": … }` JSON body for a message,
falling back to the status text. Extract that into a helper so it is written once,
then have `api()` call it — the note fetches call it too:

```js
// On a non-2xx response, surface the API's JSON {error} message (falling back to
// the status text) by throwing it. Shared by the JSON api() helper and the
// plain-text note fetches so the error-extraction logic lives in one place.
async function throwIfNotOk(res) {
  if (res.ok) return;
  let detail = res.statusText;
  try {
    const data = await res.json();
    if (data && data.error) detail = data.error;
  } catch (_) {
    /* non-JSON error body */
  }
  throw new Error(detail);
}
```

`api()` keeps its JSON behavior, now delegating the not-ok branch:

```js
async function api(method, path, body) {
  const opts = { method, headers: {} };
  if (body !== undefined) {
    opts.headers["Content-Type"] = "application/json";
    opts.body = JSON.stringify(body);
  }
  const res = await fetch(path, opts);
  await throwIfNotOk(res);
  if (res.status === 204) return null;
  return res.json();
}
```

### 8.2 Note load / save

Text-bodied counterparts to `api()`, reusing the shared helper:

```js
async function fetchNote() {
  const res = await fetch("/api/note");
  await throwIfNotOk(res);
  return res.text();
}

async function putNote(text) {
  const res = await fetch("/api/note", {
    method: "PUT",
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
    body: text,
  });
  await throwIfNotOk(res);
}
```

Dirty tracking compares the textarea against the last loaded-or-saved text, so the
Save button and the status line never disagree about whether there are pending
edits. The note has its own status element, so a note error never clobbers the
todo `#error` line (and vice versa).

`is the textarea dirty` is one decision, and the Save button's `disabled`, the
status message, and "is it clean" all follow from it — so a single `refreshNote`
derives all of them from that one check. Its only per-caller difference is the
message to show when the note is *clean* (`""` after a load, `"Saved"` right after
a save); when dirty the message is always "Unsaved changes". `cleanStatus` is the
status text to display, not a behavior switch, so it is data, not a flag argument:

```js
// The note text as last loaded or saved. The textarea is "dirty" (unsaved) when it
// differs from this.
let savedNote = "";

// Re-derive every note control from the one source of truth — whether the textarea
// differs from the last saved text. `cleanStatus` is the message to show when the
// note is clean ("" on load, "Saved" just after a save); when dirty the message is
// always "Unsaved changes". The Save button and the status line therefore can never
// disagree about pending edits, because both come from this single check.
function refreshNote(cleanStatus) {
  const dirty = els.noteInput.value !== savedNote;
  els.noteSave.disabled = !dirty;
  setNoteStatus(dirty ? "Unsaved changes" : cleanStatus, false);
}

// Single writer for the note status line; `isError` toggles the error styling so
// a failed load/save shows red in the same slot as the saved/unsaved text.
function setNoteStatus(message, isError) {
  els.noteStatus.textContent = message;
  els.noteStatus.classList.toggle("is-error", Boolean(isError));
}

async function loadNote() {
  try {
    const text = await fetchNote();
    els.noteInput.value = text;
    savedNote = text;
    refreshNote(""); // clean: Save disabled, blank status
  } catch (err) {
    setNoteStatus(`Could not load note: ${err.message}`, true);
  }
}

async function saveNote() {
  const text = els.noteInput.value;
  try {
    await putNote(text);
    savedNote = text;
    refreshNote("Saved"); // clean now: Save disabled, "Saved"
  } catch (err) {
    setNoteStatus(`Could not save note: ${err.message}`, true);
  }
}
```

Wiring and startup (added to the existing tail of the file). The `input` handler
shows the blank clean-status, so a user who types and then deletes back to the
saved text sees the "Unsaved changes" hint clear:

```js
els.noteInput.addEventListener("input", () => refreshNote(""));
els.noteSave.addEventListener("click", saveNote);
// …
loadNote();
```

`els` gains `noteInput`, `noteSave`, `noteStatus`. No existing handler, the add
form, the filters, the sort, or `loadLabels` is touched — the note path is
entirely additive.

## 9. Styling (`web/style.css`)

- Widen `main`'s `max-width` from `720px` to about `1100px` so two panes fit; the
  centering and padding stay.
- `.panes { display: flex; gap: 1.5rem; align-items: flex-start; }` with
  `.pane { flex: 1; min-width: 0; }` (the `min-width: 0` lets a flex child shrink
  instead of overflowing).
- A narrow-screen stack: `@media (max-width: 720px) { .panes { flex-direction:
  column; } }`, so on phones the note sits above the todo pane (the original
  single-column feel, nothing hidden).
- `.note-input` — full-width `<textarea>`, a comfortable min-height (e.g.
  `60vh`), `resize: vertical`, monospace font (it is Markdown source), reusing the
  existing border/radius/`--border` tokens like the other inputs.
- `.note-bar` — a flex row holding the Save button and the status text; the Save
  button reuses the accent-button look already applied to `form#add-form button`
  (add `.note-bar button` to that selector list, rather than re-declaring the
  accent colors).
- `.note-status` — muted text (`--muted`); `.note-status.is-error` switches to the
  same red the `#error` rule already uses (`#b91c1c`).

No existing rule's meaning changes except `main`'s width and the accent-button
selector list; the add-form/filter/list/dialog rules are untouched.

## 10. Tests (`tests/api.rs`)

The integration harness builds `Config { file: <temp>, ..Config::default() }`;
with the new field it must also point `note_file` at a temp path so a test never
reads or writes the real `Note.md`:

```rust
fn app(tag: &str) -> axum::Router {
  let path = temp_path(tag);
  let note_path = temp_path(&format!("{tag}-note"));
  let store = Store::load(&path).unwrap();
  let note = NoteStore::load(&note_path).unwrap();
  let config = Config {
    file: path,
    note_file: note_path,
    ..Config::default()
  };
  router(AppState::new(store, note, config))
}
```

New test mirroring `raw_get_and_replace` — an empty note reads as empty, a PUT
returns 204, and a subsequent GET returns the written text:

```rust
#[tokio::test]
async fn note_get_and_replace() {
  let app = app("note");

  // Empty by default.
  let resp = app.clone().oneshot(get("/api/note")).await.unwrap();
  assert_eq!(resp.status(), StatusCode::OK);
  assert_eq!(body_text(resp).await, "");

  // PUT replaces, 204.
  let resp = app
    .clone()
    .oneshot(put_text("/api/note", "# Title\n\n- a note\n"))
    .await
    .unwrap();
  assert_eq!(resp.status(), StatusCode::NO_CONTENT);

  // GET returns the written text.
  let resp = app.oneshot(get("/api/note")).await.unwrap();
  assert_eq!(body_text(resp).await, "# Title\n\n- a note\n");
}
```

This adds two tiny local helpers in the test file — `get(uri)` (a GET request
builder) and `body_text(resp)` (read the body as a UTF-8 `String`, the text
counterpart of the existing `body_json`) — and reuses a `put_text(uri, body)`
shaped like the existing raw-PUT request in `raw_get_and_replace`. `Config` and
`NoteStore` are imported at the top beside the existing `Config`/`Store` imports.

## 11. Files touched

- `src/atomicfile.rs` (new) — shared atomic whole-file write (section 2).
- `src/note.rs` (new) — `NoteStore` + unit tests (section 3).
- `src/lib.rs` — `pub mod atomicfile;` and `pub mod note;`.
- `src/store.rs` — `write_file` delegates to `atomicfile::write`; remove the
  inline temp/rename (section 2). No behavior change.
- `src/config.rs` — `note_file` field, default, env read (section 4).
- `src/api.rs` — `AppState.note`, `AppState::new` arg, `/api/note` routes +
  handlers, `From<io::Error> for AppError` (section 5).
- `src/main.rs` — load `NoteStore`, pass to `AppState`, log line (section 6).
- `web/index.html` — two-pane wrapper + note pane (section 7).
- `web/app.js` — `throwIfNotOk`, note fetch/save, dirty tracking, wiring,
  `loadNote()` at startup (section 8).
- `web/style.css` — two-pane layout, narrow stack, note pane styling (section 9).
- `tests/api.rs` — temp note file in `app()`, `note_get_and_replace`, text helpers
  (section 10).
- `README.md` — `NOTE_FILE` row, `/api/note` rows, note-pane WebUI paragraph.
- `implemented/note-md-support.md` — written after implementation.

## 12. Verification

- **Rust gates:** `cargo test` (existing task tests + the new `NoteStore` unit
  tests + `note_get_and_replace`), `cargo clippy --all-targets`, and
  `cargo fmt --check` all green. `cargo build` revalidates the embedded web assets
  via `include_str!`.
- **Manual (the WebUI has no JS harness — see Risks):** `cargo run`, open
  `http://127.0.0.1:8080`, then confirm:
  - Two panes appear side by side, note on the left, todo on the right; the todo
    pane works exactly as before (add, filter, sort, edit, complete, delete).
  - Typing in the note enables **Save** and shows "Unsaved changes"; clicking Save
    persists it, disables the button, and shows "Saved".
  - Reloading the page shows the saved note text; the on-disk `Note.md` (or
    `$NOTE_FILE`) contains exactly what was typed.
  - `curl localhost:8080/api/note` returns the text; `curl -X PUT --data-binary @-`
    replaces it and a re-GET reflects the change.
  - Narrowing the window below ~720px stacks the panes (note above todo) with
    nothing clipped.
  - A note save while the server is read-only / the file is unwritable shows the
    red error status in the note pane without disturbing the todo list.

## 13. Risks

- **Knowledge duplication of the atomic write.** Mitigated in section 2 by
  extracting `atomicfile::write` and having both stores call it, so the temp/rename
  durability rule has one home; `store.rs`'s behavior is unchanged.
- **Note vs. todo error bleed.** A shared error line would let a note failure
  overwrite a todo error. Mitigated in section 8 by giving the note its own
  `#note-status` slot; `showError`/`#error` stays todo-only.
- **Last-write-wins on the note.** Two browser tabs (or an API client) saving
  concurrently can clobber each other, exactly as `PUT /api/raw` already can for
  the task file. Accepted as consistent with the userless, single-file model; an
  optimistic-concurrency token is out of scope (section 1).
- **Two label-fetch-shaped helpers (`api` vs. note fetches).** Kept distinct
  because the bodies/replies differ (JSON vs. text); the genuinely shared part —
  not-ok error extraction — is centralized in `throwIfNotOk` (section 8), so this
  is not duplicated knowledge, just two thin format-specific wrappers.
- **No JS test harness** (the deliberate no-build-step decision, consistent with
  every prior WebUI feature), so the note pane's dirty tracking and load/save are
  manually verified; the Rust side (`NoteStore`, the `/api/note` round-trip) is
  covered by `cargo test`.

## 14. brooks-review outcome

`reviews/note-md-support.md` first scored the plan 99/100 with one 🟢 Suggestion
(R3 Knowledge Duplication): the note's single "is the textarea dirty" decision drove
the Save button's `disabled` and the status text inside `reflectNoteState`, but
`loadNote` re-expressed the same "clean ⇒ Save disabled, blank status" rule inline —
splitting one decision across two places, the same DRY smell the `sort-filter`
review caught in `STATUSES`/`buildQuery`. Adopted: section 8 now derives the button,
the status, and "clean vs dirty" from a single `refreshNote(cleanStatus)` that all
three callers (load, input, save) go through, with only the clean-state message
passed in as data (not a flag argument). Re-review scored 100/100, clearing the 95
gate. The only remaining note is the constraint-bound absence of a JS test harness
(no-build-step decision, consistent with every prior WebUI feature), which is not a
deduction; the Rust side (`NoteStore` unit tests + the `/api/note` round-trip) is
covered by `cargo test`.
