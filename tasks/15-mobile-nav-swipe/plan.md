# Plan: Mobile UI — bottom nav + swipe to switch views (branch `mobile-nav-swipe`)

On narrow screens, replace the current "stack both panes and scroll the page" mobile layout
with a **one-pane-at-a-time app shell**: the viewport shows a single pane (note *or* todo),
a **bottom navigation bar** switches between them, and a **horizontal swipe** does the same.
Desktop (the side-by-side, no-scroll two-pane layout from task 14) is unchanged.

This is a WebUI-only change — `index.html`, `style.css`, `app.js`. The server, API, stored
files, note editor, and task logic are untouched; only *which pane is visible on mobile* and
*how you move between them* is new.

## 1. Goal and scope

Requirement (from `agents/ADD_NEW_FEATURE.md`):

> - Mobile UI
>   - Use bottom navigation bar to swap between view
>   - Swipe gesture support

Chosen interpretation: on mobile the two panes become two **views** of a single-screen app.
A fixed bottom bar with two segments (**Note**, **Todo**) selects the active view; a
left/right swipe moves to the adjacent view. The active view fills the screen above the bar
and scrolls internally (the same fixed-height / internal-scroll model task 14 gave desktop —
now usable on mobile precisely *because* only one pane shows at a time). On desktop both
panes remain side by side and the bottom bar is hidden.

In scope (WebUI only):

- `web/index.html` — add a `<nav class="bottom-nav">` with two view buttons; give the two
  `<section>` panes ids so the buttons can reference them for accessibility.
- `web/app.js` — a single `VIEWS` order-of-truth, a `setView(view)` function, bottom-nav
  click wiring, and touch swipe handling guarded to mobile widths; one init call.
- `web/style.css` — hide the bottom bar on desktop; on mobile show one pane + the bar, and
  **remove the task-14 mobile page-scroll reset** (mobile now keeps the fixed-height layout).
- `README.md` — describe the mobile bottom-nav / swipe behaviour.
- `tasks/15-mobile-nav-swipe/implemented.md` — after implementation.

Out of scope (explicit non-goals):

- **No change to desktop layout.** Above 720px the side-by-side no-scroll layout from task 14
  is byte-for-byte unchanged; the bottom nav is `display: none` and swipe is disabled there.
- **No animation / carousel transform.** Switching a view is an instant show/hide, not an
  animated slide. A slide transform would mean keeping both panes laid out off-screen and
  animating `transform` on every swipe — more code and state for cosmetic gain; rejected as
  accidental complexity (section 8). A future enhancement, not this change.
- **No view persistence.** The app opens on the first view (`note`) each load; no
  `localStorage`. The task did not ask for it and it adds a storage concern for little gain.
- **No new gesture beyond horizontal swipe** (no pinch, long-press, vertical pull). Vertical
  drags stay with the browser so the pane scrolls normally.
- **No server / API / store / note-editor / task-logic change.** `src/` untouched.

## 2. The model: two panes are two views, one source of truth for their order

One idea the whole feature rests on:

> **The two panes form an ordered list of views, `VIEWS = ["note", "todo"]`. The bottom-nav
> button order, the swipe direction math, and the default view all derive from this one
> array; the active view is recorded in a single place — `main`'s `data-view` attribute —
> which CSS reads to decide what shows on mobile.**

Consequences that fall out of this:

- **Switching views is one function**, `setView(view)`: set `main.dataset.view` and mark the
  matching nav button active. The nav click handler and the swipe handler both call it — they
  are two *inputs* to the same one state change, never two copies of the switch logic (R3).
- **CSS, not JS, decides visibility — and the breakpoint.** JS only writes `data-view`; the
  `@media (max-width: 720px)` rules show the matching pane, hide the other, and reveal the bar.
  Above 720px those rules do not apply, so `data-view` is inert and both panes always show.
  The swipe handler asks CSS whether the mobile layout is active (is the bar displayed?) rather
  than re-testing the width, so the 720px breakpoint is written **once**, in the stylesheet
  (R2/R3: no width literal duplicated between CSS and JS).
