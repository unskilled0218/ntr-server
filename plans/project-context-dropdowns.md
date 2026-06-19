# Plan: project/context dropdowns and A-C priority (branch `project-context-dropdowns`)

Rework the add form's structured controls: limit the priority picker to `A`-`C`,
and replace the single combined "Projects & contexts" multi-select with two
separate custom dropdown menus (one for projects, one for contexts). Each
dropdown lists the existing labels as multi-select checkboxes and carries an
"Add" button at the bottom that opens a modal for creating a new label.

## 1. Goal and scope

Requirements (from `agents/ADD_NEW_FEATURE.md`):

> - Priority: A ~ C, remove rest
> - Separate projects and contexts
> - Make projects and contexts dropdown menu
>   - Add button to add projects or contexts at the bottom of the dropdown menu
>   - When user clicks add button, a modal to add projects or contexts opens

In scope (WebUI only):

- Priority picker offers only a blank placeholder plus `A`, `B`, `C`.
- Two independent dropdown widgets, "Projects" and "Contexts", replacing the
  one `#add-tags` multi-select. Each is a toggle button + a popover panel of
  checkbox items (multi-select preserved), with an "+ Add project" / "+ Add
  context" button pinned at the bottom of the panel.
- A modal (native `<dialog>`) to create a new project/context label, opened by
  the panel's add button.
- Compose the todo.txt line from priority + description + selected projects
  (`+name`) + selected contexts (`@name`) + due date, through the existing
  `POST /api/tasks`.

Out of scope (explicit non-goals):

- No server/API/Rust changes. The contract already accepts a full todo.txt line
  via `{ "text": "..." }`; this feature only changes how the WebUI builds it,
  exactly as the prior `add-task-selects` feature was scoped. Keeping a single
  task-construction path avoids a second, divergent one in the API.
- No backend "label registry". todo.txt has no concept of a project/context
  that exists independently of a task (labels are derived from task
  descriptions), so there is nothing to persist a label into. See section 4.
- The parser/model is not restricted to `A`-`C`. See section 2.
- The inline edit flow (`window.prompt` with raw todo.txt) is unchanged.
- The `#tag-filter` text filter in `#filters` is unchanged (separate concern:
  it filters the list, it does not build a task). `renderMeta` (task-list
  display of `+`/`@`) is unchanged.
- No build step / framework — stays vanilla embedded HTML/CSS/JS.

## 2. "Priority A-C, remove rest" is a UI-only restriction

`initPriorityOptions()` currently generates `A`..`Z`. It will generate only the
placeholder plus `A`, `B`, `C`.

The change is deliberately confined to the add picker. The `Task` parser
(`take_priority`) still accepts `(A)`..`(Z)` and `Display` still round-trips
them, because:

- todo.txt files written by other tools (the format is interoperable by design)
  may legitimately contain `(D)`..`(Z)`. Narrowing the parser would corrupt or
  reject those lines on load and break the round-trip guarantee the tests pin.
- A task whose priority is outside `A`-`C` (loaded from file, or set via the raw
  edit prompt / `PUT /api/raw`) still displays its `(X)` badge unchanged. We are
  removing those options from the *picker*, not removing the *capability* from
  the data model.

This boundary is documented here so a reviewer does not read "the parser still
allows D" as an incomplete implementation.

## 3. Custom dropdown: why not a native `<select>`

The requirement is a "dropdown menu" with an Add button "at the bottom of the
dropdown menu". A native `<select>`'s option list cannot host a button, so a
small custom widget is required. Each dropdown is:

```
<div class="dropdown" data-kind="project">
  <span class="field-label">Projects</span>
  <button type="button" class="dropdown-toggle"
          aria-haspopup="true" aria-expanded="false">Select projects</button>
  <div class="dropdown-panel" hidden>
    <div class="dropdown-options"><!-- checkbox rows injected by JS --></div>
    <button type="button" class="dropdown-add">+ Add project</button>
  </div>
</div>
```

- Multi-select is preserved with one checkbox per known label (a task may carry
  several projects/contexts, as today's `<select multiple>` allowed).
- The toggle's text reflects the current selection (e.g. `Projects (2)` when two
  are checked, `Select projects` when none), so state is visible while collapsed.
- Open/close: clicking the toggle toggles the panel and `aria-expanded`. A
  single document-level click listener closes any open panel when the click
  lands outside a `.dropdown`; `Escape` also closes. Opening one dropdown closes
  the other so only one panel is open at a time.
