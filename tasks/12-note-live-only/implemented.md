# Implemented: Live-only note view (branch `note-live-only`)

Collapsed the note pane's **Edit | Live | Preview** toggle down to a single always-on live
view: the raw-Markdown `<textarea>` and the rendered preview always sit side by side over
the same note text, and the preview re-renders on every keystroke. The mode layer, the tab
group, and the conditional split layout are gone. Matches `plan.md` exactly; the one
plan-review 🟢 (the vestigial `.note-head` wrapper) was adopted during planning and so was
removed here too.

## Changes

### `web/index.html`
- Deleted the `<div class="note-tabs">` group (the Edit/Live/Preview buttons).
- Deleted the `<div class="note-head">` wrapper; `<h1>note</h1>` now sits directly in the
  `.note-pane` section.
- Removed the `hidden` attribute from `#note-preview` — it is always shown now.
- Every retained id (`note-input`, `note-preview`, `note-status`, `note-views`) unchanged.

### `web/app.js`
- Removed the `noteEditTab`, `noteLiveTab`, `notePreviewTab`, and `noteViews` handles from
  `els` (the split is now CSS-default, so no JS handle to `#note-views` is needed).
- Deleted `let noteMode`, the `NOTE_MODES` descriptor table, the `isSplit` predicate, and
  the entire `setNoteMode` function.
- Deleted the tab-wiring loop (`for (… of Object.entries(NOTE_MODES)) tab.addEventListener(
  "click", …)`).
- `renderNotePreview` kept unchanged (still the one place preview HTML is written; still
  saves/restores `scrollTop`); only its comment updated to note it now runs on load and on
  every keystroke with no mode gate.
- `loadNote`: added `renderNotePreview()` right after `els.noteInput.value = text`, so the
  preview is populated from the loaded note before the first keystroke.
- The `input` handler dropped the `isSplit(NOTE_MODES[noteMode])` guard and now calls
  `renderNotePreview()` unconditionally.
- Updated the stale renderer-section comment ("the Preview tab renders it") to "the live
  preview beside the editor".
- Renderer (`escapeHtml`, `safeHref`, `renderInline`, `formatText`, `blockKind`,
  `renderMarkdown`, the `TAKE` table) and autosave are otherwise untouched.

### `web/style.css`
- Promoted the old `.note-views.is-split` declarations to the base `.note-views` rule
  (`display: flex; gap: 1rem;` + `flex: 1; min-width: 0` on the two children), so the
  two-column layout is unconditional.
- Retargeted the `@media (max-width: 720px)` stack from `.note-views.is-split` to
  `.note-views`.
- Deleted the `.note-tabs`, `.note-tabs button`, `.note-tabs button[aria-pressed="true"]`,
  and `.note-head` rules — their elements no longer exist.

### `README.md`
- Replaced the "Edit / Live / Preview toggle" paragraph with the single always-live
  editor+preview description; kept the Markdown-subset list, the "rendering is purely a
  view" note, and the autosave paragraph unchanged.

## Verification

- **Dangling-reference sweep:** `grep -rn` over `web/` and `README.md` for
  `noteMode|NOTE_MODES|setNoteMode|isSplit|is-split|note-edit-tab|note-live-tab|
  note-preview-tab|noteEditTab|noteLiveTab|notePreviewTab|note-tabs|note-head` → no matches.
- **Rust gates:** `cargo fmt --check` clean; `cargo clippy --all-targets` clean;
  `cargo test` → 8 passed, 0 failed (`cargo build` re-embeds the edited assets via
  `include_str!`, so a malformed `index.html` would have failed the build).
- **Runtime smoke:** ran the server, fetched `/` — the note pane serves the title then the
  editor + preview side by side, no tab group, preview not `hidden`; the embedded
  `app.js` contains zero `setNoteMode`/`NOTE_MODES` references.
- **Manual (per plan section 7):** to confirm in a browser — typing updates the preview
  per keystroke; the preview holds scroll position while typing; autosave still cycles
  `Editing…`/`Saving…`/`Saved`; `<script>` and `javascript:` inputs stay inert; below
  720px the two columns stack.

## Not done / out of scope (unchanged from plan)

- No single-surface WYSIWYG/`contenteditable` editing — still a `<textarea>` beside a
  rendered `<div>` (no-build/no-dependency constraint).
- No server-side rendering, no new endpoint, no library, no build step. `src/` untouched.
- No JS test harness for the client renderer — the standing no-build constraint from
  tasks 10–11; the renderer is reused verbatim and the server half stays covered by the
  Rust suite.
