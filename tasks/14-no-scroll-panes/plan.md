# Plan: No page scroll — fixed panes with internal scroll regions (branch `no-scroll-panes`)

Make the WebUI fill the viewport with **no page-level scroll**. Instead, scrolling moves
*inside* each pane where it belongs:

- **todo pane** — the "menu" (the `todo` heading, the add form, and the filter/sort row)
  **stays fixed on top**; only the task list scrolls.
- **note pane** — the editor surface itself scrolls; the heading and the status bar stay put.

This is a layout-only change. It is **CSS-first** with one small structural wrapper in
`index.html` (the todo list region) and **no JavaScript and no behaviour change** — the
note editor, autosave, task rendering, filters, and the server are all untouched.

## 1. Goal and scope

Requirement (from `agents/ADD_NEW_FEATURE.md`):

> - No scroll
>   - todo
>     - menu should stay on top
>     - only tasks scroll
>   - note
>     - Make scroll inside note editor

Chosen interpretation: the application chrome (both pane headers, the todo add/filter
controls, the note status bar) is always visible; the page (`body`) does not scroll. Each
pane owns a single internal scroll region — the **task list** in the todo pane and the
**editor surface** in the note pane — and those regions flex to fill whatever height the
viewport leaves after the fixed chrome.

In scope (WebUI only):

- `web/style.css` — turn the existing flow layout into a viewport-height flex column layout
  with two internal scroll regions; reset it back to natural page scroll on narrow screens.
- `web/index.html` — wrap the todo list region (`ul#tasks` + the `#empty` and `#error`
  messages) in one `<div class="task-list">` so the "list" is a single scroll container
  distinct from the menu above it. No other markup changes.
- `README.md` — one sentence in the WebUI section noting the fixed-chrome / internal-scroll
  layout.
- `tasks/14-no-scroll-panes/implemented.md` — after implementation.

Out of scope (explicit non-goals):

- **No JavaScript change.** No height measurement, no resize handlers — the layout is pure
  CSS flexbox, so it self-adjusts to viewport and content with zero script. `web/app.js`,
  `src/`, the API, and the stored files are untouched.
- **No element-id changes.** `app.js` looks up `#tasks`, `#empty`, `#error` by id; wrapping
  them in a div does not change those lookups. No handler is touched.
- **No change to the note editor** (highlighter, caret, autosave) or the **task pane logic**
  (filters, sort, add, edit). Only where the scrollbar lives changes.
- **No removal of mobile usability.** On narrow screens the panes already stack into one
  column; there, two independent fixed-height scroll regions stacked in one viewport would
  be cramped, so the no-scroll layout is **deliberately disabled below 720px** and the page
  returns to its current natural scroll. See section 4.

## 2. The model: one fixed-height column, two flex scroll regions

The whole change rests on one idea:

> **`main` is exactly one viewport tall and lays its content out as a flex column. Every
> ancestor of a scroll region is a flex column with `min-height: 0`, so the scroll region
> (`flex: 1`) takes the leftover height and scrolls its own overflow instead of growing the
> page.**

The `min-height: 0` on each flex-column ancestor is the crux: a flex item's default
`min-height` is `auto`, which refuses to shrink below its content and would push the overflow
out to the page (re-introducing the page scrollbar). Setting it to `0` lets the chain shrink
to the viewport so the innermost `overflow: auto` element is what actually scrolls.

The ancestor chain for each scroll region:

```
body (no scroll)
└─ main            height: 100dvh; display:flex; column        (the one viewport-tall box)
   └─ .panes       flex: 1; min-height: 0; display:flex (row)  (fills under nothing else)
      ├─ .note-pane  display:flex; column; min-height: 0
      │   ├─ h1            (fixed chrome)
      │   ├─ .note-input   flex: 1; min-height: 0; overflow:auto   ← SCROLLS
      │   └─ .note-bar     (fixed chrome: status)
      └─ .todo-pane  display:flex; column; min-height: 0
          ├─ h1 / #add-form / #filters   (fixed chrome: the "menu")
          └─ .task-list  flex: 1; min-height: 0; overflow-y:auto   ← SCROLLS
              └─ ul#tasks / #empty / #error
```

