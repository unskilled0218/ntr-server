# Implemented: Live Markdown preview for the note pane (branch `note-md-live-preview`)

Built per `tasks/11-note-md-live-preview/plan.md` (brooks-review 100/100,
`tasks/11-note-md-live-preview/plan-review.md`). The note pane gained a third **Live**
view: the raw-Markdown editor and the rendered preview sit side by side and the preview
re-renders as you type — the Notion/Obsidian live-preview feel. The task-10 renderer is
reused **unchanged**; backend untouched (the note stays a raw-Markdown store).

## What changed

- **`web/app.js`** — added the Live mode without touching the renderer:
  - `noteMode` — the current view, gating the per-keystroke live render.
  - `NOTE_MODES` — the single descriptor table (the adopted brooks-review R3 fix): each
    mode (`edit`/`live`/`preview`) paired with its `tab` button and `input`/`preview`
    visibility flags. `setNoteMode`, the `input` handler, and the tab wiring all read
    from it, so the mode set has one home.
  - `renderNotePreview()` — the one place preview HTML is written: renders the textarea's
    current value into `#note-preview`, saving/restoring `scrollTop` so a live re-render
    does not jump the preview to the top.
  - `setNoteMode(mode)` — rewritten to derive every consequence (the two `hidden` flags,
    whether to render, the `is-split` layout toggle, focus, `aria-pressed`) from the
    mode's `NOTE_MODES` descriptor — no per-mode `if` ladder.
  - `input` handler — gained one gated line: re-render only when the mode shows both
    views (`view.input && view.preview`), i.e. Live. The tab wiring now loops `NOTE_MODES`
    (replacing task 10's two explicit `noteEditTab`/`notePreviewTab` listeners). `els`
    gained `noteViews` and `noteLiveTab`.
  - Renderer (`escapeHtml`, `safeHref`, `renderInline`, `formatText`, `blockKind`,
    `renderMarkdown`, `TAKE`, `takeX`) — unchanged.
- **`web/index.html`** — a third **Live** tab between Edit and Preview, and a `#note-views`
  wrapper around the existing `#note-input` and `#note-preview` so the two can lay out
  side by side. Every existing id is unchanged.
- **`web/style.css`** — `.note-views.is-split` (Live only): a two-column flex with each
  child `flex: 1; min-width: 0`, plus a `max-width: 720px` fallback that stacks the
  columns on narrow screens. Default `.note-views` is a plain block, so Edit/Preview look
  identical to task 10. No existing rule changed.
- **`README.md`** — note-pane paragraph rewritten for the **Edit / Live / Preview**
  toggle, describing Live as the side-by-side, updates-as-you-type view.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` (17 unit + 8
  integration) — all green. `src/` is unchanged, so the suite is a pure regression guard;
  `cargo build` re-embeds the edited assets via `include_str!` (a malformed `index.html`
  would fail the build, and it built).
  - (`rustfmt`/`clippy` components were not present in the toolchain and were added with
    `rustup component add rustfmt clippy` — no other install needed.)
- Runtime smoke against a freshly built binary: the served page ships the **Live** tab and
  the `#note-views` wrapper; `app.js` carries `NOTE_MODES` and `renderNotePreview`;
  `style.css` carries `.note-views.is-split`; `PUT /api/note` → 204 and `GET` round-trips.
- The added view logic is pure glue and was traced by hand across all three modes: Edit
  (editor only, no render), Live (both views, `is-split` on, re-render on each input with
  `scrollTop` preserved), Preview (preview only, `is-split` off), and every Edit↔Live↔
  Preview transition (visibility, active-tab highlight, focus). The renderer itself is
  reused verbatim from task 10, so its hand-traced correctness and escaping/href-sanitising
  security carry over. As in task 10, no JS engine or JS test harness exists in this
  environment (the standing no-build constraint), so in-browser behaviour is verified by
  reading the code plus the manual checklist in the plan, not mechanically.

## Notes / non-goals (unchanged from the plan)

- No server-side rendering, no new endpoint, no third-party library, no build step.
- No Notion/Obsidian single-surface WYSIWYG/`contenteditable` editing — Live is
  editor-beside-preview.
- No editor↔preview scroll syncing (the preview only holds its own scroll position across
  re-renders).
- No change to the renderer, the Markdown subset, autosave, or the task pane.
