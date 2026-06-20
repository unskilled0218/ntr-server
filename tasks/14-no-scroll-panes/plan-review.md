# Brooks-Lint Review

**Mode:** PR Review
**Scope:** tasks/14-no-scroll-panes/plan.md (design plan for the no-scroll layout)
**Health Score:** 98/100
**Trend:** 100 → 98 (−2) vs last PR Review run

A clear, appropriately-scoped CSS-first layout plan whose one real maintainability seam is the
desktop-layout / mobile-reset split that two CSS blocks must keep in sync.

---

## Findings

### 🟢 Suggestion

**Change Propagation — the "fixed-height" rule set and its mobile reset must stay in sync**
Symptom: The no-scroll layout is defined as a desktop default (`body{overflow:hidden}`,
`main{height:100dvh}`, `.note-input{flex:1;min-height:0}`, `.task-list{overflow-y:auto}`) and
then partially *undone* in the `@media (max-width:720px)` block (section 3.5). The list of
declarations that constitute "fixed-height mode" lives in one place and the list that must be
reversed lives in another.
Source: Ousterhout — A Philosophy of Software Design (Information Leakage, Ch. 5)
Consequence: A later editor who adds another viewport-pinning rule (or changes one) can fix the
desktop layout and silently leave the narrow-screen layout with a trapped inner scroller,
because nothing forces the reset block to be updated in lockstep.
Remedy: Make the coupling discoverable from both ends — add a short cross-referencing comment in
each block ("desktop fixed-height layout; the ≤720px block reverses these" / "undo the
fixed-height layout above") so the two halves of the single decision are found together. Keep the
existing single-breakpoint, desktop-first convention (do not introduce a second `721px`
breakpoint, which would split the breakpoint value across two magic numbers).

**Cognitive Overload — unexplained `60vh` constant reintroduced in the reset only**
Symptom: The `60vh` editor floor is dropped from the base rule (section 3.3) and reappears with
no rationale in the narrow-screen reset (section 3.5); a reader sees a bare magic number whose
purpose ("a comfortable editor height once the page scrolls again") is implicit.
Source: McConnell — Code Complete (Ch. 12: Fundamental Data Types — magic numbers)
Consequence: Minor — the next editor cannot tell whether `60vh` is meaningful or arbitrary, so it
is changed timidly or copied without understanding.
Remedy: Add a one-clause comment on the reset's `min-height: 60vh` stating it is the
page-scroll-mode editor height (the same floor the layout used before this change).

---

## Summary

The plan is sound: CSS-only, correctly identifies the `min-height: 0` flexbox trap as the crux,
preserves mobile usability, and keeps `app.js`/`src` untouched. The single highest-leverage
improvement is making the desktop-layout/mobile-reset coupling explicit so the two CSS blocks
cannot drift; the `60vh` comment is a trivial readability nicety. No Critical or Warning issues —
adopt both suggestions and the plan is ready to implement.