- A single `setupDropdown(kind, root)` factory wires both dropdowns (information
  hiding; no copy-paste between projects and contexts). It owns rendering the
  option rows and the toggle label, and exposes a `render()` used after loads
  and after the modal adds a label.

## 4. Label sourcing and the staging tradeoff (key decision)

Because labels live only on tasks, the dropdowns' known labels come from the
same place the old combined select used: one unfiltered `GET /api/tasks`, whose
responses already carry `projects` and `contexts` arrays. No client-side parsing.

The Add modal creates a label that is not yet attached to any task. There is no
place to persist a free-floating label without inventing backend storage (out of
scope, section 1). The resolution:

- **Stage new labels client-side.** Each kind keeps a `staged` set. The set of
  options shown is `known = server-derived ∪ staged`. A newly added label is put
  in `staged` and marked selected immediately, so it is usable for the very next
  add.
- A staged label becomes real and persistent the moment a task using it is
  created: `composeText` emits `+name`/`@name`, the file gains the token, and the
  subsequent `loadLabels()` reports it from the server like any other.
- A staged label that is never attached to a task disappears on reload. This is
  consistent with todo.txt's model (a project/context with no task does not
  exist) and is accepted and documented, not a bug. The alternative — writing a
  contentless placeholder task just to anchor the label — would pollute the
  file and is explicitly rejected.

This keeps the whole feature frontend-only and preserves "todo.txt is the
canonical store."

## 5. Client state and the per-kind descriptor

A single descriptor states everything that differs between the two kinds, so the
prefix and the user-facing words live in exactly one place (DRY; avoids the
prefix/label drifting across HTML and JS):

```
const KINDS = {
  project: { prefix: "+", singular: "project", plural: "Projects" },
  context: { prefix: "@", singular: "context", plural: "Contexts" },
};

const labels = {
  project: { known: new Set(), selected: new Set(), staged: new Set() },
  context: { known: new Set(), selected: new Set(), staged: new Set() },
};
```

Labels are stored as bare names (no `+`/`@`); the prefix is added only at
compose time, since the kind is now explicit per dropdown. `setupDropdown`, the
toggle text, the panel's add-button text, and the modal title all read their
words from `KINDS[kind]`; `composeText` reads the prefix from it. `loadLabels()`
rebuilds each `known` as `server ∪ staged`, sorts the rendered rows for a stable
order, and drops any `selected` entry no longer in `known` (mirroring the old
"preserve still-valid selection" behavior).

## 6. Composition (one pure function, one source of truth)

```
function composeText() {
  const parts = [];
  if (els.priority.value) parts.push(`(${els.priority.value})`);
  const desc = els.input.value.trim();
  if (desc) parts.push(desc);
  for (const kind of Object.keys(KINDS))
    for (const name of labels[kind].selected) parts.push(`${KINDS[kind].prefix}${name}`);
  if (els.due.value) parts.push(`due:${els.due.value}`);
  return parts.join(" ").trim();
}
```

Iterating `KINDS` (rather than one hardcoded line per kind) means the prefix
mapping is read from the descriptor, so projects emit before contexts with no
duplicated prefix literals.

Priority leads (todo.txt requires `(A)` as the first token); the remaining order
is cosmetic because the parser is order-independent for the description
remainder. `composeText` stays a pure, side-effect-free read of control state so
it is trivially testable if a JS harness is ever added.

## 7. The Add modal (native `<dialog>`)

One shared dialog, parameterized by kind, rather than two near-identical ones:

```
<dialog id="label-modal">
  <form id="label-form">
    <h2 id="label-modal-title">Add project</h2>
    <label>Name <input id="label-input" type="text" autocomplete="off" /></label>
    <p id="label-error" class="modal-error" hidden></p>
    <div class="modal-actions">
      <button type="button" id="label-cancel">Cancel</button>
      <button type="submit">Add</button>
    </div>
  </form>
</dialog>
```

- `openLabelModal(kind)` sets the title, clears the input and error, calls
  `dialog.showModal()` (native focus trap + backdrop + `Escape` to dismiss), and
  focuses the input.
- On submit: take `input.value`, trim it, strip an optional leading `+`/`@` the
  user may have typed. Validation:
  - empty after trim -> error "Name is required.", keep open.
  - contains whitespace -> error "No spaces allowed in a project/context.",
    keep open. (todo.txt projects/contexts are single whitespace-free tokens.)
  - otherwise valid. If already in `known`, just select it (no error); else add
    to `staged` and `known`. Mark `selected`, re-render that dropdown, close.
