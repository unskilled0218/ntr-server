# Brooks-Lint Review

**Mode:** PR Review
**Scope:** tasks/15-mobile-nav-swipe/plan.md (design plan for the mobile bottom-nav + swipe UI)
**Health Score:** 94/100
**Trend:** 100 → 94 (−6) vs last PR Review run

A well-structured single-page view-switch plan; its one real flaw is that the 720px mobile
breakpoint is encoded in both the CSS and the JS, which can silently drift.

---

## Findings

### 🟡 Warning

**Change Propagation — the 720px breakpoint is duplicated across CSS and JS**
Symptom: The mobile boundary is written as `@media (max-width: 720px)` in `style.css` (which
decides single-pane layout and bottom-bar visibility) *and again* as
`window.matchMedia("(max-width: 720px)")` in `app.js` (section 4.3, which gates swipe). The same
design decision — "this width is mobile" — lives in two languages.
Source: Ousterhout — A Philosophy of Software Design (Information Leakage, Ch. 5); Hunt & Thomas
— The Pragmatic Programmer (DRY)
Consequence: Changing the breakpoint in one file and not the other makes swipe activate at a
different width than the single-pane layout — swipe firing while both panes show, or being dead
on a phone that got the single-pane layout. A latent, easy-to-miss bug exactly when the layout
is next tuned.
Remedy: Make CSS the single source of truth for "are we mobile" and have JS *read* that decision
instead of re-encoding the number — e.g. gate swipe on whether the bottom bar is currently
displayed: `getComputedStyle(els.nav).display !== "none"`. Drop the `matchMedia`/`720` literal
from `app.js`; the breakpoint then exists once, in the CSS.

### 🟢 Suggestion

**Accidental Complexity — initial view depends on JS running, risking a first-paint flash**
Symptom: `main` has no `data-view` until `setView(VIEWS[0])` runs, so on mobile both
`main[data-view=…]` rules miss and *both panes are hidden* until init (the plan acknowledges this
in section 8 as "imperceptible").
Source: McConnell — Code Complete (Ch. 5: Design in Construction — prefer correct-by-construction
state)
Consequence: Minor today (init is synchronous), but it makes correct first paint depend on JS
timing; if init ever gains an `await` before the call, mobile shows a blank screen for a beat.
Remedy: Declare the default in the markup — `<main data-view="note">` — so the correct pane
shows from first paint with no JS dependency, and at init call `setView(els.main.dataset.view)`
to sync the nav highlight to whatever the markup declared. The default then has one home (the
HTML attribute) and `VIEWS` stays purely the order, removing both the flash and any default
duplication.

---

## Summary

The view-as-data-view model, single `setView`, and `VIEWS` order-of-truth are sound, and folding
the task-14 mobile reset away is a genuine simplification. The must-fix is the duplicated 720px
breakpoint: route the JS mobile check through the CSS's own decision so the breakpoint lives in
one place. Adopt that plus the markup default and the plan is ready.