`main`'s only in-flow child is `.panes` (the `<dialog>` is top-layer/out-of-flow), so
`.panes` alone takes the leftover height under `main`'s padding. `box-sizing: border-box`
is already global (`style.css:11`), so `main`'s `2rem 1rem` padding is *inside* the
`100dvh`, not added to it — the box stays exactly one viewport tall and `body` never
overflows.

## 3. `web/style.css`

### 3.1 Kill the page scroll, make `main` one viewport tall

```css
/* DESKTOP FIXED-HEIGHT LAYOUT (the no-scroll mode). Every declaration tagged with the
   marker below is reversed in the ≤720px block (section 3.5) to restore page scrolling on
   narrow screens; keep the two blocks in sync — adding a viewport-pinning rule here means
   adding its undo there. */
body {
  /* …existing margin/background/color/font… */
  overflow: hidden;            /* [fixed-height] the page never scrolls; panes scroll internally */
}

main {
  /* …existing max-width / margin auto / padding… */
  height: 100dvh;              /* [fixed-height] exactly one viewport tall (border-box keeps padding inside) */
  display: flex;
  flex-direction: column;
}
```

The `[fixed-height]` marker is the one home for "this declaration is part of the desktop
no-scroll layout": every marked rule (here and in 3.3/3.4) has a matching reversal in the
reset block, so a reader changing the layout can grep the marker and find both halves.

`100dvh` (dynamic viewport height) tracks mobile browser chrome correctly; it is moot on
mobile anyway because section 4 disables this whole layout below 720px, but it is the right
unit for the desktop case and degrades to "viewport height" everywhere modern.

### 3.2 Let `.panes` and each `.pane` fill and shrink

```css
.panes {
  /* …existing display:flex; gap; … */
  flex: 1;                     /* fill the height main leaves under its padding */
  min-height: 0;              /* allow children to shrink so inner overflow scrolls */
  align-items: stretch;        /* was flex-start: panes must be full-height to host a scroller */
}

.pane {
  /* …existing flex:1; min-width:0; … */
  display: flex;
  flex-direction: column;
  min-height: 0;              /* same shrink-to-fit reason, one level down */
}
```

Note the `align-items` change: today `.panes` uses `align-items: flex-start` so each pane is
only as tall as its content. For an internal scroll region the pane must be **full column
height**, so it becomes `stretch` (the flex default). This is the one existing declaration
that is replaced rather than added.

### 3.3 Note pane — the editor is the scroll region

```css
.note-input {
  /* …existing border / radius / padding / font / colors / white-space / wrap … */
  flex: 1;                     /* [fixed-height] take the height left under the h1 and the status bar */
  min-height: 0;              /* [fixed-height] shrink to the pane; its own overflow:auto then scrolls */
  /* overflow: auto;  already present — this is what now actually scrolls */
  /* min-height: 60vh;  REMOVED here: a 60vh floor would exceed the available pane height and
     re-push the page; the flex:1 height replaces it for the desktop layout. The 60vh floor
     is the page-scroll-mode editor height and is restored only in the narrow-screen reset
     (section 3.5) where the page scrolls again. */
}
```

`h1` and `.note-bar` are ordinary block children with their own intrinsic height, so they
stay fixed and the editor flexes between them.

### 3.4 Todo pane — only the list scrolls, the menu stays

The `todo` heading, `#add-form`, and `#filters` are in-flow blocks above the new
`.task-list`, so they pin to the top automatically; the list region flexes and scrolls:

```css
.task-list {
  flex: 1;                     /* [fixed-height] fill the height under the heading + add form + filters */
  min-height: 0;
  overflow-y: auto;            /* [fixed-height] only the task list scrolls */
}
```

