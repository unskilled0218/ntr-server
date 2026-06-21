# 17 - Note conflict — code-review fix plan

Addresses the 🟡 from code-review.md. The 🟢 (coverage) is an accepted, justified tradeoff
(no JS harness in repo) and is left as-is, consistent with tasks 8-16.

## Fix (🟡 race) — re-check the guard after the await in `maybeReloadNote`

`maybeReloadNote` guards on `savingNote || noteConflict` only *before* `await fetchNote()`.
Two note events arriving close together (a `change` event plus the reconnect `open`
resync) can both pass the guard while `noteConflict` is still null, both await, and both
reach the `conflict` branch — the second `openNoteConflict` then calls `showModal()` on an
already-open `<dialog>` (throws `InvalidStateError`) and overwrites `noteConflict`.

Change: after `const local = readNoteText(els.noteInput);` (i.e. after the await resolves),
re-check before dispatching:

```
if (savingNote || noteConflict) return; // state may have changed during the fetch
```

Because JS runs each async continuation to completion without interruption, whichever
continuation runs first takes ownership (opens the dialog / sets `noteConflict` / starts a
save) and the other returns at this re-check. That fully closes the window — no second
`showModal`, no clobbered `noteConflict`.

Belt-and-braces: also make `openNoteConflict` defensive — if `els.conflictDialog.open` is
already true, refresh the pending remote + the "theirs" preview and return without calling
`showModal()` again. This keeps the function safe even if a future caller forgets the guard
(one home for "never showModal twice").

## Verification

- `node --check web/app.js` clean.
- Re-run the `classifyNoteEvent` decision-table check in node (unchanged) — still green.
- `cargo test` / `fmt` / `clippy` green (web assets re-embedded).
- Manual reasoning: simulate two overlapping calls — first opens dialog, second hits the
  post-await re-check and returns; no exception.

## Out of scope

- JS test harness (the 🟢 coverage finding) — separate decision, as in the plan.
