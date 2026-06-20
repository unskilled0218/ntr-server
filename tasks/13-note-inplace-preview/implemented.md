# Implemented: In-place single-surface live note editor (branch `note-inplace-preview`)

Replaced the note pane's two-element view (textarea + separate preview div) with a single
`contenteditable` surface that is both editor and preview: you type raw Markdown and it
renders live in place — headings grow, `**bold**` shows bold, `-` lists indent — with the
syntax markers kept visible and dimmed so the text stays editable raw Markdown and the file
still saves byte-identical. Matches `plan.md`; both plan-review 🟢s (the `lineLen` invariant
helper and the `highlightInline`/`formatInline` split) were adopted during planning and are
in the code.

## Changes

### `web/index.html`
- Replaced the `#note-views` wrapper + `<textarea#note-input>` + `<div#note-preview>` with a
  single `<div id="note-input" class="note-input" contenteditable="true" role="textbox"
  aria-multiline="true" aria-label="Note (Markdown)" data-placeholder="Notes (Markdown)…">`.

### `web/app.js`
- **Removed** `els.notePreview` (the only remaining note handle besides `noteInput`/
  `noteStatus`).
- **Deleted** task 10's semantic renderer — `renderMarkdown`, `renderInline`, `formatText`,
  `safeHref`, `takeHeading/Fence/Blockquote/List/Paragraph`, the `TAKE` table — and
  `renderNotePreview`. Their only caller was the removed side-by-side preview.
- **Kept and reused** `escapeHtml` and `blockKind`; the block-pattern constants are kept,
  with `LIST_ITEM`/`QUOTE_MARKER` extended (and a new `HEADING`) to also capture their parts
  for the highlighter.
- **Added the character-preserving highlighter:** `mark` (the one place marker spans are
  built), `formatInline` (emphasis + links, markers kept), `highlightInline` (code-span
  partition), `highlightLine` (one source line → one `<div class="md-line md-KIND">`), and
  `highlightMarkdown` (per-line, tracking fenced-code state). Every character of the source
  is preserved; only styling spans are added.
- **Added the text/caret layer:** `lineLen` (THE LINE INVARIANT — one home for "lines join
  with a single `\n`"), `readNoteText` (the join), `getCaret`/`setCaret`/`placeInLine`
  (logical char-offset save/restore, inverse of the join via `lineLen`), `setEditorText`
  (the single write path; toggles `is-empty` for the placeholder), `rerenderNote`, and
  `spliceNote`.
- **Rewired input:** the `input` handler re-highlights in place (`rerenderNote`, caret
  preserved); `keydown` Enter and `paste` are intercepted and routed through `spliceNote`
  on the logical text so the DOM stays flat (paste also admits only `text/plain`);
  `compositionstart`/`compositionend` guard against re-rendering mid-IME.
- **Rewired autosave/load:** `noteDirty`, `flushNote`, `flushNoteBeacon` now read
  `readNoteText(els.noteInput)` instead of `.value`; `loadNote` renders via
  `setEditorText(text, null)`.

### `web/style.css`
- `.note-input` is now the contenteditable editor box (border/padding/min-height kept; added
  `white-space: pre-wrap`, `overflow-wrap: anywhere`, `overflow: auto`, a `:focus` outline);
  added the `.note-input.is-empty::before` placeholder.
- Added `.md-mark` (dimmed markers) and the `.md-*` line/inline styles (`.md-h1`…`.md-h6`,
  `.md-strong`, `.md-em`, `.md-link`/`.md-url`, `.md-code-inline`, `.md-code`, `.md-li`,
  `.md-quote`, `.md-hr`).
- Removed `.note-views*`, the `@media .note-views` rule, `.note-preview`, and all
  `.markdown-body …` rules (their only element, the preview, is gone).

### `README.md`
- Rewrote the note-pane paragraph to describe the single in-place live editor (markers kept,
  saves as raw Markdown); kept the Markdown-subset list and the autosave paragraph.

## Verification

- **Character-preservation invariant (the core of the feature), tested in Node** against the
  extracted pure highlighter: for 12 inputs — headings, mixed bold/italic, indented and
  ordered lists, blockquote with inline code, a fenced block containing `<b> & 'stuff'`, a
  link whose URL has underscores, raw HTML-significant chars, `---`, empty, trailing blank
  lines, and a `javascript:` link — stripping the rendered HTML's tags and decoding entities
  reproduces the source exactly (12 ok, 0 fail). This is the invariant saving and caret math
  depend on.
- **XSS:** `highlightMarkdown("<script>…</script> [x](javascript:…)")` produces no raw
  `<script>` tag and no `href` attribute (links render as styled text, never anchors).
- **JS parse:** `node --check web/app.js` passes (cargo embeds the file as a string, so this
  is the guard against a JS syntax error).
- **Rust gates:** `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` (8 passed)
  all green; `cargo build` re-embeds the edited assets.
- **Runtime smoke:** server boots and `/` serves the single contenteditable editor div (no
  textarea, no preview div, no tabs).
- **Dangling-reference grep:** no surviving references to the removed symbols
  (`renderMarkdown`/`renderNotePreview`/`note-preview`/`markdown-body`/etc.); the remaining
  `.value` hits are unrelated task-form selects/inputs.

## Needs a real browser (manual checklist in plan section 7)

The caret/IME paths are DOM-dependent and not covered by the Node test (the repo has no JS
test harness — the standing no-build constraint). To confirm in a browser: caret stays put
while typing across the live re-render; Enter splits a line; backspace merges; multi-line
rich paste lands as plain text; IME composition is not duplicated/reversed; reload shows the
saved text byte-identical. The pure invariant test above covers the rendering correctness
these paths build on.

## Not done / out of scope (unchanged from plan)

- No marker-hiding WYSIWYG; markers stay visible/dimmed (the chosen option). No HTML→Markdown
  serialization — the editor's text *is* the Markdown.
- No library, no build step, no server-side rendering, no new endpoint. `src/` untouched.
- Native Ctrl-Z undo may be truncated by the per-keystroke re-render — accepted for a single
  autosaved personal note (a known limitation noted in plan section 8).
