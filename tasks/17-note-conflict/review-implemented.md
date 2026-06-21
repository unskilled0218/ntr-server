# 17 - Note conflict — review fix implemented

Applied the 🟡 from code-review.md per review-plan.md.

## Fix — showModal race (🟡 -> resolved)
- maybeReloadNote re-checks `if (savingNote || noteConflict) return;` AFTER `await
  fetchNote()`, so two overlapping note events (change event + reconnect resync) can't both
  reach the conflict branch. JS runs each continuation to completion, so the first owner
  wins and the rest back off.
- openNoteConflict is now a no-op for showModal when the dialog is already open (it just
  refreshes the incoming "theirs" preview): one home for "never show the dialog twice".

## Left as-is
- 🟢 Coverage: no JS harness in repo (accepted tradeoff; pure classifyNoteEvent isolated +
  verified in node).

## Verification
- node --check clean; classifyNoteEvent decision table green.
- cargo test 30 pass; fmt + clippy clean.

Re-review: 99/100.
