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


## Summary

After one iteration the plan clears the 95 gate at 100/100. The note's dirty state
now has a single source (`refreshNote` derives the button, the status, and
clean-vs-dirty from one check), the durable-write knowledge has one home
(`atomicfile::write`, shared by both stores with no cross-store coupling), and the
error-extraction logic is shared by the JSON and text fetch paths (`throwIfNotOk`).
Proceed to implementation.