- **Swipe is direction + clamp over the same array.** Swipe-left → next index, swipe-right →
  previous, clamped to `[0, VIEWS.length)`. Adding a third view later is a one-line array
  edit plus one `<button>`; the swipe math is unchanged.

## 3. `web/index.html`

Give each pane an id (for `aria-controls`), declare the default view on `<main>` so the right
pane shows at first paint before any JS runs, and add the bottom nav as the last in-flow child
of `<main>` (after `.panes`, before the `<dialog>`):

```html
<main data-view="note"> <!-- default view; one home for the default — see setView init (4.2) -->
…
<section class="pane note-pane" id="note-view">…</section>
<section class="pane todo-pane" id="todo-view">…</section>
…
</div> <!-- .panes -->

<nav class="bottom-nav" aria-label="Switch view">
  <button type="button" data-view="note" aria-controls="note-view">Note</button>
  <button type="button" data-view="todo" aria-controls="todo-view">Todo</button>
</nav>
```

The buttons are real `<button>`s (keyboard- and screen-reader-operable for free);
`data-view` ties each to a `VIEWS` entry, `aria-controls` to the pane it reveals. `setView`
adds `aria-current="page"` to the active button (section 4). The nav sits inside `<main>` so
the existing flex column places it: `.panes` (flex: 1) fills, `.bottom-nav` (flex: none) pins
to the bottom. The `<dialog>` is top-layer/out-of-flow, so it is unaffected by nav order.

## 4. `web/app.js`

### 4.1 The one source of truth and the one switch function

```js
// The two panes as an ordered list of views: nav-button order, swipe direction, and the
// default view all derive from this. Add a view here + a matching <button> and nothing else
// changes. (THE VIEW ORDER — one home.)
const VIEWS = ["note", "todo"];

// The single place the active view changes: record it on main (CSS reads data-view on mobile)
// and reflect it on the nav buttons. Both the nav click and the swipe call this — one switch,
// two inputs.
function setView(view) {
  els.main.dataset.view = view;
  for (const btn of els.navButtons) {
    const active = btn.dataset.view === view;
    btn.classList.toggle("is-active", active);
    if (active) btn.setAttribute("aria-current", "page");
    else btn.removeAttribute("aria-current");
  }
}
```

New `els` handles (added to the existing object):

```js
main: document.querySelector("main"),
panes: document.querySelector(".panes"),
nav: document.querySelector(".bottom-nav"),
navButtons: Array.from(document.querySelectorAll(".bottom-nav button")),
```

### 4.2 Bottom-nav wiring and init

```js
for (const btn of els.navButtons) {
  btn.addEventListener("click", () => setView(btn.dataset.view));
}
```

Init (beside the existing `initPriorityOptions()` / `refresh()` calls):
`setView(els.main.dataset.view || VIEWS[0]);` — adopt the view declared in the markup
(`<main data-view="note">`, section 3) so the correct pane is already shown at first paint;
this init call only **syncs the nav-button highlight** to that markup default (and `VIEWS[0]`
is a defensive fallback if the attribute is ever absent). The default view therefore has one
home — the HTML attribute — and `VIEWS` stays purely the order. On desktop `data-view` is
inert (both panes show); on mobile the markup already selected the pane before JS ran.

### 4.3 Swipe handling (mobile only, horizontal-dominant)

Swipe is recognised on the `.panes` container, only when the single-pane mobile layout is
active, and only when the gesture is clearly horizontal — so a vertical drag still scrolls the
active pane and desktop is untouched:

