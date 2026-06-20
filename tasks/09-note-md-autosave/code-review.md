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
