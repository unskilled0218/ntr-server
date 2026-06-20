# Review-fix plan (branch `note-inplace-preview`)

Address the three 🟢 findings from `code-review.md`. All three are pure, behaviour-preserving
refactors in `web/app.js`; the highlighter's output HTML is unchanged, so the Node
round-trip invariant test and the Rust gates remain the verification.

## Fix 1 — `highlightLine` → keyed `LINE` dispatch table (R4)

Replace the 7-branch if-ladder with a table keyed by line kind, mirroring task 10's `TAKE`
and tasks 11–12's `NOTE_MODES`:

```js
// One renderer per line kind -> the line's HTML. Keyed dispatch (like TAKE) so adding a
// kind is one entry, not a new conditional. highlightMarkdown only ever produces these
// seven kinds, so the table is total.
const LINE = {
  blank: () => `<div class="md-line"><br></div>`,
  code: (line) => {
    const body = escapeHtml(line);
    return `<div class="md-line md-code">${body === "" ? "<br>" : body}</div>`;
  },
  hr: (line) => `<div class="md-line md-hr">${mark(escapeHtml(line))}</div>`,
  heading: (line) => {
    const [, marker, text] = line.match(HEADING);
    return `<div class="md-line md-h${marker.trim().length}">${mark(escapeHtml(marker))}${highlightInline(escapeHtml(text))}</div>`;
  },
  quote: (line) => {
    const [, marker, text] = line.match(QUOTE_MARKER);
    return `<div class="md-line md-quote">${mark(escapeHtml(marker))}${highlightInline(escapeHtml(text))}</div>`;
  },
  list: (line) => {
    const [, indent, bullet, gap, text] = line.match(LIST_ITEM);
    return `<div class="md-line md-li">${escapeHtml(indent)}${mark(escapeHtml(bullet + gap))}${highlightInline(escapeHtml(text))}</div>`;
  },
  para: (line) => `<div class="md-line">${highlightInline(escapeHtml(line))}</div>`,
};
```

`highlightMarkdown`'s map body becomes `return LINE[kind](line);`. The `highlightLine`
function is removed (the table replaces it).

## Fix 2 — factor `formatInline`'s emphasis wrap (R3)

```js
// Wrap one emphasis run, re-emitting the fixed marker as dimmed marks so textContent is
// preserved. The four bold/italic branches share this; the link branch keeps its own shape.
const emph = (marker, cls) => (_, t) => `${mark(marker)}<span class="${cls}">${t}</span>${mark(marker)}`;
```

`formatInline` keeps the link `.replace(...)` as-is and uses `emph("**","md-strong")`,
`emph("__","md-strong")`, `emph("*","md-em")`, `emph("_","md-em")` for the four emphasis
replaces.

## Fix 3 — drop the redundant `blockKind` guard (R4)

`if (QUOTE_MARKER.test(line) && line.startsWith(">")) return "quote";` →
`if (QUOTE_MARKER.test(line)) return "quote";` (the `^>`-anchored regex already implies it).

## Verification

- Re-run the Node round-trip + XSS harness (same 12 cases) — output HTML is unchanged, so it
  must still pass 12/0 and stay href/script-free.
- `node --check web/app.js`; `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`.

## brooks-review outcome

`/brooks-review` of this fix plan: 100/100, no findings. All three changes restore idioms
already in the file (keyed dispatch mirroring `TAKE`, a factored callback like the `mark`
helper, dead-code removal) and preserve the highlighter's output HTML exactly, so they add
no duplication, complexity, or coupling and keep the verified char-preservation invariant.
Cleared to implement.
