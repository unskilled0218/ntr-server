# Brooks-Lint Review

**Mode:** PR Review
**Scope:** `plans/note-md-autosave.md` (design plan for the Note.md autosave feature)
**Health Score:** 100/100 (99/100 first pass, one Suggestion adopted)

The autosave design is tight and well-justified — each moving part maps to a concrete
failure it prevents; the one first-pass Suggestion (a dirty-check decision repeated in
three places) was folded back into the plan.


## Findings

### 🟢 Suggestion — adopted into the plan (first pass, now resolved)

**Knowledge Duplication — the "note is dirty" decision was expressed three times**
Symptom: The predicate `els.noteInput.value !== savedNote` (and its inverse `===`)
appeared in three sites of section 3 — the `flushNote` "nothing changed" guard, the
post-save re-check that re-arms the debounce, and the `flushNoteBeacon` early-out.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY: Don't Repeat Yourself (the
*decision* "what counts as dirty", not the keystrokes, is the duplicated knowledge);
Fowler — Refactoring, Duplicate Code.
Consequence: If the dirty definition ever changes (e.g. trimming trailing whitespace
before comparison, or normalising line endings the server rewrites), one of the three
copies is easy to miss, and a missed copy means a save that silently no-ops or a
beacon that fires a redundant write — exactly the kind of drift the original
`note-md-support` plan avoided by routing the button + status through one
`refreshNote` dirty check.
Remedy: Extract a one-line `noteDirty()` predicate and call it from all three sites,
so the dirty definition has a single home. **Adopted:** section 3 now defines
`noteDirty()` and the three sites call it.


## Remaining (100/100)

No further findings. Considered and dismissed:

- **Accidental Complexity — the post-save re-check looks redundant with the
  per-keystroke debounce.** It is not: `flushNote` calls `clearTimeout` *before* the
  single-flight early-return, so a `blur` (or debounce) that fires during an in-flight
  save cancels the pending timer and then bails on the guard — without the completion
  re-check, the edit typed during that save would never be persisted until the next
  keystroke. The re-check is load-bearing, not belt-and-suspenders. No finding.
- **Accidental Complexity — `pagehide` keepalive flush is speculative.** It prevents a
  real, reproducible data loss (typing then closing the tab while focused, where `blur`
  never fires) — core to "auto save", not generality for an absent requirement. The
  ~64 KB keepalive cap is documented as a bounded, normal-path-covered limitation. No
  finding.
- **Cognitive Overload — three module-level `let`s plus a timer.** `savedNote`,
  `noteSaveTimer`, `savingNote` are each single-purpose, named, and commented; the
  whole save flow is under twenty lines with no nesting past one level. Below the
  20-line / nesting-3 thresholds. No finding.
- **Change Propagation / Dependency Disorder.** The change is WebUI-only and additive;
  `src/` (the backend the autosave calls) is untouched, and `PUT /api/note` already
  exists. The html/css/js edits are one cohesive feature, not shotgun surgery. No
  finding.
- **Coverage Illusion — no automated test for the autosave logic.** Constraint-bound:
  the repo has no JS test harness by its deliberate no-build-step decision (consistent
  with every prior WebUI feature), and the endpoint autosave drives (`PUT /api/note`)
  stays covered by the existing `note_get_and_replace` integration test. Not a
  deduction.

## Summary

The plan is ready to implement. The single actionable finding — a dirty-check decision
duplicated across three call sites — is resolved by extracting a `noteDirty()`
predicate, giving the autosave's one definition of "dirty" a single home. Score 100/100
after that change.

