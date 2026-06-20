# Plan: Live Markdown preview for the note pane (branch `note-md-live-preview`)

Add a **Live** view to the note pane: the raw-Markdown `<textarea>` and the rendered
preview sit side by side, and the preview re-renders **as you type** — the Notion /
Obsidian "live preview" feel. This builds directly on the Edit/Preview toggle and the
pure `renderMarkdown` renderer shipped in task `10-note-md-preview`; it adds a third mode
and a split layout, and reuses the existing renderer unchanged.

## 1. Goal and scope

Requirement (from `agents/ADD_NEW_FEATURE.md`):

> - show preview lively while edit note.md like notion or obsidian

Task 10 added an **Edit / Preview** toggle (Obsidian *reading-view* model) and listed
"no live per-keystroke side-by-side preview" as an explicit non-goal. This feature
delivers exactly that deferred piece: a **Live** mode that shows the editor and the
rendered output together and keeps the render in sync with each keystroke.

Interpretation of "like notion or obsidian": the achievable, codebase-consistent analog
is a **split live preview** — editor on the left, rendered Markdown on the right,
updating live. (Obsidian's own "Live Preview" and Notion's inline WYSIWYG are
single-surface `contenteditable` editors that re-implement editing itself; that is a
fundamentally larger thing and remains an explicit non-goal — section 9.)

In scope (WebUI only — the note is already stored and served as raw Markdown, and the
renderer already exists):

- `web/app.js` — a third note mode `"live"` in the existing `setNoteMode`, a tiny
  `renderNotePreview()` helper (the one place that writes rendered HTML into the preview
  node, preserving scroll), a `noteMode` state variable, a single `NOTE_TABS`
  mode→button map driving both `aria-pressed` and click-wiring, and a one-line addition
  to the existing `input` handler so Live re-renders on each keystroke. `renderMarkdown`
  and every other renderer function are **untouched**.
- `web/index.html` — a third tab button (**Live**) and a `#note-views` wrapper around the
  existing `#note-input` and `#note-preview` so the two can be laid out side by side.
