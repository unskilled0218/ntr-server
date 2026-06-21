# 18 - Re-check note conflict on focus — implemented

Focusing the note editor now re-GETs `Note.md` and runs the conflict check, so a divergence
is caught when the user returns to the note (not only on a live `note` event). If unsaved
edits would be overwritten, the task-17 dialog asks which version to keep.

## Classifier fix (the substantive part)

Reconciling on focus makes "server unchanged from our base while I have unsaved edits" a
common input — which the old `classifyNoteEvent` mis-classified as `conflict` (it assumed an
SSE event meant the server had changed), so it would have falsely prompted on every focus
with unsaved edits. Refined to ask "did the server diverge from our base?" first and added a
`"noop"` verdict:

| saved | local | remote | verdict  |
|-------|-------|--------|----------|
| b     | b     | b      | noop     |
| b     | m     | b      | noop     (the fix) |
| b     | b     | t      | adopt    |
| b     | m     | m      | in-sync  |
| b     | m     | t      | conflict |

The SSE path is strictly better too: an echo event whose content equals our base is now
`noop` instead of a needless reload/prompt.

## Refactor

- `reconcileNote()` — the shared fetch-and-act core (overlap-guarded before and after the
  await), used by both triggers.
- `maybeReloadNote` — SSE trigger; keeps only the focused-clean caret-protection skip, then
  delegates to `reconcileNote`.
- `els.noteInput` `focus` listener -> `reconcileNote` directly (no caret skip: at focus the
  user has not started interacting).

The dialog's focus-return (closing the modal refocuses the editor, re-firing `focus`) is
safe under `"noop"`: after Use theirs the refetch is `remote === saved` -> noop; after Keep
mine the in-flight save guards it. At most one extra GET, no loop.

## Files changed

- `web/app.js` only: refined `classifyNoteEvent`; added `reconcileNote`; reduced
  `maybeReloadNote`; added the `focus` listener. No server / HTML / CSS changes (task-17
  dialog, styles, and endpoints reused).

## Verification

- `node --check web/app.js` clean; all `getElementById` ids resolve.
- `classifyNoteEvent` verified across the five-row verdict table in node (incl. the new
  `[b,m,b] -> noop`); all pass.
- `cargo test` 30 pass; `cargo fmt --check` + `cargo clippy --all-targets` clean (web assets
  embedded via `include_str!`). Served app.js confirmed to contain the focus reconcile.

Not automated: the interactive focus/dialog flow in a real browser — no JS/browser harness in
the repo (tasks 8-17 norm); the pure decision is verified programmatically, the DOM shell by
inspection, and the manual scenarios are listed in plan.md.
