# 17 - Note.md conflict — implemented

When a `note` live-update arrives while the editor has unsaved local edits and the remote
text differs from them, the editor now asks the user which version to keep, instead of
silently dropping the remote change (the task-16 behavior).

## Decision (pure)

`classifyNoteEvent(saved, local, remote)` returns one of:
- `in-sync` — editor already equals remote; just move `savedNote` to it.
- `adopt` — no unsaved local edits; take the remote version.
- `conflict` — unsaved local edits AND remote differs from them; ask the user.

It is pure (three strings -> one verdict), so the meaning of an event has one testable home
and the DOM code is a thin shell.

## Files changed

- `web/app.js`:
  - `noteConflict` state (pending remote text; truthy = dialog owns the editor).
  - `adoptNote(text)` factored out of `loadNote` (one home for "show this note as saved/clean").
  - `classifyNoteEvent` (pure); `maybeReloadNote` reworked to fetch + dispatch on the verdict.
  - `openNoteConflict` (sets the flag before `showModal` so the induced blur can't sneak a
    save in) and idempotent `resolveNoteConflict(useTheirs)`: use-theirs adopts remote;
    keep-mine sets the remote as the new base then `flushNote()` saves local over it.
  - `flushNote` guarded with `if (noteConflict) return;` so an armed autosave can't resolve
    the conflict before the user does.
  - `els` entries + wiring: both buttons, and the dialog `close` event -> keep-mine default
    (Escape/dismiss never discards the user's unsaved work).
- `web/index.html`: `<dialog id="note-conflict">` with two labelled `<pre>` previews and
  Use theirs / Keep mine (`.primary`) buttons.
- `web/style.css`: dialog + backdrop matching the label modal; `.note-conflict-versions`
  flex row that wraps to one column on narrow widths (no new breakpoint); scrollable
  monospace previews; `.modal-actions button.primary` reusing the accent treatment.
- `README.md`: documented `GET /api/events` and the live-update + conflict behavior;
  corrected the note's "last-write-wins" wording.

## Verification

- `node --check web/app.js` clean; every `getElementById` id resolves against index.html.
- `classifyNoteEvent` exercised across the full case table in node (in-sync incl. own-save
  echo, adopt, and both conflict shapes) — all pass.
- `cargo test` 30 pass, `cargo fmt --check` clean, `cargo clippy --all-targets` clean
  (web assets are embedded via `include_str!`, so the build re-embeds them).
- Server serves the updated index.html / app.js / style.css with the conflict feature.

Not done automatically: the interactive two-browser dialog flow (clicking the buttons in
real sessions) — the repo has no JS/browser test harness (consistent with tasks 8-16), so
the pure decision is verified programmatically and the DOM shell by inspection; the manual
two-session scenarios are listed in plan.md for a human pass.