- `web/style.css` — `.note-views` (default: today's stacked single-view behaviour) and
  `.note-views.is-split` (the two-column Live layout, stacking on narrow screens). No
  existing rule changes meaning.
- `README.md` — document the third **Live** mode.
- `tasks/11-note-md-live-preview/implemented.md` — written after implementation.

Out of scope (explicit non-goals — section 9 expands the rationale):

- **No server-side rendering, no new endpoint, no third-party library, no build step.**
  Same constraints task 10 honoured; this change is purely additive client-side view code.
- **No Notion/Obsidian-style single-surface WYSIWYG / `contenteditable` editing.** Live
  mode is editor-beside-preview, not in-place rendered editing.
- **No scroll-position syncing between editor and preview** (cursor-follows-scroll). The
  preview holds its own scroll position across re-renders (section 3.2); locking the two
  scrollbars together is a separate, larger feature.
- **No change to the renderer, the Markdown subset, autosave, or the task pane.**

## 2. Layout and interaction

The note-pane header keeps its title + tab group, now with **three** tabs:
**Edit | Live | Preview**. Below it, the existing `<textarea>` (`#note-input`) and
rendered `<div>` (`#note-preview`) are wrapped in a single `#note-views` container.

- **Edit** (default, unchanged): textarea only, full width — the pane opens exactly as
  today.
- **Preview** (unchanged): rendered output only, full width — the reading view.
- **Live** (new): textarea and preview side by side; typing in the textarea re-renders
  the preview immediately. The autosave status bar stays below, visible in every mode.

Visibility is still driven by each view's `hidden` attribute; Live shows **both**. The
split itself is a single class toggle (`is-split`) on `#note-views`, so the layout
switch is one CSS rule, not per-element style juggling. Entering Live or Preview renders
the textarea's current value first, so the preview always matches what is on screen.
Entering Edit or Live focuses the textarea (you are there to type).

## 3. `web/app.js`

The renderer from task 10 (`escapeHtml`, `safeHref`, `renderInline`, `formatText`,
`blockKind`, `renderMarkdown`, the `TAKE` table and `takeX` consumers) is **unchanged**.
The only additions are the mode state, the render helper, and the three-mode toggle.

### 3.1 Mode state and the single mode descriptor table

`els` gains `noteLiveTab` and `noteViews`; `noteEditTab`, `notePreviewTab`, `notePreview`,
`noteInput` already exist. The three view modes are described **once** in a table that
pairs each mode with its tab button *and* records which of the two views it shows. The
`aria-pressed` sync, the click-wiring, and every visibility decision all read from this
one table — so "what modes exist and which views each shows" has a single home (adopted
brooks-review R3 fix; same shape as task 10's `TAKE` dispatch table):

```js
// The note's current view; drives whether the live render runs on each keystroke.
let noteMode = "edit";

// Single source of truth for the note's view modes: each mode's tab button and which
// of the two views (editor / preview) it shows. setNoteMode, the tab wiring, and the
// aria-pressed sync all read from here, so adding or renaming a mode is one edit with
// no second list (or hand-written boolean) to keep in step. "live" shows both — the
// split layout — and is the only mode whose preview tracks typing.
const NOTE_MODES = {
  edit: { tab: els.noteEditTab, input: true, preview: false },
  live: { tab: els.noteLiveTab, input: true, preview: true },
  preview: { tab: els.notePreviewTab, input: false, preview: true },
};
```

### 3.2 The render helper (the one place preview HTML is written)

```js
// Render the textarea's current Markdown into the preview node. renderMarkdown's output
// is already escaped/sanitised, so assigning it to innerHTML is safe. scrollTop is saved
// and restored so a live re-render while typing does not jump the preview to the top.
function renderNotePreview() {
  const { scrollTop } = els.notePreview;
  els.notePreview.innerHTML = renderMarkdown(els.noteInput.value);
  els.notePreview.scrollTop = scrollTop;
}
```

Preserving `scrollTop` is what makes Live feel steady: on each keystroke the preview is
rebuilt, and without this the pane would snap to the top mid-edit. On first entry
`scrollTop` is `0`, so the save/restore is a no-op there — one code path, no special case.

### 3.3 The three-mode toggle

```js
// Show one of the note's three views, reading visibility from NOTE_MODES so there is no
// per-mode branching. A mode showing the preview renders the textarea's CURRENT text
// first (so output matches what is on screen); a mode showing both views is the split
// (Live) layout; any mode showing the editor focuses it. aria-pressed marks the active
// tab from the same table.
function setNoteMode(mode) {
  noteMode = mode;
  const view = NOTE_MODES[mode];
  if (view.preview) renderNotePreview();
  els.noteInput.hidden = !view.input;
  els.notePreview.hidden = !view.preview;
  els.noteViews.classList.toggle("is-split", view.input && view.preview);
  for (const [m, { tab }] of Object.entries(NOTE_MODES))
    tab.setAttribute("aria-pressed", String(m === mode));
  if (view.input) els.noteInput.focus();
}
```

Every consequence — the two `hidden` flags, whether to render, whether the split layout
is on, where focus goes — is derived from the one `view` descriptor, so there is no
per-mode `if` ladder and no hand-written `mode === "x"` membership test.

### 3.4 Live re-render on input

The existing `input` handler already sets the status and arms autosave. Live mode adds
one line — re-render only when the preview is actually on screen, so Edit/Preview pay
nothing:

```js
els.noteInput.addEventListener("input", () => {
  setNoteStatus("Editing…", false);
  scheduleNoteSave();
  // Re-render only when the on-screen preview tracks the editor — i.e. the split (Live)
  // mode, the one mode showing both views. Edit/Preview never enter this branch.
  const view = NOTE_MODES[noteMode];
  if (view.input && view.preview) renderNotePreview();
});
```

`renderMarkdown` is pure and O(n) over the note text; for a single personal note,
rendering synchronously per keystroke is imperceptible and gives the true "lively" feel
the request asks for (no debounce lag). Autosave keeps its own independent debounce
(`scheduleNoteSave`) — the two are unrelated. See section 8 for the large-note tradeoff.

### 3.5 Wiring

The three tabs wire from the same `NOTE_MODES` table; no startup call is needed because
the markup ships in Edit mode (textarea visible, preview `hidden`, Edit `aria-pressed=true`,
`#note-views` not split):

```js
for (const [mode, { tab }] of Object.entries(NOTE_MODES))
  tab.addEventListener("click", () => setNoteMode(mode));
```

This replaces task 10's two explicit `noteEditTab` / `notePreviewTab` click listeners.
Autosave, `loadNote`, and every task handler are untouched.

## 4. `web/index.html`

Add the **Live** tab between Edit and Preview, and wrap the two views in `#note-views`:

```html
<section class="pane note-pane">
  <div class="note-head">
    <h1>note</h1>
    <div class="note-tabs" role="group" aria-label="Note view">
      <button type="button" id="note-edit-tab" aria-pressed="true">Edit</button>
      <button type="button" id="note-live-tab" aria-pressed="false">Live</button>
      <button type="button" id="note-preview-tab" aria-pressed="false">Preview</button>
    </div>
  </div>
  <div id="note-views" class="note-views">
    <textarea id="note-input" class="note-input" aria-label="Note (Markdown)" placeholder="Notes (Markdown)…"></textarea>
    <div id="note-preview" class="note-preview markdown-body" hidden></div>
  </div>
  <div class="note-bar">
    <span id="note-status" class="note-status" role="status"></span>
  </div>
</section>
```

Every existing id (`note-input`, `note-preview`, `note-status`, the tab ids) is unchanged,
so all `app.js` lookups and autosave keep working; the only structural change is the
`#note-views` wrapper and the one new button.

## 5. `web/style.css`

- `.note-views` — default behaviour: a plain block whose children (textarea, preview)
  are full-width and stacked, exactly the current single-view geometry. The `hidden`
  view is not rendered, so Edit/Preview look identical to today.
- `.note-views.is-split` — Live only: `display: flex; gap: 1rem;` with each child
  `flex: 1; min-width: 0;` so the editor and preview share the pane width evenly without
  overflow. `.note-input` and `.note-preview` already share the same height envelope, so
  the two columns line up.
- A narrow-screen guard: under the existing `max-width: 720px` breakpoint (where the two
  panes already stack), `.note-views.is-split` switches to `flex-direction: column` so
  Live stacks editor-above-preview instead of cramming two columns into a phone width.

No existing rule is modified; `.note-input`, `.note-preview`, `.markdown-body`,
`.note-head`, and `.note-tabs` are all reused as-is. This is purely additive.

## 6. `README.md`

Update the note-pane paragraph (the WebUI section): the toggle is now **Edit / Live /
Preview**. Describe **Live** as "editor and rendered preview side by side, updating as
you type (Notion/Obsidian-style live preview)", keeping the existing description of Edit
(raw editor), Preview (full-width reading view), and the supported Markdown subset. Note
that Live reuses the same client-side renderer — the stored file still stays raw Markdown.

## 7. Tests and verification

No new automated test: the renderer and toggle are client-side JS and the repo has no JS
test harness by its deliberate no-build decision — the same constraint-bound position as
task 10 and every prior WebUI feature. `src/` is untouched, so the entire Rust suite
still applies as a pure regression guard, and `cargo build` re-embeds the edited assets
via `include_str!` (a malformed `index.html` fails the build).

- **Rust gates:** `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check` green.
- **Manual (the Live mode):** `cargo run`, open `http://127.0.0.1:8080`:
  - The note pane shows **Edit | Live | Preview**; it opens in Edit (textarea) exactly as
    before; Edit and Preview behave exactly as task 10 shipped them.
  - Click **Live**: editor and preview appear side by side; type a heading, `**bold**`,
    a `-` list, a fenced code block — the preview updates on each keystroke and matches.
  - Scroll the preview down in Live, then keep typing: the preview holds its scroll
    position (does not jump to the top) — confirms the `scrollTop` save/restore.
  - Switch Live → Preview → Edit → Live: visibility and the active-tab highlight follow
    the selected mode; autosave still shows `Editing…`/`Saving…`/`Saved` in every mode.
  - Security regression (renderer unchanged, re-confirm in Live): typing
    `<script>alert(1)</script>` shows literal text (no alert); a
    `[x](javascript:alert(1))` link renders `href="#"`.
  - Narrow the window below 720px in Live: editor and preview stack vertically instead of
    overflowing.

## 8. Risks

- **Per-keystroke render cost on a very large note.** Live rebuilds the preview's
  `innerHTML` on every input event. For a personal note this is imperceptible
  (`renderMarkdown` is a single O(n) pass and notes are small). Accepted as-is for the
  immediate "lively" feel; if a pathologically large note ever made typing lag, the fix
  is a short `requestAnimationFrame`/timeout coalesce in the one `input` line — localised
  and non-structural. Edit and Preview pay zero (the render is gated on `noteMode`).
- **Scroll jump while typing.** Replacing `innerHTML` resets `scrollTop` to 0, which
  would snap a long preview to the top mid-edit. Mitigated directly by saving/restoring
  `scrollTop` in `renderNotePreview` (section 3.2).
- **XSS via `innerHTML`.** Unchanged from task 10 and re-exercised by Live: every
  user run is `escapeHtml`-ed before it becomes HTML, `safeHref` neuters non-safe link
  targets, links carry `rel=noopener`. The renderer is reused verbatim, so its security
  properties carry over; the manual checklist re-confirms them in the new mode.
- **Two-column layout on small widths.** Two side-by-side panes inside one pane would be
  cramped on a phone. Mitigated by the `max-width: 720px` column fallback (section 5),
  consistent with how the outer panes already stack.
- **Renderer correctness still uncovered by CI.** Same standing no-build constraint as
  task 10; mitigated by reusing the already-hand-traced renderer unchanged (this feature
  adds no new parsing logic) and by the manual checklist. The server half stays covered.

## 9. Why a split Live view, client-side, not WYSIWYG or the server

- **Conceptual integrity / minimal surface.** The note is the canonical *raw* store;
  all rendering already lives on the client (task 10). Live preview is the same render,
  shown continuously beside the editor — it adds a mode and a layout, and **reuses the
  existing renderer untouched**, rather than introducing a second rendering path. This is
  the smallest change that satisfies "preview lively while edit".
- **No-build / no-dependency constraint.** A true Notion/Obsidian single-surface WYSIWYG
  editor means a `contenteditable` editing engine (selection, input, IME, undo,
  serialization back to Markdown) — effectively a Markdown editor framework, which would
  demand a library and a build step and re-implement editing itself. The split view gives
  the same "see it render as you type" outcome with a plain `<textarea>` and the renderer
  already in the tree. Honouring the repo's deliberate vanilla, no-build design.
- **Cost.** The tradeoff is the per-keystroke render (section 8), accepted as
  imperceptible for a personal note and trivially coalescable if it ever isn't. A
  WYSIWYG surface would be far more code and risk for a single-user note pane; a
  server-side live render would add a round-trip per keystroke and split the
  raw-store/presentation rule across tiers — both strictly worse for a view feature.

## 10. Files touched

- `web/app.js` — `noteMode` state, the `NOTE_MODES` descriptor table, `renderNotePreview`
  helper, the three-mode `setNoteMode`, the Live render in the `input` handler, and the
  `NOTE_MODES`-driven tab wiring (replacing the two task-10 click listeners). Two new
  `els` (`noteLiveTab`, `noteViews`). Renderer functions unchanged (section 3).
- `web/index.html` — the **Live** tab and the `#note-views` wrapper (section 4).
- `web/style.css` — `.note-views` and `.note-views.is-split` + the narrow-screen
  fallback (section 5).
- `README.md` — the third **Live** mode (section 6).
- `tasks/11-note-md-live-preview/implemented.md` — written after implementation.

## 11. brooks-review outcome

`tasks/11-note-md-live-preview/plan-review.md` scored the plan 99/100 on the first pass
with one 🟢 Suggestion (R3 Knowledge Duplication): the original draft kept the mode set in
a `NOTE_TABS` map but also re-encoded "which views each mode shows" as hand-written
booleans (`mode === "edit" || mode === "live"`) and re-tested `mode === "live"` for the
split toggle and the live render — four sites to keep in sync. Adopted: section 3.1 now
uses a single `NOTE_MODES` descriptor table carrying each mode's `tab`, `input`, and
`preview` flags, and `setNoteMode` (3.3), the `input` handler (3.4), and the tab wiring
(3.5) all read from it — one home for the mode set. The re-review found no further issues
(score 100/100). The only standing note is the constraint-bound absence of a JS test
harness for the client renderer — the same no-build decision task 10 accepted; the
renderer is reused unchanged and the server half stays covered by the Rust suite.
