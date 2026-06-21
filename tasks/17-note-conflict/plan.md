# 17 - Handle Note.md conflict (ask which version to use)

## Goal

When the note changes on the server while the user has **unsaved local edits** in the
editor, ask the user which version to keep instead of silently dropping one side. This is
the gap left open by live sync (task 16): `maybeReloadNote` currently *skips* the remote
update whenever the editor is dirty, so the local edit silently wins and the remote change
is lost on the next autosave (last-write-wins).

The motivating case: the desktop has a half-typed note when the phone saves a different
note. Today the phone's change is lost. After this change the desktop is asked: keep mine,
or use theirs.

## What counts as a conflict

A `note` SSE event only says "the note changed"; the client must fetch the current remote
text to decide. Given three texts — `savedNote` (the last-persisted base), the editor's
current `local` text, and the fetched `remote` text — there are exactly three outcomes,
named once in a pure function so "what does a note event mean" has a single home:

```
classifyNoteEvent(saved, local, remote):
  local === remote  -> "in-sync"   // editor already equals remote; just move the base
  local === saved   -> "adopt"     // no local edits to lose; take remote
  else              -> "conflict"  // local edits AND remote differs from them -> ask
```

- **in-sync**: nothing to show; set `savedNote = remote` (the editor already holds it).
- **adopt**: load remote into the editor (the existing clean-reload path).
- **conflict**: open a dialog and let the user choose.

This is pure (three strings in, one string out), so the decision is testable and the DOM
code around it stays a thin shell.

## Why a dialog (and which default)

The feature is explicitly "ask the user". A native `<dialog>` (the same primitive the label
modal already uses) gives a focus trap, backdrop, and Escape handling for free. It shows
**both** versions — the user's unsaved text and the incoming text — so the choice is
informed, with two actions:

- **Keep mine** — discard the incoming version; the local edits win.
- **Use theirs** — discard the local edits; adopt the incoming version.

"Keep mine" is the primary (accent) button and is also what an unresolved dialog defaults
to (Escape / dismiss), because silently destroying the *active user's* unsaved work is the
worst outcome; preserving it and overwriting the server copy is recoverable and least
surprising to the person at the keyboard.

## Client changes (web/app.js)

### Factor out the clean-reload core

Extract `adoptNote(text)` — `savedNote = text; setEditorText(text, null); setNoteStatus("", false)`
— the "make the editor show this note as saved/clean" step. `loadNote` becomes
`adoptNote(await fetchNote())` wrapped in its existing error handling, so the
clean-render rule has one home and `maybeReloadNote` can reuse it after its own fetch
without a second GET.

### Conflict state + decision

- `let noteConflict = null;` — holds the pending remote text while the dialog is open
  (truthy = a conflict is being resolved). It is the one guard that blocks overlapping
  reloads/saves during resolution.
- `classifyNoteEvent(saved, local, remote)` — the pure decision above.

### Rework `maybeReloadNote`

```
if (savingNote || noteConflict) return;            // a save/resolution owns the editor
if (!noteDirty() && focused) return;               // focused but clean: don't yank the caret
remote = await fetchNote() (return on failure)
local  = readNoteText(els.noteInput)
switch classifyNoteEvent(savedNote, local, remote):
  in-sync  -> savedNote = remote
  adopt    -> adoptNote(remote)
  conflict -> openNoteConflict(remote)
```

The dirty path now *fetches and decides* instead of skipping. A focused-but-clean editor
still skips (no data to lose, and reloading would steal the caret) — unchanged from task 16.
A focused-and-dirty editor still reaches the conflict prompt, because losing unsaved work
matters more than a caret.

Note that a client's **own** save does not false-trigger: after `putNote` succeeds
`savedNote === local`, so the echoed event classifies as `in-sync`/`adopt`, never
`conflict`.

### Open / resolve

- `openNoteConflict(remote)`: set `noteConflict = remote` **before** `showModal()` (so the
  blur that `showModal` causes on the editor hits the flush guard below and cannot race a
  save in), fill the two `<pre>` previews with `local` and `remote`, show the dialog.
