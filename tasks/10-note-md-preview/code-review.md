# Code Review (committed implementation, `web/*` @ commit aa7ed4c)

**Mode:** PR Review
**Scope:** `web/app.js`, `web/index.html`, `web/style.css` (the preview feature as built)
**Health Score:** 100/100 (99/100 first pass, one Suggestion fixed)

The implementation matches the 100/100 plan; reviewing it as shipped code caught one
marker pattern the plan's R3 fix had missed — the blockquote prefix — now also a
constant.

## Findings

### 🟢 Suggestion — fixed in this branch

**Knowledge Duplication — the blockquote marker `/^>\s?/` was still written twice**
Symptom: The plan's marker-constant extraction named `LIST_ITEM`, `ORDERED_ITEM`, and
`FENCE`, but the blockquote pattern `/^>\s?/` remained inline in both `blockKind` (to
classify a quote line) and `takeBlockquote` (to strip the `>` prefix) — the exact
classify-here / strip-there duplication the constants were introduced to remove.
Source: Hunt & Thomas — The Pragmatic Programmer, DRY (the "what marks a blockquote line"
decision in two places); Fowler — Refactoring, Duplicate Code.
Consequence: Changing what counts as a quote line (e.g. requiring the space after `>`, or
allowing a leading indent) would need editing both copies; a missed copy desyncs the
classifier from the stripper, mis-rendering quotes — which the absent JS harness would
not catch.
Remedy: Add a `QUOTE_MARKER = /^>\s?/` constant and reference it from both sites, so the
blockquote marker joins the other three with a single home. **Fixed.**

## Remaining (100/100)

No further findings. Considered and dismissed:

- **The heading patterns `/^#{1,6}\s/` (classify) vs `/^(#{1,6})\s+(.*)$/` (capture).**
  Not the same expression — one detects, one parses with capture groups — so this is a
  detect-vs-parse pair, not duplicated knowledge (the same call made in the plan review).
  No finding.
- **Correctness — a link URL containing `)` is truncated** (`[x](https://e/Foo_(bar))`
  stops the href at the first `)`). A real bounded limitation of regex link parsing, not
  a security issue (the truncated target still passes `safeHref`), and balanced-paren
  handling would add parser complexity disproportionate to a reading-view subset. The
  README documents the renderer as a practical subset. Accepted, not a deduction.
- **Cognitive Overload / Accidental Complexity / Security.** Unchanged from the plan
  review: small pure functions, flat `TAKE` dispatch, escape-then-sanitise. No finding.
- **Coverage Illusion — renderer untested by CI.** Constraint-bound (no JS engine or
  harness available; the no-build decision the feature respects). The pure structure and
  the per-construct manual trace stand in; `src/` is untouched and stays covered. Not a
  deduction.

## Summary

The built feature is sound and matches the plan. The one issue — the blockquote marker
escaping the plan's R3 constant extraction — is fixed by adding `QUOTE_MARKER`, so all
four repeated block markers now have a single home. Score 100/100 after the fix.
