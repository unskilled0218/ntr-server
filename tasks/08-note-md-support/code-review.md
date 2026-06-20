# Brooks-Lint Review — Code (REVIEW.md)

**Mode:** PR Review
**Scope:** code — branch `note-md-support` vs `main` (commit `0a376b3`): new
`src/atomicfile.rs`, `src/note.rs`; `src/{api,config,lib,main,store}.rs`;
`tests/api.rs`; `web/{index.html,app.js,style.css}`; `README.md`
**Health Score:** 100/100 (re-review after the `--danger` fix; first pass was 98/100)
**Trend:** 100 → 98 (−2) plan→code first pass, → 100 after the fix

The implementation matches the 100/100 plan: `NoteStore` mirrors `Store`'s
write-then-swap, the durable write is shared via `atomicfile::write` (a leaf module,
no cross-store coupling), `/api/note` is the text-bodied analog of `/api/raw`,
`throwIfNotOk` shares the error extraction, and `refreshNote` is the single source
for the note's dirty state. The Rust gates are green (17 unit + 8 API tests,
`fmt`/`clippy` clean). One actionable Suggestion was found and fixed.

## First pass (98/100)

### 🟢 Suggestion — resolved

**Knowledge Duplication — error-red color repeated as a literal in three rules**
Symptom: `web/style.css` defines every color as a `:root` custom property
(`--bg`, `--fg`, `--muted`, `--accent`, `--border`, `--done`) except the error red
`#b91c1c`, which is written as a bare literal in `#error` and `.modal-error`
(pre-existing) and now a third time in the new `.note-status.is-error`.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY: Don't Repeat Yourself
(a single design decision — "the error color" — expressed as a literal in more than
one place).
Consequence: Changing the error color, or theming it, means editing three rules in
lockstep, and they can silently drift; it is also the only color that bypasses the
established `:root`-variable convention, a small conceptual-integrity dent.
Remedy applied: Added `--danger: #b91c1c` to `:root` and pointed all three rules at
`var(--danger)`, so the error color has one home like every other color and the new
rule follows the same convention as the rest of the file.

### 🟢 Suggestion — accepted as constraint-bound (no change)

**Coverage Illusion — the WebUI note pane ships without an automated test**
Symptom: The note's dirty tracking (`refreshNote`), `loadNote`/`saveNote`, and the
`throwIfNotOk` text path are verified by reading the code and by manual smoke test,
not by a mechanical test.
Source: Feathers — Working Effectively with Legacy Code, Ch. 1.
Consequence: A regression — e.g. dropping the `savedNote` write-back so the Save
button never re-disables — would not be caught by CI.
Remedy: Accept as constraint-bound — the repo has no JS test harness by the
deliberate no-build-step decision, consistent with every prior WebUI feature. The
backend half of the feature *is* covered: `NoteStore`'s three unit tests and the
`note_get_and_replace` API round-trip. No change required.

## Remaining (100/100)

No further findings. Considered and dismissed:

- **Accidental Complexity — `atomicfile` is a one-function module / `NoteStore` is
  thin.** `atomicfile::write` has two real callers (`store` and `note`); extracting
  it is the DRY remedy, not speculative generality. `NoteStore` owns real policy
  (missing-file-as-empty, write-then-swap) and keeps `fs` out of the API layer. No
  finding.
- **Change Propagation — `AppState::new`/`Config` gained an argument/field.** The
  ripple to `main.rs` and the test harness is the minimal cost of a second backing
  file; no existing task route, the parser, or the store's behavior changed. No
  finding.
- **Dependency Disorder — direction.** `note → atomicfile` and `store → atomicfile`
  both point down at a shared leaf; no cycle, no note→store coupling. No finding.

## Summary

The code matches the 100/100 plan; the one actionable Suggestion (the error-red
literal duplicated across three CSS rules) is resolved by a `--danger` variable, so
the only remaining note is the constraint-bound absence of a JS test harness. Score
100/100 after the fix.
