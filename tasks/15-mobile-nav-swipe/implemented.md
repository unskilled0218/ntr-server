# Implemented: Mobile UI — bottom nav + swipe to switch views

Branch `mobile-nav-swipe`. WebUI-only; no `src/`, API, store, note-editor, or task-logic change.

## What changed

On narrow screens the two panes become two **views** of a one-screen app:

- A fixed **bottom navigation bar** (Note / Todo) selects the active view; the active segment
  is highlighted and carries `aria-current="page"`.
- A horizontal **swipe** on the content area switches to the adjacent view (left → next,
  right → previous, clamped — no wrap).
- The active view fills the screen above the bar and scrolls internally; the other pane is
  hidden.

Desktop (> 720px) is unchanged: both panes side by side, no bottom bar, swipe disabled.

## Key design points

- **One source of truth for view order:** `VIEWS = ["note", "todo"]` in `app.js` drives the
  nav order, the swipe direction, and the default fallback.
- **One switch function:** `setView(view)` sets `<main data-view>` and updates the nav buttons;
  both the nav click and the swipe call it.
- **CSS owns the breakpoint.** JS never re-encodes `720px`; the swipe handler asks whether the
  mobile layout is active via `getComputedStyle(els.nav).display !== "none"` (`onMobile()`), so
  the breakpoint lives once, in the `@media` rule.
- **Correct first paint without JS.** The default view is declared in markup
  (`<main data-view="note">`); init calls `setView(els.main.dataset.view || VIEWS[0])` only to
  sync the nav highlight.
- **Swipe is non-intrusive:** recognised on `touchend` only, with a 50px threshold and a
  horizontal-dominance test (`|dx| > |dy|`), and both listeners are `passive: true` (we never
  `preventDefault`), so vertical scrolling and note text-selection are unaffected.

## Files touched

- `web/index.html` — `<main data-view="note">`; pane ids `note-view` / `todo-view`;
  `<nav class="bottom-nav">` with two view buttons (`data-view` + `aria-controls`).
- `web/app.js` — new `els` handles (`main`, `panes`, `nav`, `navButtons`); `VIEWS`, `setView`,
  nav-click wiring; `onMobile()` + touchstart/touchend swipe handlers; the
  `setView(els.main.dataset.view || VIEWS[0])` init call.
- `web/style.css` — `.bottom-nav { display: none }` by default; rewrote the
  `@media (max-width: 720px)` block to show one pane + the bottom bar (replacing the task-14
  page-scroll reset); simplified the now-obsolete `[fixed-height]` marker comments (the
  fixed-height layout applies at every width now, and the `60vh` floor is gone).
- `README.md` — describes the mobile bottom-nav / swipe behaviour.

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` (8 passed) — all clean
  (Rust unaffected; assets re-embed via `include_str!`, so malformed markup fails the build).
- `node --check web/app.js` — JS parses.
- Ran the server and confirmed it serves the updated `index.html` (data-view, bottom-nav, pane
  ids), `style.css` (bottom-nav rules), and `app.js` (`VIEWS`/`setView`/`onMobile`).
- In-browser touch/swipe behaviour to be confirmed manually per plan section 7.

## Plan / review trail

- `plan.md` — design (100/100 after re-review).
- `plan-review.md` — first brooks-review of the plan (94: 1 🟡 breakpoint duplication, 1 🟢
  first-paint default).
- `review-plan.md` — re-review after adopting both (100).
