# Brooks-Lint Review — code (note-conflict vs main)

## Score 94/100

- 🟡 Change Propagation (race): maybeReloadNote checks savingNote/noteConflict before
  `await fetchNote()` but not after. Two near-simultaneous note events (e.g. a change
  event + the reconnect open-resync) both pass the guard while noteConflict is null, both
  await, both reach the conflict branch -> second openNoteConflict calls showModal() on an
  already-open <dialog> = InvalidStateError, and clobbers noteConflict.
  Remedy: re-check `if (savingNote || noteConflict) return;` after the await (and optionally
  make openNoteConflict a no-op when the dialog is already open).
- 🟢 Coverage Illusion: conflict flow not auto-tested. Justified tradeoff (no JS harness in
  repo; pure classifyNoteEvent isolated + verified). Suggestion, not blocker.

Fix plan: review-plan.md. Implementing the 🟡.
