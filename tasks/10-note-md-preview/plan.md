# Plan: Markdown preview for the note pane (branch `note-md-preview`)

Give the note pane an **Edit / Preview** toggle so the raw `Note.md` Markdown can be
read rendered — headings, bold/italic, lists, code, quotes, links — the way Obsidian's
reading view renders a note. The rendering is done by a small, dependency-free,
no-build vanilla-JS Markdown renderer, keeping the WebUI's existing
no-build/embedded-asset design intact.

## 1. Goal and scope

Requirement (from `agents/ADD_NEW_FEATURE.md`):

> - preview note.md like notion or obsidian

The original `note-md-support` plan listed "No Markdown rendering / preview" as a
non-goal precisely because rendering "would need a client-side Markdown library or a
build step, against the repo's deliberate no-build, vanilla-embedded-asset design."
This feature satisfies the request **without** breaking that constraint: instead of a
library or a build step, it adds a small hand-written renderer in the existing vanilla
`app.js`. No npm, no bundler, no new server dependency.

Interpretation of "like notion or obsidian": Obsidian's **reading view** is a mode
toggle — the same note is shown either as editable raw Markdown or as rendered output.
That is the faithful, achievable analog here. Notion-style inline WYSIWYG block editing
is a fundamentally different, far larger thing and is an explicit non-goal (section 9).

In scope (WebUI only — the note is already stored and served as raw Markdown):

- `web/app.js` — a pure `renderMarkdown(src)` renderer (with small helpers
  `escapeHtml`, `safeHref`, `renderInline`, `blockKind`, and per-block `takeX`
  consumers) and an Edit/Preview mode toggle (`setNoteMode`). Autosave is untouched.
- `web/index.html` — a small two-button tab group (Edit / Preview) in the note pane
  header and a `#note-preview` render target beside the existing `#note-input`.
- `web/style.css` — the tab styling and a `.markdown-body` typographic stylesheet for
  the rendered output (Obsidian/Notion-like clean reading style).
- `README.md` — document the toggle and the supported Markdown subset.
- `implemented/note-md-preview.md` — written after implementation.

Out of scope (explicit non-goals — section 9 expands the rationale):

- **No server-side rendering and no new endpoint.** The note stays a raw-text store;
  rendering is presentation and lives on the client, exactly like task-meta rendering,
  sorting, and filtering already do. The server keeps its "canonical raw store" role.
- **No third-party Markdown library and no build step.** A focused subset renderer in
  vanilla JS, consistent with every other WebUI feature.
- **No Notion-style WYSIWYG / contenteditable block editing**, no live side-by-side
  preview that re-renders per keystroke, no tables / images / footnotes / embedded raw
  HTML / nested lists (the subset is enumerated in section 3.4 and documented in the
  README so its limits are explicit, not surprising).
- **No change to autosave or to the task pane.**

## 2. Layout and interaction

The note pane header becomes a row: the `note` title on the left, an **Edit | Preview**
tab group on the right. Below it, two stacked views share the pane — the existing
`<textarea>` (Edit) and a new rendered `<div>` (Preview) — exactly one visible at a
time via the `hidden` attribute. The autosave status bar stays below, visible in both
modes.

- Default mode is **Edit** (the textarea), so the pane opens exactly as it does today.
- Clicking **Preview** renders the textarea's *current* value into `#note-preview` and
  shows it; clicking **Edit** hides the preview and refocuses the textarea.
- Rendering happens **on toggle**, not per keystroke — Obsidian's reading-view model,
  and it avoids parsing on every keypress.
- Switching to Preview blurs the textarea, which (via the existing autosave `blur`
  handler) flushes a save first; the render reads the live textarea value regardless,
  so the preview always reflects exactly what is on screen.

## 3. `web/app.js` — the renderer

The renderer is a set of **pure functions**: text in, HTML string out, no DOM reads or
writes. That keeps it trivially reasoned-about and isolates the one place that touches
the DOM (`setNoteMode`) from the parsing logic.

### 3.1 Safety first — escaping and link sanitisation

