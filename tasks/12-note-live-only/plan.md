# Plan: Make Live the only note view — drop the Edit/Preview toggle (branch `note-live-only`)

Collapse the note pane's three-mode **Edit | Live | Preview** toggle down to a single,
always-on **Live** view: the raw-Markdown `<textarea>` and the rendered preview sit side
by side over the *same* note text, and the preview re-renders as you type. The tab group
and the mode machinery go away entirely; what was task 11's `is-split` Live layout becomes
the pane's permanent and only layout. This is a pure simplification — it removes code and
markup, adds no new behaviour, and leaves the renderer, autosave, store, and API untouched.

## 1. Goal and scope

Requirement (from `agents/ADD_NEW_FEATURE.md`):

> - Can we remove Edit and Preview, just leave Live only? I want make live preview and
>   edit at same input text

Task 11 shipped a third **Live** mode (editor beside preview, re-rendering per keystroke)
on top of task 10's Edit/Preview toggle, driven by a `NOTE_MODES` descriptor table and a
`setNoteMode` switcher. The request is to keep *only* that Live experience: editing and
the live preview always share the one `#note-input` text, with no mode to choose. So the
toggle, the three tab buttons, the mode state, and the switcher are all deleted, and the
split layout that Live used becomes unconditional.

Interpretation of "live preview and edit at same input text": the editor and preview
operate on the single existing `#note-input` value (they already do — Live reads the
textarea and renders into `#note-preview`). This is **not** a request for a single-surface
WYSIWYG/`contenteditable` editor (still the explicit non-goal it was in tasks 10–11, for
the same no-build/no-dependency reasons — section 8). "Same input text" means one note,
one editor, preview always mirroring it — which is exactly what removing the other two
modes leaves.

In scope (WebUI only; the note is already stored/served as raw Markdown and the renderer
already exists and is reused verbatim):

- `web/index.html` — delete the `.note-tabs` button group; the `#note-views` wrapper now
  always holds the visible editor + preview (the preview loses its `hidden` attribute).
- `web/app.js` — delete `noteMode`, the `NOTE_MODES` table, `isSplit`, `setNoteMode`, the
  three `els` tab handles, and the tab-wiring loop. `renderNotePreview` stays (it is the
  one place preview HTML is written). The `input` handler renders the preview
  unconditionally. The preview is rendered once on initial load so it is populated before
  the first keystroke.
- `web/style.css` — the split layout becomes the base `.note-views` rule (no `.is-split`
  modifier); delete the `.note-tabs` rules and the `aria-pressed` button rule. The
  narrow-screen stack applies to `.note-views` directly.
- `README.md` — replace the "Edit / Live / Preview toggle" description with the single
  always-live editor+preview pane.
- `tasks/12-note-live-only/implemented.md` — written after implementation.

Out of scope (explicit non-goals — section 8 expands):

- **No server-side rendering, no new endpoint, no third-party library, no build step.**
  Same standing constraints as tasks 10–11; this change only removes client view code.
- **No single-surface WYSIWYG / `contenteditable` editing.** The editor stays a plain
  `<textarea>` beside a rendered `<div>`.
- **No change to the renderer, the Markdown subset, autosave, the store, the API, or the
  task pane.** `src/` is untouched.

## 2. Layout and interaction (after)

The note-pane header keeps its `<h1>note</h1>` title; the tab group is gone, so the head
is just the title. Below it, `#note-views` always shows the `<textarea>` (`#note-input`)
and the rendered `<div>` (`#note-preview`) **side by side**, and typing re-renders the
preview immediately. The autosave status bar stays below, unchanged.

- There is no mode to select and nothing is ever `hidden`: the pane opens already showing
  editor + live preview, populated from the loaded note (section 3.3), and stays that way.
- On narrow screens the two columns stack (editor above preview), the same fallback Live
  used in task 11 — now applied to the base layout.

