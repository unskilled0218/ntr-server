# Brooks-Lint Review

**Mode:** PR Review
**Scope:** `plans/note-md-autosave.md` (design plan for the Note.md autosave feature)
**Health Score:** 100/100 (99/100 first pass, one Suggestion adopted)

The autosave design is tight and well-justified — each moving part maps to a concrete
failure it prevents; the one first-pass Suggestion (a dirty-check decision repeated in
three places) was folded back into the plan.

---

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

---

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

---

# Code Review (committed implementation, `web/*` @ commit e0efc1e)

**Mode:** PR Review
**Scope:** `web/app.js`, `web/index.html`, `web/style.css` (the autosave feature as built)
**Health Score:** 100/100 (99/100 first pass, one Suggestion fixed)

The implementation matches the 100/100 plan; reviewing it as shipped code surfaced one
behaviour the plan-level review had not — a failed save re-armed the debounce, turning a
down server into a retry loop — now fixed.

## Findings

### 🟢 Suggestion — fixed in this branch

**Accidental Complexity — a failed save looped on the debounce instead of waiting for
the next edit**
Symptom: In `flushNote`, the post-save re-check `if (noteDirty()) scheduleNoteSave()`
ran after the `try/catch/finally`, so it re-armed the debounce on *failure* too (the
text is still dirty when a save throws). A persistently failing `PUT /api/note` (server
down, file unwritable) therefore retried every `NOTE_SAVE_DELAY_MS` indefinitely while
the tab stayed open — and contradicted the README, which states a failed save is
"retried on the next edit".
Source: Ousterhout — A Philosophy of Software Design, Ch. 3: Strategic vs. Tactical
Programming (the re-check quietly took on a second job — error retry — beyond its stated
"persist mid-save edits" purpose); Hunt & Thomas — The Pragmatic Programmer, Ch. 2
(code and its documentation drifting apart).
Consequence: An unbounded background retry storm against a failing endpoint, and a
code/README disagreement that would mislead the next reader about when a failed note
save is retried.
Remedy: Move the re-check inside the success branch (right after `savedNote = text`), so
it only re-arms to flush edits typed during a *successful* save. A failed save now waits
for the next edit (which arms its own timer) or the `pagehide` beacon — matching the
README. **Fixed.** Mid-save edits during a successful PUT are still caught, and edits
typed during a *failed* save are not lost because each keystroke already armed its own
debounce timer.

## Remaining (100/100)

No further findings. The dirty rule has one home (`noteDirty()`); the single-flight
guard and `pagehide` keepalive flush are load-bearing (see the plan review above); the
status literals are each single-use; `src/` is untouched so the backend stays covered by
`note_get_and_replace`; and the no-JS-test-harness gap is the standing constraint-bound
position, not a deduction.

## Summary

The built feature is sound. The one issue — a failed autosave re-arming the debounce and
contradicting the documented "retry on next edit" — is fixed by gating the post-save
re-check on success. Score 100/100 after the fix.