Everything user-typed is HTML-escaped before it can reach `innerHTML`, and link targets
are sanitised, so a note can never inject markup or a `javascript:` URL into the page
(the app is userless/loopback, but rendering untrusted-shaped text to `innerHTML`
should be safe by construction regardless):

```js
// Escape the HTML-significant characters so note text can never inject markup into the
// rendered preview. Every user-supplied run passes through here before it reaches
// innerHTML (block builders escape text; only the tags this file emits are literal).
function escapeHtml(s) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

// Allow only safe link targets — http(s), mailto, and relative/anchor links. Anything
// else (notably javascript:) collapses to "#", so a crafted link cannot run script.
function safeHref(url) {
  const u = url.trim();
  if (/^(https?:|mailto:)/i.test(u)) return u;
  if (/^[/#.]/.test(u)) return u; // relative path or in-page anchor
  return "#";
}
```

### 3.2 Inline formatting

Inline rendering runs on an **already-escaped** text run. Code spans are split out
first so their contents stay literal (no bold/italic/link processing inside code); the
remaining text gets links, then bold, then italic (bold before italic so `**` is not
eaten by the single-`*` rule):

```js
// Inline emphasis/links/code for one already-escaped text run. Splitting on backtick
// code spans (odd indices) keeps code literal; even indices get the rest of the inline
// formatting. Returns an HTML string.
function renderInline(escaped) {
  return escaped
    .split(/`([^`]+)`/)
    .map((part, i) => (i % 2 ? `<code>${part}</code>` : formatText(part)))
    .join("");
}

// Links, then bold, then italic, on a non-code escaped run. Links are resolved through
// safeHref and open in a new tab with noopener; bold runs before italic so ** is not
// consumed by the * rule.
function formatText(s) {
  return s
    .replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      (_, t, u) => `<a href="${safeHref(u)}" target="_blank" rel="noopener noreferrer">${t}</a>`,
    )
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/__([^_]+)__/g, "<strong>$1</strong>")
    .replace(/\*([^*]+)\*/g, "<em>$1</em>")
    .replace(/_([^_]+)_/g, "<em>$1</em>");
}
```

### 3.3 Block classification — one source of truth

The block line-patterns that are needed in more than one place — the list marker, the
ordered-list marker, and the fenced-code delimiter — are named once as module constants
so each "what marks this block" decision has a single home (none carry the `g` flag, so
reusing them with `.test` across the parse loop is stateless and safe):

```js
const LIST_ITEM = /^\s*([-*+]|\d+\.)\s+/; // any list item (the leading marker)
const ORDERED_ITEM = /^\s*\d+\.\s+/; //      an ordered list item specifically
const FENCE = /^```/; //                     a fenced-code delimiter line
```

A single `blockKind(line)` then decides what kind of block a line starts. The main loop
dispatches on it, and `takeParagraph` consumes lines while they are still `"para"` — so
the "what block is this?" decision lives in exactly one place (no duplicated
line-pattern knowledge split between the dispatcher and the paragraph terminator):

```js
// Classify the block a line begins. The single source of "what kind of line is this":
// the render loop dispatches on it and takeParagraph stops when a line stops being
// "para", so the block-detection patterns are never duplicated.
function blockKind(line) {
  if (line.trim() === "") return "blank";
  if (FENCE.test(line)) return "fence";
  if (/^#{1,6}\s/.test(line)) return "heading";
  if (/^(---|\*\*\*|___)\s*$/.test(line)) return "hr";
  if (/^>\s?/.test(line)) return "quote";
  if (LIST_ITEM.test(line)) return "list";
  return "para";
}
```

### 3.4 The block renderer

Each block consumer takes `(lines, start)` and returns `{ html, next }` — the rendered
HTML and the index to resume from — so the main loop is a uniform dispatch with no
shared mutable accumulator passed around:

