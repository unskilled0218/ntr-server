# Review-fixes implemented (branch `note-inplace-preview`)

Applied the three 🟢 findings from `code-review.md` per `review-plan.md`. All in
`web/app.js`; the highlighter's output HTML is unchanged.

## Changes

- **Fix 1 (R4):** replaced the 7-branch `highlightLine` if-ladder with a keyed `LINE`
  dispatch table (`{ blank, code, hr, heading, quote, list, para }` of `(line) => html`),
  mirroring task 10's `TAKE`; `highlightMarkdown` now calls `LINE[kind](line)`.
- **Fix 2 (R3):** factored the four duplicated emphasis callbacks in `formatInline` into a
  shared `emph(marker, cls)` helper; the link branch is unchanged.
- **Fix 3 (R4):** dropped the redundant `&& line.startsWith(">")` from `blockKind`'s quote
  branch (`QUOTE_MARKER` is `^>`-anchored).

## Verification

- Node round-trip + XSS harness, now 13 cases (added `__under__ and _emph_`): 13 ok, 0 fail;
  no raw `<script>` tag, no `href` — output HTML identical to pre-refactor.
- `node --check web/app.js` passes.
- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` (8 passed) all green.

## Outcome

Code re-review after the fixes: 100/100, no findings. The three idiom-restoring refactors
preserved behaviour and the verified char-preservation invariant.
