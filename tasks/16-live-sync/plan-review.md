# Brooks-Lint Review — plan.md

## Round 1 — Score 89/100

- 🟡 R4 Accidental Complexity: two change-detection paths (handler-notify + poller)
  where the poller alone already catches every change. Remove handler-notify.
- 🟡 R3 Knowledge Duplication: reload_from_disk would be a third copy of the
  parse-file-into-entries loop (load, replace_raw, reload). Extract parse_entries.
- 🟢 R2 Hyrum's Law: "tasks"/"note" wire vocabulary duplicated server (Change::as_str)
  and client (JS dispatch) with nothing linking them. Add cross-reference comments.

## Round 2 — Score 100/100

All three resolved:
- Poller is the sole change detector; handlers untouched. ≤1s trade-off justified.
- parse_entries(content, start_id) shared by load, replace_raw, reload_from_disk.
- Mirrored cross-reference comments on Change enum <-> JS change dispatch.

Proceeding to implementation.

## Round 3 — Score 99/100 (design correction)

End-to-end testing of the implementation revealed the round-2 "poller-only" design was
WRONG for the primary use case: a mutation handler updates memory and disk together, so
the poller (which fires only when disk != memory) never observes an API change. The
phone-edits-via-app -> desktop-updates case produced no event.

Correction: restored the two-trigger design (handler announces its own writes; poller
covers out-of-band file edits only). Verified end-to-end — API POST/PUT and an external
file write each emit exactly one event, no double-fire.

Residual 🟢: every mutating handler must call notify() (no compile-time enforcement; the
poller is not a safety net since API writes leave disk==memory). Mitigated by the
documented invariant on AppState::notify. 99/100, above the >95 gate.
