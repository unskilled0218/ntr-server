# Implemented: Markdown preview for the note pane (branch `note-md-preview`)

Built per `plans/note-md-preview.md` (brooks-review 100/100,
`reviews/note-md-preview.md`). The note pane gained an **Edit / Preview** toggle
(Obsidian reading-view style) backed by a small, dependency-free, no-build vanilla
Markdown renderer. Backend untouched — the note stays a raw-Markdown store; rendering is
client-side presentation.

## What changed

- **`web/app.js`** — added a pure Markdown renderer and the mode toggle:
  - `LIST_ITEM` / `ORDERED_ITEM` / `FENCE` — the repeated block line-patterns named once
    (the adopted brooks-review R3 fix).
  - `escapeHtml`, `safeHref` — security: every user run is escaped before it becomes
    HTML; link targets are sanitised (javascript: → `#`), links open with `rel=noopener`.
  - `formatText` / `renderInline` — inline links/bold/italic, with code spans split out
    first so their contents stay literal.
  - `blockKind` — the single source of block classification; `takeHeading`, `takeFence`,
    `takeBlockquote`, `takeList`, `takeParagraph` — per-block consumers returning
    `{ html, next }`; `TAKE` table + `renderMarkdown` flat dispatch loop.
  - `setNoteMode(mode)` — the one DOM-touching piece: renders the textarea's current
    text into `#note-preview` on entering Preview, toggles `hidden`, sets `aria-pressed`,
    refocuses the textarea on Edit. `els` gained `notePreview`, `noteEditTab`,
    `notePreviewTab`; the two tab buttons are wired beside the existing note handlers.
- **`web/index.html`** — note-pane header (`.note-head`) with the `note` title and an
  Edit/Preview `.note-tabs` group; a `#note-preview .markdown-body` render target beside
  the textarea (ships `hidden`, Edit active).
- **`web/style.css`** — `.note-head`, `.note-tabs` (active tab uses `--accent`),
  `.note-preview` (same envelope as `.note-input` so toggling does not resize the pane),
  and a `.markdown-body` reading stylesheet (headings, paragraphs, `code`/`pre`,
  `blockquote`, lists, links, `hr`) built from the existing color tokens.
- **`README.md`** — note-pane paragraph rewritten to describe the Edit/Preview toggle,
  the supported Markdown subset, and that rendering is client-side (file stays raw).

Supported subset: headings (`#`..`######`), bold, italic, inline + fenced code,
ordered/unordered lists (one level), blockquotes, horizontal rules, links. Unsupported
syntax (tables, images, nested lists, raw HTML) renders as literal text.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` (17 unit + 8
  integration tests) — all green. `src/` unchanged, so the suite is a pure regression
  guard.
- Runtime smoke against a freshly built binary: the served page ships the Edit/Preview
  tabs and `#note-preview`; `app.js` carries `renderMarkdown`/`escapeHtml`/`setNoteMode`;
  `style.css` carries the `.markdown-body` rules; `PUT /api/note` → 204 and `GET` round-
  trips.
- The renderer is pure and was traced by hand across headings, lists (ordered/unordered),
  paragraphs, code spans, fenced blocks, blockquotes, HTML escaping (`<script>` shown
  literally), and link sanitisation (`javascript:` → `#`). No JS engine is available in
  this environment and the repo has no JS test harness (the standing no-build
  constraint), so the in-browser behaviour is verified by reading the code and the
  manual checklist in the plan, not mechanically.

## Notes / non-goals (unchanged from the plan)

- No server-side rendering, no new endpoint, no third-party library, no build step.
- No Notion-style WYSIWYG/contenteditable, no live per-keystroke side-by-side preview,
  no tables/images/nested lists/raw HTML.
- Autosave and the task pane are untouched.
