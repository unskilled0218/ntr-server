# 18 - Re-check note conflict on focus

## Goal

When the user focuses the note editor, re-GET `Note.md` from the server and run the existing
conflict check, so a divergence is caught the moment the user returns to the note — not only
when a live `note` event happens to arrive. If their unsaved edits would be overwritten, ask
which version to keep (the task-17 dialog).

## Why this is needed (the gap)

Live sync (task 16/17) reconciles the note only on an SSE `note` event, and even then
`maybeReloadNote` deliberately **skips a focused editor with no unsaved edits** to avoid
stealing the caret. So a remote change can sit unseen when:

- the editor was focused-but-clean when the event arrived (skipped on purpose), or
- the SSE connection dropped and events were missed between disconnect and reconnect, or
- the tab was backgrounded and the user returns to it.

Focus is the natural moment to reconcile: the user has just arrived and has not started
typing, so refreshing is expected rather than disruptive. Hence: re-GET on focus and run the
same classify/conflict logic.

## A correctness fix this surfaces: `classifyNoteEvent` must recognise "server unchanged"

`classifyNoteEvent(saved, local, remote)` today is:

```
if (local === remote) return "in-sync";
if (local === saved)  return "adopt";
return "conflict";
```

It was written for an SSE event, which *implies the server changed*, so it never considered
`remote === saved`. On focus that assumption is gone — the common case is "I have unsaved
edits and the server has NOT changed". Under the current code that is `[saved=b, local=m,
remote=b]` -> `conflict`, a **false prompt on every focus while you have unsaved edits**.

Fix the classifier to ask the right first question — *has the server diverged from our
base?* — and add a `"noop"` verdict:

```
if (remote === saved) return "noop";    // server unchanged from our base -> nothing to do
if (local === saved)  return "adopt";   // server changed, no local edits -> take it
if (local === remote) return "in-sync"; // we already hold the server's text -> rebase base
return "conflict";                      // server changed AND local edits differ -> ask
```

Re-derived verdicts (all task-17 SSE cases still behave, plus the new focus case):

| saved | local | remote | verdict   | meaning                                              |
|-------|-------|--------|-----------|------------------------------------------------------|
| b     | b     | b      | noop      | nothing changed anywhere                             |
| b     | m     | b      | noop      | unsaved edits, server unchanged -> keep editing (the fix) |
| b     | b     | t      | adopt     | clean editor, server changed -> take theirs          |
| b     | m     | m      | in-sync   | server now equals our text -> just rebase the base   |
| b     | m     | t      | conflict  | both diverged -> ask                                 |

This also makes the SSE path strictly better: a spurious/echo `note` event whose content
equals our base now resolves to `noop` instead of a needless reload/prompt.

## Refactor: one reconcile core, two triggers

Extract the fetch-and-act body shared by the live handler and the focus handler:

```
reconcileNote():
  if (savingNote || noteConflict) return;          // a save/conflict already owns the editor
  remote = await fetchNote() (return on failure)
  if (savingNote || noteConflict) return;          // re-check after the await (overlap guard)
  local = readNoteText(els.noteInput)
  switch classifyNoteEvent(savedNote, local, remote):
    noop     -> (nothing)
    in-sync  -> savedNote = remote
    adopt    -> adoptNote(remote)
    conflict -> openNoteConflict(remote)
```

Then:

- `maybeReloadNote` (SSE trigger) keeps only the caret-protection skip and delegates:
  ```
  // focused with no unsaved edits: don't yank the caret mid-session; the focus re-check
  // (and blur) will reconcile instead.
  if (!noteDirty() && document.activeElement === els.noteInput) return;
  await reconcileNote();
  ```
- The focus handler calls `reconcileNote()` directly — no caret skip, because at the moment
  of focus the user has not started interacting, so adopting/ prompting is appropriate:
  ```
  els.noteInput.addEventListener("focus", reconcileNote);
  ```

The difference between the two triggers (whether to honour the focused-clean skip) lives in
the two thin wrappers; the reconciliation itself has one home.

## Why the dialog's focus-return is safe

Opening the conflict dialog blurs the note input; closing it returns focus to the note input,
which re-fires `focus` -> `reconcileNote`. With the `"noop"` verdict this is harmless and
cannot loop:

- after **Use theirs**: `savedNote === local === remote`, so the refetch is `remote === saved`
  -> `noop`.
- after **Keep mine**: `flushNote` is in flight (`savingNote`/save), so the re-check returns;
  once saved, `savedNote === local` equals the server, so a later focus is `noop`.

At worst one extra GET on dialog close; no re-prompt, no loop.

## Files changed (web/app.js only)

- Refine `classifyNoteEvent` (reorder + `"noop"`), update its doc comment and verdict list.
- Add `reconcileNote()`; reduce `maybeReloadNote` to the caret skip + delegation.
- Add the `focus` listener on `els.noteInput`.

No server, HTML, or CSS changes: the dialog, styles, and endpoints from task 17 are reused
as-is.

## Tests / verification

As in tasks 8-17, the repo has **no JS test harness**; the pure `classifyNoteEvent` is kept
isolated so it stays unit-testable the day one exists, and is verified programmatically here:

- `node --check web/app.js` clean.
- Run the verdict table above through `classifyNoteEvent` in node (all five rows), including
  the new `[b,m,b] -> noop` row that proves the false-prompt is gone.
- `cargo test` / `fmt` / `clippy` stay green (web assets are embedded via `include_str!`).
- Manual: focus the (clean) editor after a remote change -> it adopts; focus with unsaved
  edits after a remote change -> conflict dialog; focus with unsaved edits and no remote
  change -> no prompt (the fix); dialog Use theirs / Keep mine behave as in task 17.

## Out of scope

- A JS test harness (separate decision, as before).
- Reconciling on other signals (e.g. `visibilitychange` when the tab is shown): focus covers
  the editor-interaction case the feature asks for; a tab-level trigger can be a later task.
- Todo-pane focus reconciliation: tasks re-fetch on their own `tasks` events and have no
  free-text dirty buffer, so there is nothing to conflict on focus.
