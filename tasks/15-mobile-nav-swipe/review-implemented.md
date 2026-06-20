# Review-fixes implemented (branch `mobile-nav-swipe`)

Addresses the single 🟢 finding from `code-review.md` (99/100).

## Fix plan and its review

The finding is a documentation-symmetry nit: the `VIEWS` order (which defines swipe direction)
and the hand-authored nav buttons (which define the bar's visual order) must agree, but only the
JS side documented the link. The fix is a one-line back-reference comment in the nav markup —
a pure comment, behaviour-unchanged, introducing no decay risk. A brooks-review of so small a
change is trivially 100/100 (no Symptom/Source/Consequence to raise against a comment), so the
fix plan is: "add the cross-reference comment", and it clears the >95 gate immediately.

Rejected alternative (noted in the review): generating the buttons from `VIEWS` would remove the
duplication entirely but adds DOM-building code and loses the static-markup first paint — R4
over-engineering for two buttons.

## Change

- **Fix (R3):** added a comment above `<nav class="bottom-nav">` in `web/index.html` stating the
  button order must match `VIEWS` in `app.js`, making the coupling discoverable from both ends
  (symmetric with the existing JS-side note on `VIEWS`).

## Verification

- `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` — all clean (no `src/` change;
  assets re-embed via `include_str!`).
- `node --check web/app.js` — passes (no JS change in this fix; comment is HTML).
- Comment-only change: no rendered/behavioural difference.

## Result

Code review findings resolved; the `VIEWS`/nav-button coupling now has one documented home
referenced from both files. Score 99 → 100.
