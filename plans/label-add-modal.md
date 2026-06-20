# Plan: add-new-label option + modal in the project/context dropdowns (branch `label-add-modal`)

Give the add form's `Projects` and `Contexts` dropdowns a way to create a brand
new label from inside the dropdown: an "add" entry pinned at the bottom of each
dropdown's option list that, when chosen, opens a small modal with a single text
input. Typing a name there stages the label, selects it, and makes it usable for
the very next task add.

## 1. Goal and scope

Requirements (from `agents/ADD_NEW_FEATURE.md`):

> - Make add button to bottom of projects/contexts dropdown menu
> - if user click add button modal input text

In scope (WebUI only):

- A persistent "add" entry at the bottom of each existing project/context
  dropdown's option list (`+ Add project...` / `+ Add context...`).
- Activating that entry opens a modal (native `<dialog>`) with one text input.
- A valid name typed in the modal is created as a new label, selected in the
  dropdown that opened the modal, and immediately usable when composing the next
  task. Composition still goes through the existing `POST /api/tasks`.

Out of scope (explicit non-goals):

- No server/API/Rust changes. The contract already accepts a full todo.txt line
  via `{ "text": "..." }`; this feature only changes how the WebUI builds it,
  exactly as the `add-task-selects` and `project-context-dropdowns` features
  before it were scoped. One task-construction path is kept.
- No backend "label registry". todo.txt has no concept of a project/context that
  exists independently of a task (labels are derived from task descriptions), so
  there is nothing to persist a free-floating label into. See section 4.
- The inline edit flow (`window.prompt` with raw todo.txt) is unchanged.
- The `#tag-filter` text filter and `renderMeta` (task-list display) are
  unchanged (separate concerns: they filter/display, they do not build a task).
- No change to the priority or due controls, and no change to the parser/model.
- No build step / framework — stays vanilla embedded HTML/CSS/JS.

## 2. Why an option entry, not a literal `<button>` (key decision)

The current UI uses native `<select>` elements for project and context (commit
`65ffb99`, "Match project/context UI to Priority"). That decision is preserved
here: a native `<select>`'s popup is rendered by the browser and **cannot host a
child `<button>` or any non-`<option>` markup**. The only thing that can sit "at
the bottom of the dropdown menu" of a native select is another `<option>`.

So the "add button" is implemented as a sentinel `<option>` appended after the
real labels:

```
<select id="add-project">
  <option value="">Projects</option>      <!-- placeholder -->
  <option value="alpha">alpha</option>      <!-- real labels, sorted -->
  <option value="beta">beta</option>
  <option value="+add" data-add="1">+ Add project...</option>  <!-- sentinel, pinned last -->
</select>
```

- The sentinel is identified out-of-band, by a `data-add="1"` attribute, **not**
  by its value string. The change handler branches on
  `select.selectedOptions[0].dataset.add`, so the sentinel's value never has to be
  reserved or guarded against collision. Its value is set to `ADD_VALUE = "+add"`
  only as defence-in-depth: `+` is forbidden in a real bare label by the validator
  (section 7), so even code paths that compared by value could not mistake a real
  label for the sentinel. (An earlier draft keyed the handler off an `__add__`
  value and claimed collision was impossible; that was wrong — `+__add__` is a
  legal todo.txt project, so once a task used it `populateLabelSelect` would render
  a real option whose value equalled the sentinel's. The `dataset` flag removes
  that namespace overlap entirely rather than reasoning around it.)
- Choosing it fires the select's `change` event; the handler opens the modal,
  then resets the select back to its previous value so the sentinel is never a
  "stuck" selection and never composes into a task.

This keeps the controls visually identical to the Priority select (the explicit
goal of the last commit) while still placing the add affordance at the bottom of
the dropdown, as the feature asks. The trade-off — a sentinel option instead of a
real button — is the direct consequence of the native-select decision and is
documented here so a reviewer does not read it as a hack.

Rejected alternative: reverting to a custom popover dropdown (as the abandoned
`project-context-dropdowns` widget did) would allow a real `<button>` but would
undo commit `65ffb99` and reintroduce the open/close/outside-click/focus
machinery that was deliberately removed. Out of scope.

## 3. Shared modal (native `<dialog>`)

One dialog serves both kinds, parameterized by kind, rather than two near-copies:

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

- `openLabelModal(kind)` records the kind, sets the title from `KINDS[kind]`,
  clears the input and error, calls `dialog.showModal()` (native focus trap +
  backdrop + `Escape` to dismiss), and focuses the input.