Because there is only one view, the questions task 11's `setNoteMode` answered (which
views are visible, which tab is pressed, whether to render, where focus goes) collapse:
both views are always visible, there are no tabs, the preview always renders, and the
textarea is the only interactive surface.

## 3. `web/app.js`

The renderer from task 10 (`escapeHtml`, `safeHref`, `renderInline`, `formatText`,
`blockKind`, `renderMarkdown`, the `TAKE` table and `takeX` consumers) is **unchanged**.
Autosave (`noteDirty`, `setNoteStatus`, `scheduleNoteSave`, `flushNote`, `flushNoteBeacon`,
`loadNote`) is unchanged except for the one render-on-load line (3.3). The change is a net
deletion of the mode layer.

### 3.1 Deletions

Remove, as one cohesive block (current lines ~707–750 and the tab `els` + wiring):

- `let noteMode = "edit";`
- the `NOTE_MODES` descriptor table,
- `const isSplit = (view) => view.input && view.preview;`,
- the entire `setNoteMode` function,
- the `noteEditTab`, `noteLiveTab`, `notePreviewTab` entries in `els`,
- the tab-wiring loop:
  `for (const [mode, { tab }] of Object.entries(NOTE_MODES)) tab.addEventListener(...)`.

After this, `els` retains `noteInput`, `noteViews`, `notePreview`, `noteStatus`.
(`noteViews` is kept only if section 5 still needs a JS handle — it does not, since the
split is now CSS-default, so `noteViews` is **also deleted** from `els`; `#note-views`
stays in the markup purely as the flex container styled by CSS. One fewer `els` handle.)

### 3.2 The render helper (kept, unchanged)

`renderNotePreview` is the single place preview HTML is written and stays exactly as is —
it already saves/restores `scrollTop` so a live re-render mid-edit does not jump the
preview to the top:

```js
function renderNotePreview() {
  const { scrollTop } = els.notePreview;
  els.notePreview.innerHTML = renderMarkdown(els.noteInput.value);
  els.notePreview.scrollTop = scrollTop;
}
```

### 3.3 Render on load and on input

With no mode gate, the preview must be populated from the loaded note before the first
keystroke, and refreshed on every keystroke. Two small edits:

- `loadNote` already sets `els.noteInput.value = text`. Add `renderNotePreview()` right
  after that assignment (and only on success, alongside the existing `setNoteStatus("",
  false)`), so the preview matches the loaded note on first paint. The empty-note case
  renders to empty HTML — one code path, no special case.
- The `input` handler drops the `isSplit(...)` guard and always re-renders:

```js
els.noteInput.addEventListener("input", () => {
  setNoteStatus("Editing…", false);
  scheduleNoteSave();
  renderNotePreview(); // preview is always on screen now, so always mirror the editor
});
```

`renderMarkdown` is pure and O(n) over the note text; rendering synchronously per
keystroke for a single personal note is imperceptible (the same accepted tradeoff as task
11, section 7). Autosave keeps its own independent debounce.

### 3.4 No startup mode call

Task 11 needed no startup `setNoteMode` because the markup shipped in Edit mode; now there
is no mode at all. The only startup addition is the render-on-load in `loadNote` (3.3);
`loadNote()` is already called at the end of the file, so the preview is populated as soon
as the note arrives. No other wiring changes.

## 4. `web/index.html`

Remove the tab group and its now-vestigial `.note-head` wrapper, and un-hide the preview:

```html
<section class="pane note-pane">
  <h1>note</h1>
  <div id="note-views" class="note-views">
    <textarea id="note-input" class="note-input" aria-label="Note (Markdown)" placeholder="Notes (Markdown)…"></textarea>
    <div id="note-preview" class="note-preview markdown-body"></div>
  </div>
  <div class="note-bar">
    <span id="note-status" class="note-status" role="status"></span>
  </div>
</section>
```