```js
const SWIPE_MIN = 50; // px of horizontal travel before a drag counts as a view swipe

// "Are we in the mobile single-pane layout?" — read CSS's own decision instead of re-encoding
// the 720px breakpoint here, so the breakpoint lives in exactly one place (the @media rule).
// The bottom bar is display:none on desktop and shown on mobile, so its computed display IS
// the layout mode.
const onMobile = () => getComputedStyle(els.nav).display !== "none";

let swipeStartX = null;
let swipeStartY = null;

els.panes.addEventListener(
  "touchstart",
  (e) => {
    // Only single-finger gestures while the single-pane layout is active; else ignore.
    if (!onMobile() || e.touches.length !== 1) {
      swipeStartX = null;
      return;
    }
    swipeStartX = e.touches[0].clientX;
    swipeStartY = e.touches[0].clientY;
  },
  { passive: true },
);

els.panes.addEventListener(
  "touchend",
  (e) => {
    if (swipeStartX === null) return;
    const dx = e.changedTouches[0].clientX - swipeStartX;
    const dy = e.changedTouches[0].clientY - swipeStartY;
    swipeStartX = null;
    // Ignore taps, short drags, and vertical-dominant drags (those scroll the pane).
    if (Math.abs(dx) < SWIPE_MIN || Math.abs(dx) <= Math.abs(dy)) return;
    const cur = VIEWS.indexOf(els.main.dataset.view);
    const next = cur + (dx < 0 ? 1 : -1); // swipe left → next view, right → previous
    if (next >= 0 && next < VIEWS.length) setView(VIEWS[next]);
  },
  { passive: true },
);
```

Notes:
- **`passive: true`** on both: we never `preventDefault` (we only switch on `touchend` after
  the gesture completes), so the browser keeps native scrolling responsive and does not warn.
- **`onMobile()` reads the CSS decision** (`getComputedStyle(els.nav).display`) rather than a
  duplicated `matchMedia("(max-width: 720px)")`, so the breakpoint exists once in the stylesheet
  and swipe can never activate at a width where the layout still shows both panes.
- **Horizontal dominance** (`|dx| > |dy|`) plus the 50px threshold is what separates a
  view-swipe from a vertical scroll and from an accidental tap. Touch text-selection in the
  note editor begins with a long-press, not a quick horizontal drag, so the editor keeps its
  selection behaviour.
- **Clamp, no wrap:** at the first view a right-swipe does nothing; at the last, a left-swipe
  does nothing — predictable for a two-view app.
- **Listening on `.panes`** (not the document) scopes swipes to the content area and reuses
  the element that already wraps both panes.

## 5. `web/style.css`

### 5.1 Bottom nav: hidden on desktop, a fixed bar on mobile

```css
.bottom-nav { display: none; } /* desktop: both panes show side by side, no bar */
```

### 5.2 Mobile: one pane + the bar; keep the fixed-height layout

The task-14 `@media (max-width: 720px)` block currently *reverses* the fixed-height layout
(`body { overflow: auto }`, `main { height: auto }`, `.note-input`/`.task-list` floors) so the
page scrolls with both panes stacked. That reversal is **removed**: with only one pane visible
the fixed-height / internal-scroll layout is exactly what we want on mobile too. The block
becomes:

```css
@media (max-width: 720px) {
  /* One pane at a time: hide both, then show the one main's data-view selects. The page
     stays one viewport tall (from the base layout) and the visible pane scrolls internally;
     the bottom bar switches views. */
  .pane { display: none; }
  main[data-view="note"] .note-pane,
  main[data-view="todo"] .todo-pane {
    display: flex; /* re-show; flex-direction: column comes from the base .pane rule */
  }

  .bottom-nav {
    display: flex;
    flex: none; /* a fixed-height bar; .panes (flex: 1) takes the rest */
    border-top: 1px solid var(--border);
    background: #fff;
  }
  .bottom-nav button {
    flex: 1; /* two equal-width segments */
    border: none;
    border-radius: 0;
    background: none;
    padding: 0.75rem;
    color: var(--muted);
    font: inherit;
  }
  .bottom-nav button.is-active {
    color: var(--accent);
    font-weight: 600;
  }
}
```