```js
// Render a Markdown subset to an HTML string. Supported: headings (#..######), bold,
// italic, inline code, fenced code blocks, blockquotes, unordered/ordered lists (one
// level), horizontal rules, links, and paragraphs with single-line breaks. Unsupported
// constructs (tables, images, nested lists, raw HTML) render as their literal text —
// see the README. Pure: string in, HTML string out.
function renderMarkdown(src) {
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  const out = [];
  let i = 0;
  while (i < lines.length) {
    const kind = blockKind(lines[i]);
    if (kind === "blank") { i += 1; continue; }
    const block = TAKE[kind](lines, i);
    out.push(block.html);
    i = block.next;
  }
  return out.join("\n");
}

// Block consumers keyed by blockKind, so renderMarkdown stays a flat dispatch.
const TAKE = {
  fence: takeFence,
  heading: takeHeading,
  hr: (lines, i) => ({ html: "<hr>", next: i + 1 }),
  quote: takeBlockquote,
  list: takeList,
  para: takeParagraph,
};

function takeHeading(lines, i) {
  const [, hashes, text] = lines[i].match(/^(#{1,6})\s+(.*)$/);
  const level = hashes.length;
  return { html: `<h${level}>${renderInline(escapeHtml(text))}</h${level}>`, next: i + 1 };
}

function takeFence(lines, i) {
  let j = i + 1;
  const body = [];
  while (j < lines.length && !FENCE.test(lines[j])) { body.push(lines[j]); j += 1; }
  if (j < lines.length) j += 1; // consume the closing fence
  // Code is escaped but NOT inline-formatted — it is literal.
  return { html: `<pre><code>${escapeHtml(body.join("\n"))}</code></pre>`, next: j };
}

function takeBlockquote(lines, i) {
  let j = i;
  const buf = [];
  while (j < lines.length && blockKind(lines[j]) === "quote") {
    buf.push(lines[j].replace(/^>\s?/, ""));
    j += 1;
  }
  const inner = renderInline(escapeHtml(buf.join("\n"))).replace(/\n/g, "<br>");
  return { html: `<blockquote>${inner}</blockquote>`, next: j };
}

function takeList(lines, i) {
  const ordered = ORDERED_ITEM.test(lines[i]);
  const items = [];
  let j = i;
  // One list = consecutive list lines of the same kind (ordered vs unordered); a switch
  // starts a new list. One level only — indentation is not nested.
  while (
    j < lines.length &&
    blockKind(lines[j]) === "list" &&
    ORDERED_ITEM.test(lines[j]) === ordered
  ) {
    const text = lines[j].replace(LIST_ITEM, "");
    items.push(`<li>${renderInline(escapeHtml(text))}</li>`);
    j += 1;
  }
  const tag = ordered ? "ol" : "ul";
  return { html: `<${tag}>${items.join("")}</${tag}>`, next: j };
}

function takeParagraph(lines, i) {
  let j = i;
  const buf = [];
  while (j < lines.length && blockKind(lines[j]) === "para") { buf.push(lines[j]); j += 1; }
  const text = renderInline(escapeHtml(buf.join("\n"))).replace(/\n/g, "<br>");
  return { html: `<p>${text}</p>`, next: j };
}
```

Every block consumer is short, single-purpose, and named for what it consumes; the main
loop is a ~9-line dispatcher. No function exceeds ~10 lines or nests past two levels.

### 3.5 The mode toggle (the one DOM-touching piece)

```js
// Show one of the note's two views. Entering Preview renders the textarea's CURRENT
// text (so it always matches what was on screen) into #note-preview; Edit refocuses the
// textarea. aria-pressed marks the active tab. renderMarkdown's output is already
// escaped/sanitised, so assigning it to innerHTML is safe.
function setNoteMode(mode) {
  const preview = mode === "preview";
  if (preview) els.notePreview.innerHTML = renderMarkdown(els.noteInput.value);
  els.noteInput.hidden = preview;
  els.notePreview.hidden = !preview;
  els.noteEditTab.setAttribute("aria-pressed", String(!preview));
  els.notePreviewTab.setAttribute("aria-pressed", String(preview));
  if (!preview) els.noteInput.focus();
}
```

`els` gains `noteEditTab`, `notePreviewTab`, `notePreview`. Wiring beside the existing
note handlers:

```js
els.noteEditTab.addEventListener("click", () => setNoteMode("edit"));
els.notePreviewTab.addEventListener("click", () => setNoteMode("preview"));
```