- `resolveNoteConflict(useTheirs)` (idempotent — returns if `noteConflict` is already
  null):
  - clear `noteConflict`, close the dialog if open;
  - **use theirs** -> `adoptNote(remote)` (local edits discarded);
  - **keep mine** -> `savedNote = remote` (acknowledge their version as the new base) then
    `flushNote()` — since `local !== remote`, the editor is now dirty against the new base,
    so the flush PUTs the local text and it wins on the server.
- Wire both buttons to `resolveNoteConflict(false/true)`, and the dialog's `close` event to
  `resolveNoteConflict(false)` so Escape/dismiss defaults to keeping the user's work. The
  idempotent guard makes the button -> close() -> close-event chain a no-op the second time.

### Guard autosave during a pending conflict

Add `if (noteConflict) return;` at the top of `flushNote`, so a debounce timer or the
showModal-induced blur that was already armed cannot push the local text up (silently
resolving the conflict as "keep mine") before the user chooses. "Keep mine" calls
`flushNote` explicitly after clearing the flag, so the deliberate save still happens.

## Markup / style

- `web/index.html`: a `<dialog id="note-conflict">` after the label modal, with a title, a
  one-line explanation, two labelled `<pre>` preview panes (`#conflict-mine`,
  `#conflict-theirs`), and a `.modal-actions` row with `Use theirs` and `Keep mine`
  (`.primary`) buttons.
- `web/style.css`: dialog + backdrop matching `dialog#label-modal`; a
  `.note-conflict-versions` flex row that **wraps** (`flex: 1 1 14rem` on each section) so it
  stacks on narrow screens without re-encoding the mobile breakpoint; scrollable monospace
  `.note-conflict-text` (`max-height: 40vh`, `white-space: pre-wrap`); and
  `.modal-actions button.primary` reusing the accent treatment the submit button already has.
- `web/app.js` `els`: add the dialog, the two `<pre>` nodes, and the two buttons.

## Docs (README.md)

The note section still says saving is "last-write-wins". Update it, and document the live
behavior this builds on (currently undocumented):

- Add a `GET /api/events` row to the API table ("Server-Sent Events stream of `change`
  events (`tasks` / `note`) for live updates").
- In the Note / WebUI prose: note that clients update live when the file changes, and that
  if the note changed elsewhere while you have unsaved edits the editor asks which version
  to keep (idle/clean editors just adopt the change).

## Tests

The client conflict logic is pure in its core (`classifyNoteEvent`) but the repo has **no
JavaScript test harness** — every prior WebUI change (tasks 8–16) was verified manually,
and introducing a JS runner is a separate decision, out of scope here. Extracting
`classifyNoteEvent` keeps the decision unit-testable the day a harness is added, and keeps
the DOM shell thin enough to verify by inspection.

Verification is therefore:

- `cargo test` / `fmt` / `clippy` stay green (no server code changes, but the build must
  still pass).
- Manual end-to-end with two browser sessions against one server:
  1. Edit the note in session A (leave it dirty, unsaved). Save a *different* note in
     session B. Session A shows the conflict dialog with both versions.
  2. "Use theirs" -> A's editor becomes B's text; server keeps B's text.
  3. Repeat; "Keep mine" -> A's text is saved over B's; server ends with A's text.
  4. Idle/clean editor in A + change in B -> A adopts B's text with no dialog (task 16
     behavior preserved).
  5. Escape on the dialog keeps the user's unsaved text (no data loss).

## Out of scope

- Three-way merge / inline diff of the two versions: the choice is whole-document, which is
  all the single-block note supports (it has no structure to merge).
- Conflict handling for the **todo** list: tasks are edited per-item through dedicated
  endpoints and already re-fetch on a `tasks` event; there is no free-text dirty buffer to
  conflict, so this feature is note-only.
- A JavaScript test harness (see Tests).