Changes: the `<div class="note-tabs" …>…</div>` block (three buttons) is deleted, the
preview `<div>` loses its `hidden` attribute (it is always shown now), and the
`<div class="note-head">` wrapper is removed so the `<h1>note</h1>` sits directly in the
pane. The `.note-head` wrapper existed only to lay the title and the tab group out on one
row (flex, `space-between`); with the tabs gone it would wrap a lone `<h1>` and its flex
rules would be a single-child no-op, so it is dropped rather than left as dead structure
(adopted plan-review R4 suggestion). Every retained id (`note-input`, `note-preview`,
`note-status`, `note-views`) is unchanged, so the surviving `app.js` lookups and autosave
keep working; the `<h1>` keeps the same page `h1` styling it had inside the wrapper.

## 5. `web/style.css`

- `.note-views` — promote the old `.note-views.is-split` declarations to the base rule, so
  the editor + preview are **always** the two-column flex layout:
  `display: flex; gap: 1rem;` with `.note-views > .note-input, .note-views > .note-preview
  { flex: 1; min-width: 0; }`. The `.is-split` class no longer exists in markup or JS, so
  the modifier is dropped.
- Narrow-screen fallback — the `@media (max-width: 720px)` rule that set
  `.note-views.is-split { flex-direction: column; }` becomes `.note-views { flex-direction:
  column; }`, so the two columns still stack on a phone.
- Delete the `.note-tabs`, `.note-tabs button`, and `.note-tabs button[aria-pressed="true"]`
  rules — the tab group no longer exists.
- Delete the `.note-head` rule (style.css:53) — its wrapper element is removed in section 4,
  so the rule would target nothing.
- `.note-input`, `.note-preview`, `.markdown-body`, `.note-bar`, `.note-status` are
  unchanged.

Net: the split layout that was conditional becomes the default, and the tab + head styling
is removed. No surviving rule changes meaning.

## 6. `README.md`

Rewrite the note-pane paragraph in the **WebUI** section: drop the "Edit / Live / Preview
toggle" sentence and describe the pane as a single always-live view — "the note pane shows
a raw-Markdown editor and its rendered preview side by side, updating as you type
(Notion/Obsidian-style live preview; the columns stack on narrow screens)". Keep the
existing description of the supported Markdown subset, the "rendering is purely a view: the
stored file stays raw Markdown" note, and the autosave paragraph (all unchanged). Remove
the now-inaccurate mentions of separate Edit and Preview modes.

## 7. Tests and verification

No new automated test: the renderer and view are client-side JS and the repo has no JS
test harness by its deliberate no-build decision — the same constraint-bound position as
tasks 10–11. `src/` is untouched, so the entire Rust suite still applies as a regression
guard, and `cargo build` re-embeds the edited assets via `include_str!` (a malformed
`index.html` fails the build).

- **Rust gates:** `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check` green.
- **Manual:** `cargo run`, open `http://127.0.0.1:8080`:
  - The note pane shows **no tabs** — just the title, then editor + preview side by side,
    populated from the existing `Note.md` content on load.
  - Type a heading, `**bold**`, a `-` list, a fenced code block: the preview updates on
    each keystroke and matches.
  - Scroll the preview down, then keep typing: the preview holds its scroll position
    (does not jump to the top) — confirms `renderNotePreview`'s `scrollTop` save/restore.
  - Autosave still shows `Editing…`/`Saving…`/`Saved` while editing; reload re-shows the
    saved text with the preview already rendered.
  - Security regression (renderer unchanged): typing `<script>alert(1)</script>` shows
    literal text (no alert); `[x](javascript:alert(1))` renders `href="#"`.
  - Narrow the window below 720px: editor and preview stack vertically (no overflow).
  - No console errors (confirms no dangling reference to the removed tab elements/handlers).

## 8. Risks

- **Dangling references to removed elements.** Deleting the tab buttons and the mode layer
  must be complete: any leftover `getElementById("note-edit-tab")` or `setNoteMode`/
  `NOTE_MODES`/`isSplit`/`noteMode` reference would throw at load. Mitigated by removing
  the `els` tab handles, the table, the helper, the function, and the wiring loop together
  (section 3.1) and by a post-change `grep` for `noteMode|NOTE_MODES|setNoteMode|isSplit|
  note-edit-tab|note-live-tab|note-preview-tab|noteEditTab|noteLiveTab|notePreviewTab`
  returning nothing, plus the "no console errors" manual check.