No startup call is needed — the markup ships in Edit mode (textarea visible, preview
`hidden`, Edit tab `aria-pressed="true"`). Autosave, `loadNote`, and every task handler
are untouched.

## 4. `web/index.html`

```html
<section class="pane note-pane">
  <div class="note-head">
    <h1>note</h1>
    <div class="note-tabs" role="group" aria-label="Note view">
      <button type="button" id="note-edit-tab" aria-pressed="true">Edit</button>
      <button type="button" id="note-preview-tab" aria-pressed="false">Preview</button>
    </div>
  </div>
  <textarea id="note-input" class="note-input" aria-label="Note (Markdown)" placeholder="Notes (Markdown)…"></textarea>
  <div id="note-preview" class="note-preview markdown-body" hidden></div>
  <div class="note-bar">
    <span id="note-status" class="note-status" role="status"></span>
  </div>
</section>
```

`#note-input`, `#note-status`, and all other ids are unchanged, so existing `app.js`
lookups and autosave keep working.

## 5. `web/style.css`

- `.note-head` — a flex row, `justify-content: space-between; align-items: baseline;`
  so the title and tabs sit on one line; the existing `h1` margin is neutralised inside
  it.
- `.note-tabs` — a small inline button group; `button[aria-pressed="true"]` gets the
  accent background (reusing `--accent`) to read as the active tab, `false` stays the
  plain bordered button. Reuse the existing generic `button` rule for base shape.
- `.note-preview` — same border/radius/padding/min-height envelope as `.note-input`
  (so toggling does not resize the pane), `background: #fff`, `overflow: auto`.
- `.markdown-body` — the reading typography: heading sizes/weights, paragraph spacing,
  `code`/`pre` with a subtle `--bg`-tinted background and monospace font, `blockquote`
  with a left `--border` rule and `--muted` text, `ul/ol` left padding, `a` in
  `--accent`, `hr` as a `--border` line. Tuned for a clean Obsidian/Notion-like read,
  all from existing tokens.

No existing rule changes meaning; this is additive plus the `.note-head` wrapper around
the existing `h1`.

## 6. `README.md`

Update the note-pane paragraph: it currently says the pane is "a plain editor over the
raw `Note.md` text (no Markdown preview)". Change to describe the **Edit / Preview**
toggle (Obsidian-style reading view) and enumerate the supported Markdown subset
(headings, bold/italic, inline + fenced code, ordered/unordered lists one level,
blockquotes, horizontal rules, links) and the explicit limits (no tables/images/nested
lists/raw HTML; unsupported syntax shows as literal text). Note that rendering is
client-side and the stored file stays raw Markdown.

## 7. Tests and verification

No new automated test: the renderer is client-side JS and the repo has no JS test
harness by its deliberate no-build-step decision (the same constraint-bound position as
every prior WebUI feature). `src/` is untouched, so the entire existing Rust suite still
applies as a pure regression guard, and `cargo build` re-embeds the edited assets via
`include_str!` (a malformed `index.html` fails the build).

