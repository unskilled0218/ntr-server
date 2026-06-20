# Brooks-Lint Review

**Mode:** PR Review
**Scope:** plans/note-md-support.md (design plan; new `Note.md` store + `/api/note`
API + side-by-side WebUI note pane)
**Health Score:** 100/100 (re-review after the section-8 fix; first pass was 99/100)
**Trend:** 99 → 100 (+1) after one iteration

The plan is single-purpose and ready to implement: add one userless Markdown file
beside the todo.txt, persist it through the same atomic write-then-swap discipline
as the task store, expose it as `GET`/`PUT /api/note` (the note analog of
`/api/raw`), and show it in a two-pane WebUI left of the unchanged todo UI. The one
first-pass Suggestion (a dirty-state decision split across two places) has been
folded in.

---

## First pass (99/100)

### 🟢 Suggestion — resolved

**Knowledge Duplication — note dirty-state decided in two places**
Symptom: The draft made "is the textarea dirty" (`els.noteInput.value !==
savedNote`) the single source for the Save button's `disabled` and the status text
inside `reflectNoteState`, but then `loadNote` re-expressed the same clean-state
rule inline (`els.noteSave.disabled = true; setNoteStatus("", false)`) instead of
deriving it. The one decision "what the controls show when the note is clean" lived
partly in `reflectNoteState` and partly in `loadNote`.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY: Don't Repeat Yourself (a
rule expressed in more than one place, free to drift).
Consequence: A later change to how a clean note presents (e.g., also clearing an
error class, or disabling a second control) would need coordinated edits in both
`reflectNoteState` and `loadNote` and could silently diverge — exactly the split
the `sort-filter-dropdowns` review flagged between `STATUSES` and `buildQuery`.
Remedy applied: Section 8 now routes all three callers (load, input, save) through
a single `refreshNote(cleanStatus)` that derives the button `disabled`, the status
message, and clean-vs-dirty from the one `value !== savedNote` check. The only
per-caller difference — the message shown when clean (`""` on load, `"Saved"` after
a save) — is passed in as a string (data, not a behavior-switching flag argument),
so the dirty rule has a single home and the same label-plus-behavior shape stays
consistent with the rest of the file.

## Remaining (100/100)

No further findings. Notes considered and dismissed:

- **Accidental Complexity — is `NoteStore` a Lazy Class? (dismissed):** it wraps a
  path + `String`, but adds real behavior — missing-file-as-empty on load and the
  atomic write-then-swap on `replace` — and keeps all filesystem access out of the
  API layer, mirroring `Store`. It is not a pure data holder or middle-man (R4 What
  Not to Flag: a thin module that owns a real policy is not a lazy class). Removing
  it would push `fs` calls into the handlers and break the existing layering.
- **Dependency Disorder — does `note` depend on the task store? (dismissed):** the
  atomic write is extracted into a leaf `atomicfile` module that neither store nor
  task imports, so both stores depend *down* onto a shared filesystem utility
  rather than the note depending sideways on the task store (R5: dependencies flow
  in one consistent direction). No cycle, no inversion.
- **Knowledge Duplication — two fetch helpers (`api` vs note fetches) (dismissed):**
  the JSON `api()` and the text `fetchNote`/`putNote` differ in body/reply format
  (JSON.stringify + res.json vs raw text), but the genuinely shared decision — how
  to extract the API's `{error}` message from a non-2xx response — is centralized in
  the new `throwIfNotOk` both call. The remaining per-format wrappers are thin and
  not duplicated knowledge.
- **Change Propagation — `AppState::new` / `Config` gain an argument/field
  (dismissed):** the 2→3 arg change to `AppState::new` and the new `Config.note_file`
  ripple to `main.rs` and the test harness, but that is the minimal, feature-required
  propagation of adding a second backing file, not shotgun surgery across unrelated
  modules. No existing task route, the task parser, or the store's behavior changes.
- **Coverage Illusion (not counted):** the WebUI note pane (dirty tracking,
  load/save) ships without an automated test. Constraint-bound — the repo has no JS
  test harness by the deliberate no-build-step decision, consistent with every prior
  WebUI feature. The Rust side is covered: `NoteStore` unit tests plus the
  `note_get_and_replace` API round-trip. Section 12 documents manual verification
  plus the green Rust gates. Not a deduction.

---

## Summary

After one iteration the plan clears the 95 gate at 100/100. The note's dirty state
now has a single source (`refreshNote` derives the button, the status, and
clean-vs-dirty from one check), the durable-write knowledge has one home
(`atomicfile::write`, shared by both stores with no cross-store coupling), and the
error-extraction logic is shared by the JSON and text fetch paths (`throwIfNotOk`).
Proceed to implementation.

---

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
