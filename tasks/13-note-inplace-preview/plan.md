# Plan: In-place single-surface live note editor (branch `note-inplace-preview`)

Replace the note pane's two-element view (a `<textarea>` beside a rendered `<div>`) with a
**single `contenteditable` surface** that is both the editor and the preview: you type raw
Markdown and it renders live in place — headings grow, `**bold**` shows bold, `-` lists
indent — while the **syntax markers stay visible and dimmed** so the text is still raw
Markdown you can edit and the file still saves as raw Markdown. This is the "in-place,
markers-kept" option the requester chose (over full marker-hiding WYSIWYG and over a
cosmetic two-pane merge).

It is the largest change in the note series: it deliberately reverses the
"no single-surface editing" non-goal that tasks 10–12 held for the no-build constraint,
because the request now asks for exactly that, and it does so **without** adding a library
or build step — a hand-written, character-preserving highlighter plus caret save/restore in
vanilla JS. The server, the API, the stored file format, and the task pane are untouched.

## 1. Goal and scope

Requirement (from `agents/ADD_NEW_FEATURE.md`):

> - Show preview on note-input. Combine "edit" and "preview" into single view

Chosen interpretation (confirmed with the requester): one editable surface where Markdown
renders as you type **with the markers kept** (e.g. `#` dimmed before a large heading,
`**` dimmed around bold text), not the marker-hiding Notion/Obsidian WYSIWYG. Keeping every
marker character is what makes this tractable in vanilla JS: the surface's text content
stays **exactly** the raw Markdown, so saving is trivial (read the text back out) and the
caret can be preserved across a re-render by a plain character offset.

In scope (WebUI only; the note is already stored/served as raw Markdown):

- `web/index.html` — replace the `#note-views` wrapper + `<textarea>` + `<div#note-preview>`
  with a single `<div id="note-input" contenteditable>`.