- **Rust gates:** `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check` green.
- **Manual (the renderer + toggle):** `cargo run`, open `http://127.0.0.1:8080`:
  - The note pane shows **Edit | Preview** tabs; it opens in Edit (textarea) exactly as
    before; autosave still works (type, see Saving…/Saved).
  - Enter Markdown exercising each construct (a `#`/`##` heading, `**bold**`, `*italic*`,
    `` `code` ``, a fenced ```` ``` ```` block, a `-` list, a `1.` list, a `>` quote,
    `---`, a `[link](https://example.com)`), click **Preview**: each renders correctly;
    **Edit** returns to the textarea with the raw text intact.
  - Security checks: typing `<script>alert(1)</script>` shows the literal text in
    Preview (escaped, no alert); a `[x](javascript:alert(1))` link renders with
    `href="#"` (inspect the DOM), and an `https://` link opens normally.
  - Unsupported syntax (e.g. a `| table |` row) shows as literal text, not broken markup.
  - Toggling Preview right after typing reflects the just-typed text (reads the live
    textarea value), and the autosave status bar stays visible in both modes.

## 8. Risks

- **XSS via `innerHTML`.** The headline risk of any HTML renderer. Mitigated by
  `escapeHtml` on every user-supplied run before it becomes HTML, `safeHref` neutering
  non-http(s)/mailto/relative link targets, and `target=_blank rel=noopener` on links —
  the only literal tags are the ones the renderer itself emits.
- **Renderer correctness is not covered by CI.** A parser is more error-prone than UI
  glue, and there is no JS test harness (the standing no-build constraint). Mitigated by
  keeping the renderer pure and small, routing all block detection through one
  `blockKind`, and the per-construct manual checklist in section 7. Accepted as
  constraint-bound, consistent with every prior WebUI feature; the server half is
  untouched and still covered.
- **Subset incompleteness surprising a user.** Someone may type a table or nested list
  and expect rendering. Mitigated by documenting the exact supported subset and the
  "unsupported renders as literal text" rule in the README, and by never emitting broken
  markup for unknown syntax (it falls through to a paragraph).
- **Cognitive load of a hand-written parser.** A Markdown parser is intrinsically some
  code. Mitigated by decomposition into small single-purpose pure functions, a single
  `blockKind` source of truth, and a flat table-dispatched main loop — no function over
  ~10 lines, no nesting past two levels.
- **Regex emphasis edge cases** (e.g. `a*b*c`, intraword underscores). Accepted: a
  subset renderer is "good enough" reading support, not a CommonMark implementation;
  the limits are documented, and worst case is cosmetic (wrong emphasis), never unsafe
  (everything is escaped first).

## 9. Why client-side vanilla, not a library or the server

- **Architectural consistency / conceptual integrity.** The server is the canonical
  *raw* store (todo.txt + Note.md); all presentation — task formatting, sorting,
  filtering — already happens on the client. Rendering Markdown server-side would split
  the "raw store vs. presentation" rule across both tiers and add a render endpoint and
  a round-trip for what is a pure view concern. Keeping it client-side preserves the
  existing separation.
- **No-build constraint.** The repo deliberately embeds hand-written vanilla assets
  (`include_str!`) with no npm/bundler step. A Markdown library or a build pipeline would
  break that; a focused vanilla renderer does not. This is the same constraint that made
  the original `note-md-support` plan defer preview — now satisfied within it.
- **Cost.** The tradeoff is the lack of an automated test for the renderer (section 8),
  accepted as constraint-bound. A server-side renderer would be unit-testable but at the
  cost of a new dependency (or a hand-rolled Rust parser), a new endpoint, a round-trip,
  and the architectural split above — a worse deal for a view feature.

## 10. Files touched

- `web/app.js` — renderer (`escapeHtml`, `safeHref`, `renderInline`, `formatText`,
  `blockKind`, `renderMarkdown` + `TAKE` table + `takeHeading/Fence/Blockquote/List/
  Paragraph`) and the `setNoteMode` toggle + tab wiring + three new `els` (section 3).
- `web/index.html` — note-pane header tabs and `#note-preview` target (section 4).
- `web/style.css` — `.note-head`, `.note-tabs`, `.note-preview`, `.markdown-body`
  (section 5).
- `README.md` — Edit/Preview toggle + supported subset (section 6).
- `implemented/note-md-preview.md` — written after implementation.

## 11. brooks-review outcome

`reviews/note-md-preview.md` scored the plan 99/100 on the first pass with one 🟢
Suggestion (R3 Knowledge Duplication): the list-marker, ordered-marker, and fence
line-patterns were each written in two-to-four places (`blockKind` + `takeList` +
`takeFence`), so extending the marker syntax would mean changing the same regex in
several spots, and a missed copy would desync the classifier from the consumers. Adopted:
section 3.3 now names the three patterns as the `LIST_ITEM`, `ORDERED_ITEM`, and `FENCE`
constants and every site references them. The re-review found no further issues — the
parser decomposition (R1), the `TAKE` table (R4), the client-side rendering choice (R5/
R6, section 9), and the escaping/href-sanitising (security) were each examined and
cleared — bringing the plan to 100/100. The only remaining note is the constraint-bound
absence of a JS test harness for the renderer (the standing no-build decision the feature
is designed to respect); the server is untouched and stays covered by the existing suite.