- Cancel and `Escape` close with no change. The select that opened the modal has
  already been reset to its prior value (section 2), so a dismissed modal leaves
  the form exactly as it was.
- Submit is handled in JS and `preventDefault`ed, so an invalid value shows an
  inline error and keeps the dialog open rather than closing it.

## 4. Label sourcing and the staging trade-off

Today `loadLabels()` derives the dropdown options from one unfiltered
`GET /api/tasks`, unioning every task's `projects`/`contexts`. That stays.

A name created in the modal is not yet attached to any task, and there is no
backend store to persist a free-floating label into (section 1). Resolution,
mirroring the documented decision in `plans/project-context-dropdowns.md`:

- **Stage new labels client-side.** Each kind keeps a `staged` set. The options
  rendered are `serverDerived ∪ staged`. A name added via the modal goes into
  `staged`, is rendered as an option, and is selected immediately so it is usable
  for the very next add.
- A staged label becomes real and persistent the moment a task using it is
  created: `composeText` emits `+name`/`@name`, the file gains the token, and the
  next `loadLabels()` reports it from the server like any other label. At that
  point it is in both `serverDerived` and `staged`; the union de-dupes it.
- A staged label never attached to a task disappears on reload. This is
  consistent with todo.txt (a project/context with no task does not exist) and is
  accepted and documented, not a bug. Writing a contentless placeholder task just
  to anchor a label is explicitly rejected.

This keeps the feature frontend-only and preserves "todo.txt is the canonical
store."

## 5. Per-kind descriptor (one source of truth)

The existing `KINDS` descriptor is extended so the words that differ between the
two kinds live in exactly one place (the add-entry text, the modal title):

```
const KINDS = {
  project: { prefix: "+", singular: "project", plural: "Projects" },
  context: { prefix: "@", singular: "context", plural: "Contexts" },
};
const ADD_VALUE = "__add__";
```

The sentinel option's label is `` `+ Add ${KINDS[kind].singular}...` ``; the
modal title is `` `Add ${KINDS[kind].singular}` ``. `populateLabelSelect` reads
`plural` for the placeholder (unchanged) and `singular` for the add entry.
`composeText` reads `prefix` (unchanged). No per-kind literal is duplicated
across HTML/JS. `ADD_VALUE` is the sentinel option's value, but the sentinel is
recognised by its `data-add` flag (section 2), so `ADD_VALUE` carries no
correctness weight.

## 6. Rendering, selection, staging (the changed JS)

- `populateLabelSelect(kind, names)` (existing) is extended: after appending the
  real labels (now `serverDerived ∪ staged`, sorted), it appends the sentinel
  add `<option>` last, carrying `value = ADD_VALUE` and `dataset.add = "1"`.
  Selection preservation is unchanged: keep `previous` if it is still a real
  label, else fall back to the placeholder; `previous` is never the sentinel
  because the change handler resets it before the modal opens.
- `loadLabels()` (existing) gains a `staged` union: for each kind it renders
  `server[kind] ∪ staged[kind]`. `staged` lives in module scope, one `Set` per
  kind, initialized empty.
- New `onLabelSelectChange(kind)`: if the selected option carries the `data-add`
  flag (`select.selectedOptions[0]?.dataset.add`), restore the select to its
  remembered prior value and call `openLabelModal(kind)`. Otherwise it is a normal
  selection (remember it as the new prior value). The prior value is tracked per
  kind so the reset after the sentinel is exact. Branching on the flag, not the
  value, is what makes a real label named like the sentinel impossible to confuse
  with it (section 2).
- `confirmLabel()` (modal submit): parse/validate the input (section 7). On a
  valid name: add to `staged[kind]`, re-run `populateLabelSelect(kind, ...)` so
  the option exists, set the select's value to the new name (selected), close the
  dialog. If the name already exists as an option, skip staging and just select
  it (no error) — picking an existing label from the "add" path is harmless.

## 7. Validation (one pure function)

`parseLabelInput(raw)` returns `{ name }` or `{ error }`, reusing the rules the
prior feature settled on so behavior does not drift:

- Trim. Strip all leading sigils: `replace(/^[+@]+/, "")`.
- Empty after trim -> error "Name is required."
- Contains whitespace -> error "No spaces allowed in a project or context."
- Still contains `+`, `@`, or `:` -> error "A project or context cannot contain
  + @ or :". (`:` is rejected because a `key:value` name would compose into a
  token the parser reads as a tag, not a label.)