`.pane { display: none }` (specificity 0,1,0) is overridden by
`main[data-view="…"] .note-pane` (0,2,1) for the active pane, both inside the media query, so
exactly one pane shows. Because this is all inside `@media (max-width: 720px)`, above 720px the
base `.pane { display: flex }` and the hidden `.bottom-nav` apply unchanged — desktop is
untouched and `data-view` is inert there.

### 5.3 What this simplifies (task-14 follow-through)

Removing the mobile reset deletes the `[fixed-height]`-marker reversal that task 14 added
(`body`/`main`/`.note-input`/`.task-list` overrides in the 720px block) **and the magic
`60vh` editor floor** that lived only in that reset. The base layout's `[fixed-height]`
comment markers — which existed only to point at that now-deleted reversal — are simplified to
plain comments, since the fixed-height layout now applies at every width and has no paired
"undo" to stay in sync with. This is a net reduction in CSS and removes the cross-block
coupling the task-14 review flagged.

## 6. `README.md`

Update the WebUI layout sentence: on wide screens the two panes sit side by side; on narrow
screens the app shows one pane at a time with a **bottom navigation bar** (Note / Todo) and
**horizontal swipe** to switch between them, each view filling the screen and scrolling
internally. Drop the "panes stack and the page scrolls" wording (no longer true on mobile).

## 7. Tests and verification

No automated test: this is presentation + small client-side interaction, and the repo has no
JS/CSS test harness by its standing no-build decision (tasks 10–14). `src/` is untouched, so
the Rust suite is unaffected; `cargo build` re-embeds the edited assets via `include_str!`, so
malformed `index.html` fails the build (the structural guard).

- **Rust gates (regression):** `cargo test`, `cargo clippy --all-targets`,
  `cargo fmt --check` green (expected: unaffected — no `src/` change).
- **Manual (`cargo run`, open `http://127.0.0.1:8080`):**
  - **Desktop (> 720px):** both panes side by side, no bottom bar, no page scroll — identical
    to task 14. Mouse use is unaffected.
  - **Mobile width (DevTools device toolbar or window < 720px):** exactly one pane shows; a
    bottom bar with **Note** / **Todo** appears; the active segment is highlighted.
  - Tap **Todo**: the todo pane replaces the note pane; tap **Note**: back. The active
    segment updates and carries `aria-current="page"`.
  - **Swipe left** on the note view → todo view; **swipe right** → back to note. At the note
    view a right-swipe does nothing; at todo a left-swipe does nothing (clamped).
  - **Vertical scroll** inside a long note or a long task list still scrolls the pane and does
    *not* switch views (horizontal-dominance guard).
  - The visible pane fills the screen above the bar and scrolls internally; the bar stays put.
  - Add tasks / edit the note on mobile: autosave, filters, and the label modal still work
    within the visible pane.
  - Resize across 720px both ways: layout flips between side-by-side and single-pane with no
    reload and no stuck-hidden pane.

## 8. Risks

- **Swipe vs. in-pane interaction (the central risk).** A horizontal drag could fight note
  text-selection or be mistaken for a scroll. Mitigated by: acting only on `touchend`, the
  50px threshold, the `|dx| > |dy|` horizontal-dominance test, and that mobile text-selection
  starts with a long-press rather than a quick horizontal drag. If an edge ever misfires it
  only switches view (no data effect) and a swipe back returns. `passive: true` keeps native
  scrolling smooth because we never call `preventDefault`.
- **iOS Safari edge-swipe back-navigation.** A swipe starting at the very screen edge may
  trigger the browser's back gesture instead of ours. Accepted: it is an OS-level gesture at
  the extreme edge; mid-screen swipes (the common case) are ours. Not worked around.
- **Reset removal changing desktop.** The deleted overrides live entirely inside
  `@media (max-width: 720px)`, so removing them cannot affect ≥ 720px. The manual desktop
  check confirms task-14 behaviour is intact.