`#empty` ("No tasks.") and `#error` live *inside* `.task-list`, so when the list is short
they sit just under it and when it is long they scroll with it — the list region is one
coherent scroll area rather than the menu and the messages drifting independently.

### 3.5 Narrow-screen reset (≤720px): back to natural page scroll

The existing `@media (max-width: 720px)` block already stacks the panes into one column. Two
fixed-height internal scrollers stacked inside a single non-scrolling viewport would each be
painfully short, so below 720px we **undo** the no-scroll layout and let the page scroll as
it does today:

```css
@media (max-width: 720px) {
  .panes { flex-direction: column; }   /* existing */

  /* UNDO the [fixed-height] desktop layout above (sections 3.1/3.3/3.4): on narrow screens
     the page scrolls and the panes flow naturally. One reversal per [fixed-height] rule. */
  body { overflow: auto; }             /* page scrolls again */
  main { height: auto; }               /* not pinned to one viewport */
  .note-input { flex: none; min-height: 60vh; }  /* restore the page-scroll-mode editor height */
  .task-list { flex: none; overflow-y: visible; } /* list grows with the page */
}
```

The `.panes`/`.pane` flex-column declarations are harmless when `main` is `height: auto`
(flex children just take content height with nothing to fill), so they need no reset — only
the three rules that *force* a fixed height (`body overflow`, `main height`, and the two
scroll-region floors/overflows) are undone. This keeps the reset minimal and the desktop and
mobile intents each readable in one place.

## 4. `web/index.html`

Wrap the three list-region elements in one container; nothing else changes:

```html
        <ul id="tasks"></ul>
        <p id="empty" hidden>No tasks.</p>
        <p id="error" role="alert" hidden></p>
```

becomes

```html
        <div class="task-list">
          <ul id="tasks"></ul>
          <p id="empty" hidden>No tasks.</p>
          <p id="error" role="alert" hidden></p>
        </div>
```

The ids `tasks`, `empty`, `error` are unchanged, so every `app.js` lookup
(`getElementById`) and every handler keeps working; only their shared parent gains a class
that becomes the todo scroll region (section 3.4).

## 5. `README.md`

Add one sentence to the WebUI section: the page fills the viewport without scrolling — each
pane keeps its chrome fixed (the todo add/filter controls and the note status bar) while the
task list and the note editor scroll internally; on narrow screens the panes stack and the
page scrolls normally. No other prose changes.

## 6. Tests and verification

No automated test: this is presentation-only CSS/markup, and the repo has no JS/CSS test
harness by its standing no-build decision (tasks 10–13). `src/` is untouched, so the Rust
suite is unaffected; `cargo build` re-embeds the edited `index.html`/`style.css` via
`include_str!`, so a malformed `index.html` fails the build — that is the structural guard.

- **Rust gates (regression):** `cargo test`, `cargo clippy --all-targets`,
  `cargo fmt --check` green (expected: unaffected — no `src/` change).
- **Manual (`cargo run`, open `http://127.0.0.1:8080` at a desktop width):**
  - The window shows no page scrollbar at any task-count; the browser viewport is fully used.
  - Add enough tasks to overflow: the **task list scrolls** while the `todo` heading, the add
    form, and the filter row stay fixed at the top of the todo pane.
  - Type/paste a long note: the **editor scrolls internally** while the `note` heading and the
    status line stay put; the rest of the page does not move.
  - The note status line (`Editing…`/`Saving…`/`Saved`) and the `#empty`/`#error` messages
    remain reachable (status pinned under the editor; messages scroll within the list).
  - Resize below 720px: the panes stack and the **page scrolls** normally (no trapped inner
    scrollers); the editor shows its 60vh height again. Resize back above 720px: the
    no-scroll two-column layout returns.

## 7. Risks

- **`min-height: 0` omitted on a flex ancestor → page scrollbar returns.** The classic
  flexbox overflow trap: any flex-column ancestor of a scroll region that keeps its default
  `min-height: auto` refuses to shrink and pushes overflow to the page. Mitigated by setting
  `min-height: 0` on every link of the chain (`.panes`, `.pane`) and verifying in the manual
  checklist that no page scrollbar appears.