- Cancel button and `Escape` close with no change. The form's submit is handled
  in JS (and `preventDefault`ed) so an invalid value does not close the dialog.

## 8. Submission, reset, errors

- The submit guard still requires a non-empty description; a line of only a
  priority/tags/due with no description is a client no-op (matches today).
- On success: clear description, reset priority to placeholder, clear due, clear
  both `selected` sets, re-render both dropdown toggles, then `refresh()` and
  `loadLabels()`. `staged` is left intact so just-added labels stay pickable.
- Errors continue through the existing `showError` path; `loadLabels` failure is
  non-fatal (form still works, dropdowns just show no suggestions), surfaced via
  `showError` without blocking add — same contract as today's `loadTagOptions`.

## 9. Files touched

- `web/index.html` — restructure `.add-fields`: keep priority + due, replace the
  combined tags select with the two `.dropdown` widgets and add the shared
  `<dialog>`.
- `web/app.js` — `initPriorityOptions` to `A`-`C`; `labels` state;
  `setupDropdown(kind, root)`; modal open/validate/confirm; `loadLabels`
  (replacing `loadTagOptions`); `composeText`; reset and submit wiring; document
  click/`Escape` to close panels.
- `web/style.css` — `.dropdown`, `.dropdown-toggle`, `.dropdown-panel`,
  `.dropdown-options`, `.dropdown-add`, and `dialog#label-modal` / `::backdrop`
  / `.modal-actions` / `.modal-error`. Reuse existing color variables; no new
  tokens.
- `README.md` — update the WebUI paragraph: priorities `A`-`C`, separate
  project/context dropdowns with add-via-modal, and the staged-until-used note.
- `implemented/project-context-dropdowns.md` — written after implementation.

No Rust source, no API surface, no `Cargo.toml`/store/task change.

## 10. Verification

Frontend has no automated test harness (the suite is Rust API/unit tests, none
of which this touches), so feature verification is manual: `cargo run`, open
`http://127.0.0.1:8080`, then confirm:

- Priority select offers only the placeholder plus `A`, `B`, `C`.
- Projects and Contexts are two separate dropdowns; each lists existing labels
  as checkboxes; multiple can be checked; the toggle text shows the count.
- Opening one dropdown closes the other; outside-click and `Escape` close panels.
- "+ Add project" / "+ Add context" opens the modal; a valid name stages,
  selects, and shows it checked, and it composes into the task as `+name` /
  `@name`; empty or spaced input shows an inline modal error and keeps the modal
  open; Cancel/`Escape` close without adding.
- After the add, the new label is reported by the server and stays pickable; all
  controls reset.
- A pre-existing `(D)`..`(Z)` task (added via the raw edit prompt) still loads
  and displays its priority badge — confirming the parser was not narrowed.

`cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` are still run
to confirm the unchanged backend is green; the web assets are embedded via
`include_str!`, so `cargo build` revalidates they are present and compiled in.

## 11. Risks

- A custom dropdown replaces a native control: more markup/JS, and it must handle
  open/close, outside-click, `Escape`, and focus. Mitigated by one shared
  `setupDropdown` factory and a native `<dialog>` for modal semantics (focus
  trap, backdrop, `Escape`) instead of hand-rolling them.
- Staged-but-unused labels vanish on reload (section 4) — accepted, documented,
  and consistent with todo.txt; a persistent label store is an explicit
  non-goal.
- Priority `A`-`C` is UI-only (section 2); files with `(D)`-`(Z)` still parse and
  display — documented to avoid a false "incomplete" reading.
- `<dialog>` and the custom popover assume a modern browser, consistent with the
  existing use of flexbox, `replaceChildren`, and `URLSearchParams`.

## 12. brooks-review outcome

`reviews/project-context-dropdowns.md` scored the plan 98/100 (over the 95 gate),
with two Suggestions:

1. Knowledge Duplication — per-kind metadata could scatter across HTML/JS.
   Adopted: section 5 now defines a single `KINDS` descriptor that
   `setupDropdown`, the toggle/add-button text, the modal title, and
   `composeText` all read from. Re-scored after this change: 99/100.
2. Coverage Illusion — new client logic ships without an automated test.
   Accepted as constraint-bound: the repo has no JS test harness by the
   deliberate no-build-step decision, and `composeText`/validation are kept pure
   so they are ready to test if a harness is ever added. No decay risk
   introduced; scope stays `web/*` plus docs.