- **First-paint view correctness.** The default view is declared in the markup
  (`<main data-view="note">`), so the correct pane shows from the very first paint with no
  dependency on JS timing; init only syncs the nav highlight. This removes the "both panes
  hidden for a frame" risk entirely and stays correct even if init later becomes async.
  (Desktop is unaffected — both panes show regardless of the attribute.)
- **dvh + a fixed bottom bar on mobile.** The bar is a flex child of the `100dvh` column, so it
  sits above the dynamic browser chrome and the visible pane gets the remaining height; `dvh`
  tracks the chrome. Same accepted unit as task 14.
- **A future third view.** The design scales to it (one `VIEWS` entry + one `<button>` + one
  `main[data-view=x] .x-pane` selector), but with > 2 views a clamp-no-wrap swipe and a
  growing flat bar may want revisiting. Noted, not built.

## 9. Why a CSS show/hide + data-view, not a JS carousel or separate pages

- **Conceptual integrity.** The two panes already exist as one document; making them two
  *views* of it via a single `data-view` attribute keeps one DOM, one set of handlers, and one
  source of truth for order. JS writes one attribute; CSS owns visibility. No second layout
  system, no route, no duplicated width logic.
- **No-build / no-dependency, still honoured.** Bottom nav + swipe is a `<nav>`, a `setView`,
  and two touch listeners — no router, no gesture library, no animation framework.
- **Carousel/slide rejected (accidental complexity).** An animated slide needs both panes laid
  out off-screen, transform/opacity state, and gesture-tracking during the move — significant
  code and edge cases (interrupted swipes, momentum) for a cosmetic transition on a two-view
  personal app. Instant show/hide is the smallest faithful implementation; animation can be
  layered on later without changing the `VIEWS`/`setView`/`data-view` model.
- **Separate mobile pages rejected.** Two routes would split the note and todo state and
  duplicate the shell; a single-page view switch keeps both panes live (e.g. autosave keeps
  running) and the code in one place.

## 10. Files touched

- `web/index.html` — `<main data-view="note">` default; pane ids `note-view`/`todo-view`;
  `<nav class="bottom-nav">` with two view buttons (section 3).
- `web/app.js` — `VIEWS`, `setView`, the new `els` handles (`main`/`panes`/`nav`/`navButtons`),
  nav-click wiring, the touch swipe handlers gated by `onMobile()` (which reads CSS's own
  decision, not a duplicated breakpoint), and the `setView(els.main.dataset.view || VIEWS[0])`
  init call (section 4).
- `web/style.css` — `.bottom-nav` hidden by default; the rewritten `@media (max-width: 720px)`
  block (one pane + bar, replacing the task-14 page-scroll reset); simplified base
  `[fixed-height]` comments (section 5).
- `README.md` — mobile bottom-nav / swipe description (section 6).
- `tasks/15-mobile-nav-swipe/implemented.md` — after implementation.

## 11. brooks-review outcome

`tasks/15-mobile-nav-swipe/plan-review.md` scored the first draft 94/100 (one 🟡 Warning, one
🟢 Suggestion); both adopted:

- **R2/R3 (Change Propagation / Knowledge Duplication) — Warning:** the 720px breakpoint was
  encoded in both the CSS `@media` rule and a JS `matchMedia`, so they could drift and make
  swipe active at a width where both panes still show. Adopted: the swipe handler now gates on
  `onMobile()` = `getComputedStyle(els.nav).display !== "none"`, reading CSS's own decision; the
  breakpoint literal exists once, in the stylesheet (sections 2, 4.3).
- **R4 (Accidental Complexity) — Suggestion:** the initial view depended on JS running, hiding
  both panes for a frame on mobile. Adopted: the default is declared in the markup
  (`<main data-view="note">`) so first paint is correct without JS, and init calls
  `setView(els.main.dataset.view || VIEWS[0])` only to sync the nav highlight — the default has
  one home (the HTML attribute) and `VIEWS` stays purely the order (sections 3, 4.2, 8).

The re-review (`review-plan.md`) found no further issues (100/100).
