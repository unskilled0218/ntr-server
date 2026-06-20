# Plan: Auto-save Note.md (branch `note-md-autosave`)

Make the WebUI note pane save itself. Today the note is persisted only when the
user clicks **Save**; this feature replaces that explicit click with debounced
autosave so edits are written shortly after the user stops typing, on blur, and on
page close — and removes the now-redundant Save button.

## 1. Goal and scope

Requirement (from `agents/ADD_NEW_FEATURE.md`):

> - Auto save note.md

The `note-md-support` feature deliberately listed this as a non-goal ("No autosave
/ no keyboard save shortcut") because saving was an explicit button click,
consistent with the Add button and the Edit prompt. This feature reverses that one
decision for the note pane only: the note is a single free-form scratchpad, and a
scratchpad that silently keeps itself saved matches how notes are used better than a
Save button does. Nothing about tasks changes — task add/edit stay explicit.

In scope (WebUI only — the backend already has everything autosave needs):

- `web/app.js` — replace the Save-button save flow (`refreshNote`, `saveNote`, the
  button wiring) with a debounced autosave: a per-keystroke debounce timer, a
  single-flight guard so two PUTs never overlap, and immediate flushes on blur and
  on page close. `loadNote`, `setNoteStatus`, `throwIfNotOk`, `fetchNote`, and
  `putNote` are reused (the last gains an optional `keepalive` flag for the
  page-close flush).
- `web/index.html` — remove the `<button id="note-save">`; the `.note-bar` keeps
  only the status line.
- `web/style.css` — drop the two note-button rules (`.note-bar button` from the
  accent-button selector list, and `.note-bar button:disabled`), now dead.
- `README.md` — update the WebUI note-pane paragraph from "Save button … enabled
  only while there are unsaved edits" to the autosave behaviour and its status
  states.
- `implemented/note-md-autosave.md` — written after implementation.

Out of scope (explicit non-goals):

- **No backend change.** `PUT /api/note` already persists the note atomically and
  is already covered by the `note_get_and_replace` integration test. Autosave is a
  client that calls the existing endpoint more often; `src/` is untouched.
- **No change to task saving.** Task add and the raw-edit prompt stay explicit
  clicks/prompts. This feature touches only the note pane.
- **No autosave debounce/interval configuration, no per-edit version history, no
  conflict detection.** `PUT /api/note` stays last-write-wins, exactly as
  `PUT /api/raw` is for the task file (documented in README). Two tabs editing the
  note can still clobber each other, unchanged from today.
- **No Markdown preview, no keyboard `Ctrl/Cmd+S`.** Out of scope as before; the
  pane still edits raw Markdown text. (A `Ctrl+S` flush would be a reasonable small
  follow-up but is not part of "auto save".)

## 2. The autosave model (the one idea this feature adds)

There is a single invariant: **`savedNote` holds the text last successfully
persisted; the note is "dirty" exactly when the textarea differs from it, and the
job of autosave is to make `dirty` become false on its own.** Everything below is
in service of that invariant, with no lost edits and no overlapping writes.

Three things drive a save:

1. **Debounce** — every keystroke (re)arms a timer for `NOTE_SAVE_DELAY_MS`; when it
   fires (the user paused) the note is flushed. This is the normal path.
2. **Blur** — leaving the textarea flushes immediately, so clicking away saves at
   once instead of waiting out the debounce.
3. **Page close** — `pagehide` fires a best-effort `keepalive` PUT so the last edit
   is not lost when the tab/window closes while still focused (blur would not have
   fired). This is what makes it real autosave rather than autosave-unless-you-
   close-quickly.

Two correctness rules tie them together:

- **Single-flight.** A boolean `saving` guard means a flush started while a PUT is
  already in flight does nothing *now* — it cannot start a second, possibly
  out-of-order, write.
- **No lost final edit.** When an in-flight save completes, it re-checks the
  textarea against `savedNote`; if the user typed during the save, it re-arms the
  debounce so the newest text is persisted too. (Each keystroke during the save
  also re-arms the timer, so this re-check is the belt to that suspenders — it
  guarantees progress even in the corner case where the last keystroke landed
  while the guard was up.)

This is the minimum machinery for "saves itself without losing or duplicating
writes"; each part maps to a concrete failure it prevents (section 7 lists them),
so none is speculative.

## 3. `web/app.js`

### 3.1 The debounce constant

One named source for the pause length, beside the other top-level constants:

```js
// How long after the last keystroke the note auto-saves. One named source so the
// debounce length is not a magic number sprinkled through the save logic.
const NOTE_SAVE_DELAY_MS = 800;
```

### 3.2 `els`: drop the Save button

`els.noteSave` is removed (the button no longer exists). `els.noteInput` and
`els.noteStatus` stay.

### 3.3 `putNote`: an optional keepalive flag

The page-close flush cannot await a normal `fetch` (the page is unloading), so it
needs `keepalive: true`, which lets the browser complete the request after the
document goes away. One optional argument keeps a single note-PUT helper rather
than a second near-duplicate:

```js
// PUT the note's Markdown text (text body, 204 on success). `keepalive` lets the
// request outlive the page for the pagehide flush; it defaults off for normal saves.
async function putNote(text, { keepalive = false } = {}) {
  const res = await fetch("/api/note", {
    method: "PUT",
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
    body: text,
    keepalive,
  });
  await throwIfNotOk(res);
}
```

### 3.4 The save flow (replaces `refreshNote` / `saveNote`)

```js
// The note text as last successfully persisted. The textarea is "dirty" (unsaved)
// when it differs from this; autosave's job is to drive it back to equal.
let savedNote = "";
// Pending debounce timer id (null when none is armed).
let noteSaveTimer = null;
// True while a PUT is in flight, so a second flush never starts an overlapping,
// possibly out-of-order, write (single-flight).
let savingNote = false;

// The one definition of "the note has unsaved edits": the textarea differs from the
// last persisted text. Every save path (the flush guard, the post-save re-check, the
// page-close beacon) asks this, so the dirty rule has a single home.
function noteDirty() {
  return els.noteInput.value !== savedNote;
}

// (Re)arm the debounce: a save fires NOTE_SAVE_DELAY_MS after the last keystroke.
function scheduleNoteSave() {
  clearTimeout(noteSaveTimer);
  noteSaveTimer = setTimeout(flushNote, NOTE_SAVE_DELAY_MS);
}

// Persist the note now if it is dirty and no save is already running. Cancels any
// pending debounce (this IS the save it was waiting for). On completion it re-checks
// for edits made during the PUT and re-arms the debounce, so the newest text is
// never left unsaved. Used by the debounce, by blur, and after a save completes.
async function flushNote() {
  clearTimeout(noteSaveTimer);
  noteSaveTimer = null;
  if (savingNote) return; // single-flight; the in-flight save's re-check will catch up
  if (!noteDirty()) return; // nothing changed
  const text = els.noteInput.value;
  savingNote = true;
  setNoteStatus("Saving…", false);
  try {
    await putNote(text);
    savedNote = text;
    setNoteStatus("Saved", false);
  } catch (err) {
    setNoteStatus(`Could not save note: ${err.message}`, true);
  } finally {
    savingNote = false;
  }
  // The user may have typed during the PUT; persist that newest text too. (flushNote
  // clears the pending timer before the single-flight return, so a blur/debounce that
  // fired mid-save left nothing armed — this re-check is what re-arms it.)
  if (noteDirty()) scheduleNoteSave();
}

// Best-effort save when the page is going away (tab/window close, navigation).
// Cannot await on unload, so it uses keepalive and ignores the result; the normal
// path will have saved everything earlier, this only covers the last unsaved edit.
function flushNoteBeacon() {
  if (!noteDirty()) return;
  putNote(els.noteInput.value, { keepalive: true }).catch(() => {});
}
```

`setNoteStatus` is unchanged. `loadNote` keeps its shape, just dropping the
button-oriented `refreshNote` call for a plain status reset:

```js
async function loadNote() {
  try {
    const text = await fetchNote();
    els.noteInput.value = text;
    savedNote = text;
    setNoteStatus("", false); // freshly loaded: clean, no message
  } catch (err) {
    setNoteStatus(`Could not load note: ${err.message}`, true);
  }
}
```

### 3.5 Wiring (replaces the input/Save listeners)

```js
// Autosave: typing shows "Editing…" and (re)arms the debounce; leaving the field
// flushes at once; closing the page flushes best-effort so the last edit survives.
els.noteInput.addEventListener("input", () => {
  setNoteStatus("Editing…", false);
  scheduleNoteSave();
});
els.noteInput.addEventListener("blur", flushNote);
window.addEventListener("pagehide", flushNoteBeacon);
```

The startup `loadNote()` call at the file's tail is unchanged.

Status line lifecycle: `""` (loaded/clean) → `"Editing…"` (typing) → `"Saving…"`
(PUT in flight) → `"Saved"` (persisted), or a red `"Could not save note: …"` on
failure. After an error, the next keystroke shows `"Editing…"` and re-arms the
debounce, so autosave self-heals — a transient failure is retried on the next edit.

Removed: `refreshNote`, `saveNote`, the `els.noteSave` lookup, the
`els.noteSave.addEventListener("click", …)` line, and the old
`els.noteInput` input handler that called `refreshNote("")`.

## 4. `web/index.html`

Drop the Save button; keep the status span in `.note-bar`:

```html
<div class="note-bar">
  <span id="note-status" class="note-status" role="status"></span>
</div>
```

`#note-input`, `#note-status`, and every other id are unchanged, so all other
`app.js` lookups still resolve.

## 5. `web/style.css`

Two rules become dead once the button is gone and are removed:

- `.note-bar button` from the accent-button selector list — the list becomes just
  `form#add-form button { … }` (the add-form Save button keeps its accent style).
- The whole `.note-bar button:disabled { … }` rule.

`.note-bar` (the flex row) and `.note-status` / `.note-status.is-error` stay as-is;
the bar now lays out a single status span, which is fine. No other rule changes.

## 6. Tests and verification

No new automated test: the only code added is client-side JS, and the repo has no
JS test harness by its deliberate no-build-step decision (consistent with every
prior WebUI feature). The endpoint autosave drives — `PUT /api/note` — is already
covered by the existing `note_get_and_replace` integration test, and `src/` is
untouched, so the whole existing suite still applies unchanged.

- **Rust gates (regression guard, nothing should change):** `cargo test`,
  `cargo clippy --all-targets`, `cargo fmt --check` all green. `cargo build`
  re-embeds the edited web assets via `include_str!`, so a malformed template would
  fail the build.
- **Manual (the autosave behaviour):** `cargo run`, open `http://127.0.0.1:8080`:
  - The note pane has no Save button; a status line sits where it was.
  - Type in the note: status shows "Editing…", then ~0.8 s after you stop it shows
    "Saving…" then "Saved". `cat $NOTE_FILE` (default `Note.md`) shows the text.
  - Type and immediately click into the todo pane (blur): it saves at once
    ("Saving…"/"Saved") without waiting for the debounce.
  - Type and immediately close the tab, reopen: the last edit is present (pagehide
    keepalive flush).
  - Reload the page mid-edit-then-pause: the persisted text is shown; `savedNote`
    re-syncs and the status returns to blank.
  - Make the file unwritable (or stop the server) and edit: the status shows the
    red "Could not save note: …"; fixing it and typing again saves successfully
    (self-heal on next edit), with no overlapping requests in the network panel.
  - Hold a key to type continuously: only one PUT fires after you stop (debounce
    coalesces), and the network panel never shows two note PUTs in flight at once
    (single-flight).

## 7. Risks

- **Lost final edit (data loss).** The headline risk for any autosave. Mitigated by
  three converging triggers (debounce, blur, pagehide) plus the post-save re-check
  in `flushNote`, so the latest text is persisted on pause, on leaving the field, or
  at the latest on page close.
- **Overlapping / out-of-order writes.** Without a guard, a debounce firing during
  an in-flight PUT could start a second, racing write. Mitigated by the `savingNote`
  single-flight guard; the in-flight save's completion re-check then persists any
  edit made during the write.
- **`keepalive` body limit.** Browsers cap the total keepalive request body at
  ~64 KB. A note larger than that may not flush on `pagehide` — but the debounce and
  blur paths (no keepalive, no cap) will already have saved it during normal use;
  pagehide only covers the last unsaved edit. Acceptable and documented here.
- **Last-write-wins across tabs.** Two tabs autosaving the same note can overwrite
  each other, exactly as `PUT /api/raw` already allows for the task file. Unchanged
  by this feature and consistent with the userless single-file model; conflict
  detection is out of scope (section 1).
- **No JS test harness.** The autosave logic is verified by reading the code and by
  the manual steps in section 6, not mechanically — the same constraint-bound
  position as every prior WebUI feature. The server half it depends on (`PUT
  /api/note`) stays covered by `note_get_and_replace`.
- **`Editing…`/`Saving…`/`Saved` brief flicker.** If an edit lands during a save,
  the status can flash "Saved" then "Editing…/Saving…" as the re-check fires. Purely
  cosmetic, self-correcting, and preferable to dropping the edit.

## 8. Files touched

- `web/app.js` — autosave (debounce + single-flight + blur/pagehide flush);
  `putNote` keepalive flag; `loadNote` status reset; remove `refreshNote`,
  `saveNote`, `els.noteSave`, and the Save-button/old-input wiring (section 3).
- `web/index.html` — remove the Save button from `.note-bar` (section 4).
- `web/style.css` — remove the two now-dead note-button rules (section 5).
- `README.md` — autosave description in the WebUI note-pane paragraph (section 1).
- `implemented/note-md-autosave.md` — written after implementation.

## 9. brooks-review outcome

`reviews/note-md-autosave.md` scored the plan 99/100 on the first pass with one 🟢
Suggestion (R3 Knowledge Duplication): the "note is dirty" decision
(`els.noteInput.value !== savedNote`) was inlined at three save sites — the
`flushNote` guard, the post-save re-check, and the `flushNoteBeacon` early-out — so a
future change to what "dirty" means could miss a copy. Adopted: section 3 now defines a
single `noteDirty()` predicate that all three sites call. The re-review found no further
issues — the post-save re-check and the `pagehide` keepalive flush were each examined
for Accidental Complexity and judged load-bearing (the re-check re-arms a timer
`flushNote` cancels before its single-flight return; the beacon prevents real
close-the-tab data loss) — clearing the 95 gate at 100/100. The only remaining note is
the constraint-bound absence of a JS test harness (consistent with every prior WebUI
feature); the server side autosave depends on (`PUT /api/note`) stays covered by
`note_get_and_replace`.