- **Mobile cramping.** A single non-scrolling viewport holding two stacked fixed-height
  scrollers would be unusable on a phone. Mitigated by disabling the entire no-scroll layout
  below 720px (section 3.5) and restoring today's page-scroll behaviour there.
- **`100dvh`/`dvh` support.** `dvh` is broadly supported in current browsers; on any engine
  that lacked it the rule would be ignored and the page would simply fall back to natural
  height (today's behaviour) — a graceful, not broken, degradation. The desktop target uses
  modern browsers, and mobile bypasses this rule entirely.
- **A future element added after `.task-list` in the todo pane** would sit below the scroll
  region and could be clipped. Low risk (none today); noted so the next editor keeps new
  fixed chrome *above* `.task-list` and new scrollable content *inside* it. The wrapper makes
  that boundary explicit rather than implicit.
- **`align-items: stretch` change.** Switching `.panes` from `flex-start` to `stretch` makes
  both panes full-height; a shorter pane no longer hugs its content. This is intended (a pane
  must be full height to host a bottom-pinned status bar / fill the column) and is visually
  identical for the taller pane; the manual check confirms neither pane mis-sizes.

## 8. Why CSS-only flexbox, not JS height math or `position: fixed`

- **Conceptual integrity.** The layout is expressed once, declaratively, in the place layout
  already lives (`style.css`). A JS approach (measure viewport, subtract header heights, set
  pixel heights on scroll regions, re-run on resize) would add a second source of truth for
  layout that must be kept in sync with the CSS and the markup — exactly the kind of
  duplicated knowledge the note series worked to avoid.
- **No-build / no-dependency constraint, still honoured.** Flexbox `min-height: 0` +
  `overflow: auto` is the standard, dependency-free way to get fixed chrome with internal
  scroll; it needs no library and no script.
- **`position: fixed`/`sticky` rejected.** Pinning the menu with `position: sticky` or
  `fixed` would leave the page itself scrollable (only the *appearance* of a fixed menu) and
  require manual spacer/offset math so content doesn't hide under the menu — more fragile and
  not actually "no scroll". The flex-column approach makes the page genuinely non-scrolling
  and lets the browser do the height arithmetic.

## 9. Files touched

- `web/style.css` — `body { overflow: hidden }`; `main` height + flex column; `.panes`
  flex/min-height/`align-items: stretch`; `.pane` flex column + min-height; `.note-input`
  `flex: 1`/`min-height: 0` (drop the `60vh` floor); new `.task-list` scroll region; the
  `@media (max-width: 720px)` reset (section 3.5).
- `web/index.html` — wrap `ul#tasks` + `#empty` + `#error` in `<div class="task-list">`
  (section 4).
- `README.md` — one sentence on the fixed-chrome / internal-scroll layout (section 5).
- `tasks/14-no-scroll-panes/implemented.md` — after implementation.

## 10. brooks-review outcome

`tasks/14-no-scroll-panes/plan-review.md` scored the first draft 98/100 with two 🟢
Suggestions, both adopted:

- **R2 (Change Propagation / Information Leakage):** the desktop "fixed-height" rule set and
  its narrow-screen reversal were two lists that had to be kept in sync with nothing linking
  them. Adopted: section 3 now tags every desktop-only declaration with a `[fixed-height]`
  marker and the reset block (3.5) reverses one rule per marker with a cross-reference
  comment, so both halves of the single decision are grep-discoverable together. The existing
  single-breakpoint, desktop-first convention is kept (no second `721px` breakpoint).
- **R1 (Cognitive Overload — magic number):** the `60vh` editor floor reappeared in the reset
  with no rationale. Adopted: section 3.3 and the reset comment now name it as the
  page-scroll-mode editor height (the floor the layout used before this change).

The re-review found no further issues (100/100).