- `web/app.js` — (a) a new **character-preserving highlighter** `highlightMarkdown(src)`
  that wraps markers and content in styling spans while preserving every character;
  (b) **text I/O + caret** helpers (`readNoteText`, `getCaret`, `setCaret`, `setEditorText`)
  so the editor's logical text has one read path and one write path; (c) rewire the
  `input` handler, add `Enter` and `paste` interception and IME-composition guarding;
  (d) route autosave's "current text" through `readNoteText` instead of `.value`;
  (e) **delete** the task-10 semantic renderer (`renderMarkdown`, `renderInline`,
  `formatText`, `safeHref`, `blockKind`'s consumers `takeHeading/Fence/Blockquote/List/
  Paragraph`, the `TAKE` table) and `renderNotePreview` — they were only ever used by the
  removed side-by-side preview. `escapeHtml` and the block-pattern constants are kept and
  reused by the highlighter (section 3.2).
- `web/style.css` — the editor box (`.note-input` becomes the contenteditable surface), the
  `.md-*` line/marker styles, and a placeholder; remove `.note-views`, `.note-preview`, and
  the now-unused `.markdown-body` tag rules (the preview document is gone).
- `README.md` — describe the single in-place live editor.
- `tasks/13-note-inplace-preview/implemented.md` — after implementation.

Out of scope (explicit non-goals):

- **No marker-hiding WYSIWYG.** Markers stay visible/dimmed (the chosen option). No
  HTML→Markdown serialization engine, because the surface's text *is* the Markdown.
- **No library, no build step, no server-side rendering, no new endpoint.** `src/` untouched.
- **No change to the Markdown subset, the autosave protocol, the store, or the task pane.**
- **No multi-level nesting, tables, or images** — same subset task 10 supported, now shown
  in place rather than rendered to a separate document.

## 2. The model: text stays the source of truth

The one invariant the whole feature rests on:

> **The editor's logical text — every direct child line's `textContent`, joined by `"\n"` —
> is exactly the raw Markdown.** Highlighting only *wraps* characters in `<span>`s; it never
> adds, removes, or reorders a character.

Consequences that fall out of this invariant, each removing what would otherwise be a hard
sub-problem:

- **Saving** is `readNoteText()` — no serialization. The PUT body is identical to today's.
- **Caret preservation** across a re-render is a single integer: the count of characters
  (including the `"\n"` between lines) before the caret. Because the characters are
  unchanged by re-rendering, the same offset points at the same place.
- **XSS** reduces to escaping: every text run is `escapeHtml`-ed before it becomes
  innerHTML, exactly as task 10's renderer did. We never build an `<a href>` (links render
  as dimmed-bracket styled text, not clickable anchors), so there is no href sink and
  `safeHref` is dropped.

## 3. `web/app.js`

### 3.1 Editor text I/O and caret (the one read/write path)

```js
// THE LINE INVARIANT (one home for the "lines join with a single \n" decision):
// the editor's logical text is each direct-child line's textContent joined by exactly one
// "\n". readNoteText performs that join; getCaret/setCaret count this same per-line length
// via lineLen, so they are exact inverses of the join and cannot drift. Change the
// separator here and the caret math follows from lineLen — there is no second copy.
const lineLen = (line) => line.textContent.length + 1; // +1 = the joining "\n"

// The editor's logical text — the raw Markdown, since highlighting only wraps characters,
// never changes them. What autosave reads and what the caret offsets count against.
function readNoteText(root) {
  return Array.from(root.children).map((line) => line.textContent).join("\n");
}

// Render `text` into the editor and (optionally) restore the caret to character offset
// `caret`. The single write path: load, every keystroke, Enter, and paste all go through
// here, so "what the editor shows" and "where the caret is" have one home.
function setEditorText(text, caret) {
  els.noteInput.innerHTML = highlightMarkdown(text);
  els.noteInput.classList.toggle("is-empty", text === "");
  if (caret != null) setCaret(els.noteInput, caret);
}
```

Caret as a logical character offset, measured/placed consistently with `readNoteText` (each
line = its `textContent.length`, plus one for the joining `"\n"`):

```js
// Characters before the caret, counting one "\n" per line boundary — the inverse of the
// readNoteText join, so an offset survives a re-render unchanged.
function getCaret(root) {
  const sel = window.getSelection();
  if (!sel.rangeCount) return null;
  const range = sel.getRangeAt(0);
  if (!root.contains(range.startContainer)) return null;
  let offset = 0;
  for (const line of root.children) {
    if (line === range.startContainer || line.contains(range.startContainer)) {
      const within = document.createRange();
      within.setStart(line, 0);
      within.setEnd(range.startContainer, range.startOffset);
      return offset + within.toString().length; // no block boundaries inside a line
    }
    offset += lineLen(line); // same per-line length the join implies (THE LINE INVARIANT)
  }
  return offset;
}

// Place the caret at logical offset `pos`, walking lines then text nodes within the target
// line. An empty line (just <br>) has no text node, so the caret sits at (line, 0).
function setCaret(root, pos) {
  for (const line of root.children) {
    if (pos <= line.textContent.length) return placeInLine(line, pos);
    pos -= lineLen(line); // step over this line and its joining "\n" (THE LINE INVARIANT)
  }
  placeInLine(root.lastElementChild, root.lastElementChild.textContent.length);
}
```

`placeInLine` walks the line's descendant text nodes accumulating lengths to find the node
and local offset (a short TreeWalker loop); within a line `range.toString().length` equals
the character count because a line contains only inline spans, no block boundaries. This is
the crux that makes the offset exact.

### 3.2 The highlighter (character-preserving)

`escapeHtml` is **kept and reused**. The block-pattern constants (`LIST_ITEM`,
`ORDERED_ITEM`, `FENCE`, `QUOTE_MARKER`, and the heading test) are kept; `blockKind` is
kept and reused to classify each non-fence line, so "what kind of line is this" still has
one home (no second classifier — R3). New code is the marker-keeping wrapping.

Two single-level functions, mirroring task 10's `renderInline`/`formatText` split so each
stays readable and the marker-preservation regexes are hand-verified in isolation (adopted
plan-review R1): `highlightInline` only partitions code spans from the rest;
`formatInline` only wraps emphasis and links on a non-code run. Both keep every character.

```js
// Partition one already-escaped line on `code` spans (odd segments), keeping the backticks
// as dimmed marks; non-code segments (even) go to formatInline. Mirrors renderInline but
// preserves the markers. One job: separate code from non-code.
function highlightInline(escaped) {
  return escaped
    .split(/(`[^`]+`)/)
    .map((part, i) => (i % 2 ? markCode(part) : formatInline(part)))
    .join("");
}

// Wrap emphasis and links on a non-code escaped run, re-emitting every original marker
// character inside an .md-mark span so textContent === the source run. Order mirrors task
// 10 (links, then bold, then italic) so ** is not consumed by the single-* rule. One job:
// style a non-code run. Each replacement reinserts the literal markers via capture groups,
// e.g. **x** -> <span class=md-mark>**</span><span class=md-strong>x</span><span class=md-mark>**</span>.
function formatInline(run) { … }

// One source line -> one block <div class="md-line md-KIND">…</div>. KIND drives block
// styling (heading size, quote rule, list indent, code mono). Empty line -> <br> so it has
// height and is selectable. fenceState carries across lines so fenced-code body lines style
// as code regardless of their content.
function highlightLine(line, kind) { … }

// src -> HTML string, one block per line, every character preserved. Used only by
// setEditorText (the write path).
function highlightMarkdown(src) {
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  let inFence = false;
  return lines
    .map((line) => {
      const fence = FENCE.test(line);
      const kind = inFence || fence ? "code" : blockKind(line);
      if (fence) inFence = !inFence;
      return highlightLine(line, kind);
    })
    .join("");
}
```

Marker dimming is purely a `<span class="md-mark">` wrap; the heading/quote/list/code class
goes on the line div. Because `blockKind` returns `"blank"` for empty lines, `highlightLine`
emits `<div class="md-line"><br></div>` for those.

### 3.3 Input, Enter, paste, IME

The browser handles ordinary typing/deletion (so native caret, undo, and IME keep working);
we re-render after each `input` to refresh the highlight, restoring the caret by offset.
Two structural inputs are intercepted so they operate on the logical text (avoiding the
nested-block DOM the browser would otherwise create):

```js
let composingNote = false; // true mid-IME so we don't re-render and break composition

els.noteInput.addEventListener("compositionstart", () => { composingNote = true; });
els.noteInput.addEventListener("compositionend", () => {
  composingNote = false;
  rerenderNote(); // highlight the just-committed composed text once
});

els.noteInput.addEventListener("input", () => {
  setNoteStatus("Editing…", false);
  scheduleNoteSave();
  if (!composingNote) rerenderNote();
});

// Re-highlight in place, preserving the caret. Reads the current text, re-renders, restores.
function rerenderNote() {
  setEditorText(readNoteText(els.noteInput), getCaret(els.noteInput));
}

// Enter and paste are reimplemented on the logical text so structure stays flat (one div
// per line). Enter splices a "\n" at the caret; paste splices the plain-text clipboard
// (also neutralising any pasted rich HTML — only text/plain ever enters the editor).
els.noteInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") { e.preventDefault(); spliceNote("\n"); }
});
els.noteInput.addEventListener("paste", (e) => {
  e.preventDefault();
  spliceNote(e.clipboardData.getData("text/plain"));
});

function spliceNote(insert) {
  const text = readNoteText(els.noteInput);
  const at = getCaret(els.noteInput) ?? text.length;
  setEditorText(text.slice(0, at) + insert + text.slice(at), at + insert.length);
  setNoteStatus("Editing…", false);
  scheduleNoteSave();
}
```

This keeps the DOM canonical (flat lines) for the two operations that would otherwise nest
blocks; a backspace that merges two lines simply removes a line div (one fewer `"\n"`),
which `readNoteText` reads correctly, and the next re-render re-canonicalises. The residual
case — a delete/paste edge that leaves a transient nested block for a single keystroke — is
self-healing on the next `input` and is called out in section 7.

### 3.4 Autosave and load rewiring

`els.noteInput.value` no longer exists (the element is a div). The three autosave readers
and the loader switch to the one read/write path:

- `noteDirty`: `readNoteText(els.noteInput) !== savedNote`.
- `flushNote`: `const text = readNoteText(els.noteInput);`.
- `flushNoteBeacon`: `putNote(readNoteText(els.noteInput), { keepalive: true })`.
- `loadNote`: `setEditorText(text, null);` (render the loaded note; no caret to restore yet)
  replacing `els.noteInput.value = text;` **and** the now-deleted `renderNotePreview()` call.

The debounce, single-flight, post-save re-check, blur flush, and pagehide beacon are
otherwise unchanged — they already route through `noteDirty`/`flushNote`, so giving those a
new text source is the whole change.

### 3.5 Deletions

Remove (only consumer was the deleted preview): `renderNotePreview`, `renderMarkdown`,
`takeParagraph`, `takeList`, `takeBlockquote`, `takeFence`, `takeHeading`, the `TAKE` table,
`renderInline`, `formatText`, `safeHref`. Keep `escapeHtml`, `LIST_ITEM`, `ORDERED_ITEM`,
`FENCE`, `QUOTE_MARKER`, and `blockKind` (reused by the highlighter). Remove the
`noteViews`/`notePreview` handles from `els`; keep `noteInput` and `noteStatus`.

## 4. `web/index.html`

```html
<section class="pane note-pane">
  <h1>note</h1>
  <div
    id="note-input"
    class="note-input"
    contenteditable="true"
    role="textbox"
    aria-multiline="true"
    aria-label="Note (Markdown)"
    data-placeholder="Notes (Markdown)…"
  ></div>
  <div class="note-bar">
    <span id="note-status" class="note-status" role="status"></span>
  </div>
</section>
```

The `#note-views` wrapper, the `<textarea>`, and the `#note-preview` div are gone; the
single contenteditable div keeps the id `note-input` so the autosave wiring keeps the same
element handle. `role="textbox"` + `aria-multiline` keep it announced as a multi-line text
field; `data-placeholder` drives the CSS placeholder (section 5).

## 5. `web/style.css`

- `.note-input` — the editor box: keep today's border, radius, padding, `min-height: 60vh`,
  background, and monospace font (markers read clearly in mono); add `white-space: pre-wrap`
  (newlines and spaces render as typed, long lines wrap) and `overflow: auto`. A `:focus`
  outline for visible focus.
- Placeholder: `.note-input.is-empty::before { content: attr(data-placeholder); color:
  var(--muted); pointer-events: none; }` — shown only when the logical text is empty (the
  `is-empty` class is toggled in `setEditorText`), since a contenteditable is never CSS
  `:empty` once it holds an empty line div.
- `.md-mark { opacity: 0.45; }` — dimmed markers (`#`, `**`, `*`, `` ` ``, `>`, list
  bullets, brackets/parens).
- `.md-line` block styling by kind, mirroring the former preview typography but on per-line
  divs: `.md-h1`…`.md-h6` (font-size/weight), `.md-quote` (left rule + muted), `.md-code`
  (mono + subtle background), `.md-li` (indent), `.md-hr` (the rule line). Heading sizes
  reuse the existing `--accent`/spacing tokens.
- Remove `.note-views`, `.note-views.is-split`, the `@media .note-views` rule, `.note-preview`,
  and the `.markdown-body …` tag rules (their only element, the preview, is gone). The
  narrow-screen `.panes` stack rule is untouched (it is about the two outer panes).

No server-rendered HTML uses `.markdown-body` anymore, so removing those rules drops dead
styling rather than changing any surviving element.

## 6. `README.md`

Rewrite the note-pane paragraph: the note pane is now a **single in-place live editor** —
one surface where you type raw Markdown and it renders as you type (headings, bold/italic,
inline and fenced code, one-level lists, blockquotes, rules, links) with the syntax markers
kept visible so the text stays editable; the file still saves as raw Markdown. Keep the
autosave paragraph unchanged. Drop the "Edit/Live/Preview" and side-by-side wording.

## 7. Tests and verification

No new automated test: the editor is client-side JS and the repo has no JS test harness by
its deliberate no-build decision (the same standing position as tasks 10–12). `src/` is
untouched, so the Rust suite still passes as a regression guard and `cargo build` re-embeds
the edited assets via `include_str!` (a malformed `index.html` fails the build). The
highlighter is pure (string in, HTML string out) and hand-traced like task 10's renderer.

- **Rust gates:** `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check` green.
- **Manual (`cargo run`, open `http://127.0.0.1:8080`):**
  - The note pane is one box (no tabs, no second pane), populated from `Note.md` and
    rendered: a `# H1` line is large with a dimmed `#`; `**bold**` shows bold with dimmed
    `**`; a `-` list indents with a dimmed marker; a fenced block styles as code.
  - Type in the middle of a heading: the caret stays put across the live re-render; keep
    typing — no caret jump, no reversed characters.
  - Press **Enter** mid-line: the line splits, caret lands at the new line start.
  - **Backspace** at a line start merges into the previous line (one fewer line), caret at
    the join.
  - **Paste** multi-line rich text: only the plain text lands, as new lines; no pasted
    markup survives in the DOM.
  - **IME** (if available): compose a CJK/accented string — characters are not re-rendered
    mid-composition and are not duplicated/reversed; the highlight refreshes on commit.
  - **Autosave:** status shows `Editing…`/`Saving…`/`Saved`; reload shows the saved text,
    re-rendered, byte-identical to what was typed (confirms read path == raw Markdown).
  - **Security:** type `<script>alert(1)</script>` and `[x](javascript:alert(1))` — both
    show as literal dimmed/plain text, no script runs, no `<a>` is created.
  - Empty the note: the placeholder shows; below 720px the pane still fits (single column).

## 8. Risks

- **Caret correctness across re-render — the central risk.** The whole feel depends on
  `getCaret`/`setCaret` agreeing with `readNoteText`. Mitigated by deriving all three from
  the one invariant (section 2: line `textContent` joined by `"\n"`), measuring within a
  line with a DOM `Range` (exact because a line has no block boundaries), and the manual
  caret checklist. If an edge ever mis-seats the caret, it is a one-keystroke visual glitch,
  not data loss — the text is always read correctly.
- **Browser-inserted nested blocks on Enter/delete.** contenteditable Enter normally nests
  `<div>`s, which would break the flat one-line-per-child model. Mitigated by intercepting
  Enter and paste to splice the logical text directly (section 3.3); the only structural op
  left to the browser is delete-merge, which reduces child count consistently, and any
  transient nesting self-heals on the next `input` re-render. Called out as the residual
  edge.
- **IME composition fighting the re-render.** Re-rendering mid-composition would drop or
  duplicate composed characters. Mitigated by the `composingNote` guard (skip re-render
  between `compositionstart` and `compositionend`, render once on end).
- **Per-keystroke render cost.** Every input rebuilds `innerHTML` for the whole note. For a
  personal note this is imperceptible (`highlightMarkdown` is one O(n) pass); if a
  pathologically large note ever lagged, the fix is a localised `requestAnimationFrame`
  coalesce in `rerenderNote`. Same accepted tradeoff as tasks 11–12.
- **XSS via innerHTML.** Every text run is `escapeHtml`-ed before wrapping; no `<a href>` is
  ever built and `paste` admits only `text/plain`, so neither a crafted note nor a paste can
  inject markup. The manual checklist re-confirms.
- **Native undo (Ctrl-Z).** Re-rendering `innerHTML` can truncate the browser's native undo
  stack. Accepted for a single autosaved personal note (autosave is the durability story,
  not undo); noted as a known limitation, not addressed here.
- **Deleting task 10's renderer.** `renderMarkdown` and its helpers are removed; a grep
  confirms `renderNotePreview` was their only caller, so this is dead-code removal, not a
  behaviour change. The block-pattern constants and `blockKind` are reused, so block
  classification keeps one home.

## 9. Why in-place markers-kept, vanilla, not a library or full WYSIWYG

- **Conceptual integrity.** The note is the canonical *raw* store. Keeping the markers means
  the editor's text *is* the store — one representation, no serialization layer, no second
  source of truth. This is what makes a single-surface editor tractable here at all.
- **No-build / no-dependency constraint, still honoured.** A marker-hiding WYSIWYG needs a
  real editing engine (HTML↔Markdown, marker-reveal on caret entry, selection/IME/undo) —
  effectively a Markdown editor framework and a build step. The markers-kept model gets the
  "renders as I type, one box" outcome with a pure highlighter and a character-offset caret,
  no library. This is the smallest faithful implementation of the chosen option.
- **Cost.** The tradeoffs are the per-keystroke render and native-undo truncation
  (section 8), both accepted as immaterial for a single personal note. A library would add a
  build step the repo forbids; full WYSIWYG would add far more code and risk for a one-user
  note pane.

## 10. Files touched

- `web/app.js` — new `highlightMarkdown`/`highlightInline`/`formatInline`/`highlightLine`
  (+ the `markCode`/`.md-mark` helpers); text+caret helpers `lineLen`/`readNoteText`/
  `getCaret`/`setCaret`/`placeInLine`/`setEditorText`; `rerenderNote`
  + the input/Enter/paste/IME handlers + `spliceNote`; autosave/`loadNote` rewired to
  `readNoteText`/`setEditorText`; deletion of the task-10 semantic renderer and
  `renderNotePreview`; `els` loses `noteViews`/`notePreview`. `escapeHtml`, the block
  patterns, and `blockKind` kept and reused.
- `web/index.html` — single `<div id="note-input" contenteditable>` replacing the views
  wrapper + textarea + preview (section 4).
- `web/style.css` — editor box + `.md-*` line/marker styles + placeholder; removal of
  `.note-views*`, `.note-preview`, and `.markdown-body` tag rules (section 5).
- `README.md` — single in-place live editor description (section 6).
- `tasks/13-note-inplace-preview/implemented.md` — after implementation.

## 11. brooks-review outcome

`tasks/13-note-inplace-preview/plan-review.md` scored the first draft 98/100 with two 🟢
Suggestions, both adopted:
- **R3 (Knowledge Duplication):** the "lines join with one `\n`" decision was encoded three
  times (`readNoteText`'s join and the `+1` in both `getCaret` and `setCaret`). Adopted:
  section 3.1 now states THE LINE INVARIANT in one place and routes both caret functions
  through a single `lineLen(line)` helper, so the separator decision has one home and the
  inverses cannot drift.
- **R1 (Cognitive Overload):** the inline highlighter was sketched as one dense multi-pass
  function. Adopted: section 3.2 now splits it into `highlightInline` (code-span partition)
  and `formatInline` (emphasis + links), mirroring task 10's `renderInline`/`formatText`
  shape so each stays at one level of abstraction.

The re-review found no further issues (100/100). The only standing note is the
constraint-bound absence of a JS test harness for the client editor — the same no-build
decision tasks 10–12 accepted; the highlighter is pure and hand-traced, `escapeHtml` and
`blockKind` are reused unchanged, and the server half stays covered by the Rust suite.