- Otherwise `{ name }`.

Kept pure (no DOM, no side effects) so it is trivially testable if a JS harness is
ever added, and so `confirmLabel` is the only place that touches state.

## 8. Submission and reset

- The submit guard still requires a non-empty description; a line with only
  priority/labels/due and no description stays a client no-op (matches today).
- On a successful task add: `resetAddForm()` clears the description, priority,
  due, and resets both label selects to the placeholder (existing behavior; the
  sentinel and staged options remain in the lists so just-added labels stay
  pickable). Then `refresh()` and `loadLabels()`.
- Errors continue through the existing `showError` path; `loadLabels` failure is
  non-fatal — same contract as today.

## 9. Files touched

- `web/index.html` — add the shared `<dialog id="label-modal">` after the form.
  No change to the four `.add-fields` controls themselves.
- `web/app.js` — `ADD_VALUE` constant; `singular` in `KINDS`; `staged` sets;
  `populateLabelSelect` appends the sentinel; `loadLabels` unions `staged`;
  `onLabelSelectChange`, `openLabelModal`, `confirmLabel`, `parseLabelInput`;
  wire the two selects' `change` events, the modal form submit, and the cancel
  button.
- `web/style.css` — `dialog#label-modal`, `::backdrop`, `.modal-actions`,
  `.modal-error`. Reuse existing color variables; no new tokens.
- `README.md` — extend the WebUI paragraph: the project/context dropdowns now end
  with an add entry that opens a modal to create a new label, staged until a task
  uses it.
- `implemented/label-add-modal.md` — written after implementation.

No Rust source, no API surface, no `Cargo.toml`/store/task change.

## 10. Verification

Frontend has no automated test harness (the suite is Rust API/unit tests, none of
which this touches), so feature verification is manual: `cargo run`, open
`http://127.0.0.1:8080`, then confirm:

- Each of the Projects and Contexts dropdowns lists existing labels and ends with
  a `+ Add project...` / `+ Add context...` entry.
- Choosing the add entry opens the modal with the matching title; the select does
  not stay on the add entry.
- A valid name added in the modal appears selected in the dropdown that opened it
  and composes into the task as `+name` / `@name`.
- Empty, spaced, or `+ @ :`-containing input shows an inline modal error and keeps
  the modal open; Cancel and `Escape` close without adding and leave the select
  unchanged.
- After a task using a staged label is added, the label is reported by the server
  on the next load and stays pickable (no duplicate option).
- Priority and due controls are unaffected.

`cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` are still run
to confirm the unchanged backend is green; the web assets are embedded via
`include_str!`, so `cargo build` revalidates they are present and compiled in.

## 11. Risks

- Sentinel-option-as-button is a native-select constraint (section 2), not a
  preferred UX. Mitigated by resetting the select immediately so the sentinel is
  never a real selection, and by recognising the sentinel through an out-of-band
  `data-add` flag so it cannot collide with any todo.txt label name.
- Staged-but-unused labels vanish on reload (section 4) — accepted, documented,
  consistent with todo.txt; a persistent label store is an explicit non-goal.
- `<dialog>` assumes a modern browser, consistent with the existing use of
  flexbox, `replaceChildren`, and `URLSearchParams`.

## 12. brooks-review outcome

`reviews/label-add-modal.md` first scored the plan 94/100 (below the 95 gate),
with one Warning and one Suggestion:

1. Cognitive Overload — the sentinel value `__add__` shared the label namespace,
   so a real project/context named `__add__` would be misread as the add entry,
   and the plan's "can never collide" prose was incorrect. Adopted: section 2 now
   recognises the sentinel through an out-of-band `data-add` flag (the handler
   branches on `dataset.add`, not the value), with the value also set to a
   validator-forbidden `+add` as defence-in-depth. The false justification was
   replaced with the explicit invariant. This removes the namespace overlap.
2. Coverage Illusion — new client logic ships without an automated test. Accepted
   as constraint-bound: the repo has no JS test harness by the deliberate
   no-build-step decision (matching `add-task-selects` and
   `project-context-dropdowns`); `parseLabelInput` is kept pure and ready to test.

Re-review after the section-2 fix scored 99/100 (over the gate): the Warning is
resolved and only the constraint-bound Coverage Illusion suggestion remains.