- **Preview empty until first keystroke.** Without the render-on-load line, the preview
  would start blank even when `Note.md` has content. Mitigated by `renderNotePreview()` in
  `loadNote` after the value is set (section 3.3).
- **Per-keystroke render cost on a very large note.** Live now always renders on input
  (no Edit mode to avoid it). For a personal note this is imperceptible (`renderMarkdown`
  is one O(n) pass); if a pathologically large note ever lagged, the fix is a short
  `requestAnimationFrame`/timeout coalesce in the one `input` line — localised and
  non-structural. Same accepted tradeoff as task 11.
- **Scroll jump while typing.** Replacing `innerHTML` resets `scrollTop`; mitigated, as
  before, by `renderNotePreview` saving/restoring it (unchanged from task 11).
- **XSS via `innerHTML`.** Unchanged: every user run is `escapeHtml`-ed, `safeHref`
  neuters non-safe link targets, links carry `rel=noopener`. The renderer is reused
  verbatim; the manual checklist re-confirms.
- **Loss of a full-width reading view.** Removing Preview means the rendered note is never
  full-width; it always shares the pane with the editor. This is the explicit intent of
  the request ("just leave Live only"), accepted as the desired simplification.

## 9. Why drop the modes entirely rather than keep them

- **Conceptual integrity / minimal surface.** The request is to make editing-with-live-
  preview the single behaviour. Keeping a one-button "toggle" or a dormant `setNoteMode`
  for a single mode would be dead complexity. Deleting the mode layer makes the code match
  the one behaviour it now has — the smallest faithful implementation.
- **No-build / no-dependency constraint, still honoured.** As in tasks 10–11, a true
  single-surface WYSIWYG editor would need a `contenteditable` engine and a build step; out
  of scope. "Same input text" is satisfied by the existing `<textarea>` + rendered `<div>`
  pair over one note value.
- **Net negative diff.** This change removes markup, CSS, and JS and adds only a
  render-on-load line and an unconditional render-on-input. Less surface to maintain, same
  client-side renderer, server untouched.

## 10. Files touched

- `web/app.js` — delete `noteMode`, `NOTE_MODES`, `isSplit`, `setNoteMode`, the three tab
  `els` handles and the `noteViews` handle, and the tab-wiring loop; add
  `renderNotePreview()` to `loadNote`; drop the `isSplit` guard in the `input` handler.
  Renderer and autosave otherwise unchanged (section 3).
- `web/index.html` — delete the `.note-tabs` group and the `.note-head` wrapper (h1 sits
  directly in the pane); remove `hidden` from `#note-preview` (section 4).
- `web/style.css` — promote `.is-split` declarations to base `.note-views`; retarget the
  narrow-screen stack to `.note-views`; delete the `.note-tabs*` and `.note-head` rules
  (section 5).
- `README.md` — single always-live editor+preview description (section 6).
- `tasks/12-note-live-only/implemented.md` — written after implementation.

## 11. brooks-review outcome

`tasks/12-note-live-only/plan-review.md` scored the first draft 99/100 with one 🟢
Suggestion (R4 Accidental Complexity): removing the tab group left `.note-head` as a
single-child flex wrapper around the lone `<h1>` — dead structure whose `space-between`
flex rules no longer do anything. Adopted: section 4 now removes the `.note-head` wrapper
(the `<h1>` sits directly in the pane) and section 5 deletes the `.note-head` CSS rule, so
no vestigial layout structure survives the simplification. The re-review found no further
issues (100/100). The only standing note is the constraint-bound absence of a JS test
harness for the client renderer — the same no-build decision tasks 10–11 accepted; the
renderer is reused unchanged and the server half stays covered by the Rust suite.
