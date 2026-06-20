# Brooks-Lint Review

**Mode:** PR Review
**Scope:** code — branch changes vs main (commit 0f2bfce): web/app.js, web/index.html,
web/style.css, README.md
**Health Score:** 99/100
**Trend:** 100 → 99 (−1) vs last code review (task 14)

A faithful implementation of the plan: the breakpoint lives only in CSS, the markup default
gives a clean first paint, and swipe is correctly non-intrusive. One small finding — the
coupling between the `VIEWS` order and the hand-authored nav buttons is documented only on the
JS side.

---

## Findings

### 🟢 Suggestion

**R3 Knowledge Duplication — `VIEWS` order ↔ nav-button order coupling is documented on only one side**
Symptom: The swipe direction is computed from `VIEWS = ["note", "todo"]` (web/app.js:845) while
the bottom bar's left-to-right segments are authored independently in markup
(web/index.html:89–90). For a swipe to move toward the segment a user sees on that side, the two
orders must agree. The `VIEWS` comment notes "add a view here + a matching `<button>`", but the
nav markup has no back-pointer, so an editor reordering the buttons in HTML gets no hint that the
swipe order is defined elsewhere.
Source: Hunt & Thomas — The Pragmatic Programmer (DRY — one decision, one home); Brooks — The
Mythical Man-Month, Ch. 4 (Conceptual Integrity) — the same cross-reference standard task 14
applied to its `[fixed-height]` markers.
Consequence: Minor today (two views, both ordered note→todo in both places). If a third view is
added or the buttons reordered, swipe could move opposite to the bar's visual order with nothing
flagging the mismatch.
Remedy: Add a one-line comment in the nav markup pointing at `VIEWS` ("button order must match
VIEWS in app.js"), making the coupling discoverable from both ends — symmetric with the JS-side
note. (Generating the buttons from `VIEWS` would remove the duplication entirely but adds DOM-
building code and loses the static-markup first paint; not worth it for two buttons — R4.)

---

## Summary

The code matches the approved plan and the two plan-review fixes are present: the 720px
breakpoint is read from CSS via `onMobile()` (no JS copy), and the default view comes from
`<main data-view="note">`. The only finding is a documentation-symmetry nit on the
`VIEWS`/nav-button coupling; adding a back-reference comment in the markup brings it to 100. No
Critical/Warning issues. `cargo fmt`/`clippy`/`test` and `node --check web/app.js` are clean.
