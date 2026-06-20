# Implemented: No page scroll — fixed panes with internal scroll regions

Branch `no-scroll-panes`. Layout-only change; no JavaScript, no `src/`, no API/behaviour change.

## What changed

The WebUI now fills the viewport with no page-level scroll. Scrolling happens inside each pane:

- **todo pane** — the `todo` heading, the add form, and the filter/sort row stay fixed on top;
  only the task list scrolls.
- **note pane** — the editor surface scrolls internally; the heading and the status bar stay put.

Below 720px the no-scroll layout is disabled and the page scrolls normally (panes stacked), as
before.

## Files touched

- `web/index.html` — wrapped `ul#tasks` + `#empty` + `#error` in `<div class="task-list">`
  (the todo scroll region). Element ids unchanged, so every `app.js` `getElementById` lookup
  still resolves.
- `web/style.css`
  - `body { overflow: hidden }` and `main { height: 100dvh; display: flex; flex-direction: column }`
    — page is one viewport tall, no page scroll.
  - `.panes` — `flex: 1; min-height: 0` and dropped `align-items: flex-start` (back to the
    `stretch` default) so panes are full column height and can shrink to the viewport.
  - `.pane` — `display: flex; flex-direction: column; min-height: 0` so each pane hosts an
    internal scroll region.
  - `.note-input` — `flex: 1; min-height: 0` (dropped the base `min-height: 60vh`); the existing
    `overflow: auto` is now what scrolls.
  - `.task-list` — new rule: `flex: 1; min-height: 0; overflow-y: auto`.
  - `@media (max-width: 720px)` — reverses the desktop layout: `body { overflow: auto }`,
    `main { height: auto }`, `.note-input { flex: none; min-height: 60vh }`,
    `.task-list { flex: none; overflow-y: visible }`.
  - Every desktop-only declaration carries a `[fixed-height]` comment marker; the media query
    reverses one rule per marker, so the two blocks stay grep-linked (plan-review R2).
- `README.md` — one sentence in the WebUI section describing the fixed-chrome / internal-scroll
  layout and the narrow-screen fallback.

## Verification

- `cargo fmt --check` — clean.
- `cargo clippy --all-targets` — clean.
- `cargo test` — 8 passed, 0 failed (Rust suite unaffected; assets re-embed via `include_str!`,
  so a malformed `index.html` would fail the build — it built).
- Manual checklist (plan section 6) to be confirmed in-browser: no page scrollbar at desktop
  width, task list scrolls under a fixed menu, note editor scrolls under a fixed heading/status,
  and below 720px the page scrolls with the panes stacked.

## Plan / review trail

- `plan.md` — design (100/100 after re-review).
- `plan-review.md` — first brooks-review of the plan (98, two 🟢 suggestions).
- `review-plan.md` — re-review after adopting both suggestions (100).
