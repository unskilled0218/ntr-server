# Plan: native project/context selects (branch `native-label-selects`)

Make the add form's Projects and Contexts controls use the same UI as Priority:
a native single-select `<select>` wrapped in a `<label>`. This replaces the
custom multi-select popover widgets (toggle button + checkbox panel + an "Add"
button) and removes the add-label modal entirely. After this change all four
structured controls (Priority, Due, Projects, Contexts) are plain native form
elements with identical styling and behavior.

## 1. Goal and scope

Requirement (from `agents/ADD_NEW_FEATURE.md`):

> Make projects and contexts ui same as Priority

Priority is a native `<select>` inside a `<label>` (single-select, with a blank
placeholder option). "Same as Priority" is therefore read, after clarifying the
two trade-offs a native `<select>` forces, as:

- **Single-select.** Each control is one native `<select>`: the form sets at
  most one `+project` and one `@context` per task (Priority sets at most one
  priority). This is a picker-only restriction, mirroring the existing "A-C only
  in the picker" decision — see section 2.
- **No Add modal.** A native `<select>` cannot host an "Add" button, and the
  modal/staging machinery existed only to feed that button. It is removed. New
  labels enter the dropdowns the same way they always have for suggestions: by
  appearing on a task (typed into the description, or via the raw edit prompt /
  `PUT /api/raw`), after which `loadLabels()` offers them — see section 4.

In scope (WebUI only):

- Replace the two `.dropdown` widgets with two `<label>`-wrapped `<select>`
  controls (`#add-project`, `#add-context`), styled exactly like `#add-priority`
  via the existing `.add-fields select` rule.
- Remove the `<dialog id="label-modal">` and all of its JS/CSS.
- Populate each select from labels already present in the file (one unfiltered
  `GET /api/tasks`, as today), with a blank placeholder first then the sorted
  label names.
- Compose the todo.txt line from priority + description + selected project
  (`+name`) + selected context (`@name`) + due date, through the existing
  `POST /api/tasks`.

Out of scope (explicit non-goals):

- No server/API/Rust changes. The contract already accepts a full todo.txt line
  via `{ "text": "..." }`; this feature only changes how the WebUI builds it, as
  the prior `add-task-selects` and `project-context-dropdowns` features were
  scoped. Keeping one task-construction path avoids a second divergent one.
- No backend label registry. todo.txt has no project/context that exists apart
  from a task; labels are derived from task descriptions. Nothing to persist.
- The parser/model is not restricted: a task may still carry several
  `+projects`/`@contexts` (from file load, the raw edit prompt, or
  `PUT /api/raw`), and `renderMeta` still displays all of them. Only the *picker*
  is single-select. See section 2.
- The inline edit flow (`window.prompt` with raw todo.txt) is unchanged.
- The `#tag-filter` text filter and the `#filters` buttons are unchanged.
- No build step / framework — stays vanilla embedded HTML/CSS/JS.

## 2. Single-select is a picker-only restriction (mirrors A-C)

This feature narrows what the *form* can set, never what the *data model* can
hold — exactly the boundary section 2 of `plans/project-context-dropdowns.md`
drew for priority `A`-`C`:

- The `Task` parser/`Display` (Rust) are untouched. A line with
  `+home +work @phone @errands` still parses, round-trips, and lists with every
  label shown by `renderMeta` (which already maps over `task.projects` /
  `task.contexts` arrays and is not changed).
- A task created/edited as raw todo.txt may carry any number of labels; the
  single-select form is just the convenient path for the common one-label case.
- Documenting this here prevents a reviewer reading "the form only sets one
  project" as data loss. No existing task is rewritten or truncated; editing an
  existing task still goes through the raw `window.prompt`, which preserves all
  its labels.

## 3. Why native `<select>` now (and why the custom widget goes)

`plans/project-context-dropdowns.md` chose a custom widget specifically to host
an Add button "at the bottom of the dropdown menu" and to allow multi-select.
With those two requirements withdrawn (section 1), the custom widget's entire
reason to exist is gone, so it is removed rather than restyled:

- A native `<select>` is the smallest control that satisfies "same as Priority":
  it reuses Priority's markup shape (`<label>` + `<select>`), its CSS rule
  (`.add-fields select`), its keyboard/focus/open-close behavior (browser-native),
  and its placeholder pattern (a value-`""` first option).
- Deleting `setupDropdown`, `closeAllDropdowns`, `initDropdowns`, the
  document-level click/`Escape` panel listeners, the `dropdowns` registry, the
  modal (`openLabelModal`, `parseLabelInput`, `confirmLabel`, `modalKind`), and
  their CSS removes ~120 lines of hand-rolled popover/modal/focus logic. Net
  result is less code with fewer states to get wrong — the change is a
  simplification, not a second parallel mechanism.

## 4. Label sourcing (unchanged source, simpler consumer)

The source of options is identical to today: one unfiltered `GET /api/tasks`,
whose responses already carry `projects` and `contexts` arrays — no client-side
parsing of raw lines. What changes is only how the options are rendered:

- `loadLabels()` collects the server label sets exactly as now, then calls a new
  `populateLabelSelect(kind, names)` instead of re-rendering checkbox rows.
- `populateLabelSelect` rebuilds the `<option>` list: a blank placeholder
  (`new Option(KINDS[kind].plural, "")`) followed by the sorted names. It
  preserves the current pick across reloads: it reads `select.value` before
  rebuilding and restores it only if `names` still contains it, otherwise falls
  back to the placeholder. This mirrors the old "drop selections no longer in
  `known`" behavior with native-select semantics.
- The `staged` set is deleted: with no Add button there is nothing to stage. A
  label that exists only because the user typed it into the description becomes a
  suggestion as soon as that task is saved and `loadLabels()` re-runs — the
  normal flow already calls `loadLabels()` after every add and after an edit.
- As before, a select is empty (just its placeholder) until the file contains at
  least one label of that kind. `loadLabels` failure stays non-fatal: it surfaces
  via `showError` and leaves the form usable with empty selects.

## 5. Client state simplification

The per-kind `labels` registry (`known` / `selected` / `staged` sets) is removed.
A native `<select>` already holds both its option list and its current value, so
no shadow state is needed. The `KINDS` descriptor stays, trimmed to what is still
differentiated — the prefix (compose) and the plural (placeholder text):

```js
const KINDS = {
  project: { prefix: "+", plural: "Projects" },
  context: { prefix: "@", plural: "Contexts" },
};
```

`singular` is dropped (it fed only the removed modal title). The two selects are
referenced by kind so the compose/reset/populate loops stay generic:

```js
els.project = document.getElementById("add-project");
els.context = document.getElementById("add-context");
// iterated as els[kind] for kind of Object.keys(KINDS)
```

Keying `els` by the kind name (`project`/`context`) lets `composeText`,
`resetAddForm`, and `loadLabels` iterate `Object.keys(KINDS)` and read
`els[kind]` with no per-kind branch — the prefix/plural live in one place
(DRY, as the prior plan established).

## 6. Composition (one pure function, one source of truth)

```js
function composeText() {
  const parts = [];
  if (els.priority.value) parts.push(`(${els.priority.value})`);
  const desc = els.input.value.trim();
  if (desc) parts.push(desc);
  for (const kind of Object.keys(KINDS))
    if (els[kind].value) parts.push(`${KINDS[kind].prefix}${els[kind].value}`);
  if (els.due.value) parts.push(`due:${els.due.value}`);
  return parts.join(" ").trim();
}
```

Same shape as today, with the inner `for…of selected` collapsed to a single
`if (els[kind].value)` because each kind now contributes at most one label.
Priority still leads (todo.txt requires `(A)` first); the rest of the order is
cosmetic. `composeText` stays pure (reads control values, returns a string) so it
is trivially testable if a JS harness is ever added.

## 7. Reset, submit, errors

- `resetAddForm()` clears the description, resets priority and both label selects
  to `""`, and clears due — all plain `value = ""` assignments now (no dropdown
  re-render, no `selected` sets to clear).
- The submit guard is unchanged: a non-empty description is required; a line of
  only priority/labels/due is a client no-op.
- On success: reset, then `refresh()` and `loadLabels()`. `loadLabels` repopulates
  the just-cleared selects with the current file's labels (the new one included).
- Errors continue through the existing `showError` path; `loadLabels` failure is
  non-fatal — same contract as today.

## 8. Files touched

- `web/index.html` — in `.add-fields`, replace the two `.dropdown` blocks with
  two `<label>`-wrapped `<select id="add-project">` / `<select id="add-context">`
  controls modeled on the Priority `<label>`. Delete the `<dialog id="label-modal">`
  block.
- `web/app.js` — trim `KINDS` (drop `singular`); remove the `labels` registry,
  `dropdowns`, `setupDropdown`, `closeAllDropdowns`, `initDropdowns`, the
  document click/`Escape` panel listeners, and the modal functions/state/wiring;
  add `els.project`/`els.context` and `populateLabelSelect`; simplify
  `loadLabels`, `composeText`, `resetAddForm`; drop the modal `els` and
  `initDropdowns()` from bootstrap.
- `web/style.css` — delete the `.dropdown*` block and the `dialog#label-modal` /
  `::backdrop` / `.modal-actions` / `.modal-error` / `#label-form` / `#label-input`
  rules. The new selects need no new CSS (reuse `.add-fields select`).
- `README.md` — update the WebUI paragraph: native single-select project/context
  dropdowns populated from existing labels, no Add modal, single-label-in-picker
  is a picker-only restriction (like A-C) while a task may still hold many labels.
- `implemented/native-label-selects.md` — written after implementation.

No Rust source, no API surface, no `Cargo.toml`/store/task change. Web assets are
embedded via `include_str!` as opaque strings; element-ID changes do not affect
the Rust build, and no test inspects HTML.

## 9. Verification

Frontend has no automated test harness (the suite is Rust API/unit tests, none of
which this touches), so feature verification is manual: `cargo run`, open
`http://127.0.0.1:8080`, then confirm:

- Projects and Contexts render as native dropdowns visually identical to
  Priority (same `<label>` + `<select>` styling), each with a blank placeholder.
- Each select lists the file's existing labels, sorted; picking one and adding a
  task composes `+name` / `@name` into the saved line; the new label then appears
  as an option after the post-add `loadLabels()`.
- Selecting the blank placeholder contributes no label; a task can be added with
  no project/context.
- The controls reset after a successful add; the selected value survives a reload
  only while the label still exists.
- A task carrying multiple `+projects`/`@contexts` (added via the raw edit prompt
  or `PUT /api/raw`) still loads and displays all of them in the meta line —
  confirming single-select is picker-only.
- No add-label modal, no checkbox popovers, and no leftover dropdown controls
  remain in the DOM.

`cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` are still run
to confirm the unchanged backend is green; `include_str!` makes `cargo build`
revalidate the web assets are present and compiled in.

## 10. Risks

- Capability narrowing: the form sets one project + one context where it set many
  before. Mitigated/bounded — single-select is picker-only (section 2); the data
  model, the raw edit path, and `renderMeta` still support and show multiple
  labels, so no existing task loses data.
- Removing the Add modal removes the ability to pre-create a label before any task
  uses it. Accepted per the clarified requirement: labels become options once a
  task carries them, consistent with todo.txt having no free-floating label.
- Net deletion of popover/modal/focus code lowers risk overall (fewer states),
  but the manual checklist in section 9 must confirm no orphaned element IDs or
  event handlers remain after the removal.
- Native `<select>` / placeholder-option assume a modern browser, consistent with
  the existing use of flexbox, `replaceChildren`, `URLSearchParams`, and the
  native `<select>` already used for Priority.

## 11. brooks-review outcome

`reviews/native-label-selects.md` scored the plan 98/100 (over the 95 gate), with
two Suggestions, both accepted as constraint-bound — no plan change:

1. Knowledge Duplication — the field caption ("Projects"/"Contexts") appears in
   both the HTML `<label>` and the JS placeholder option (`KINDS[kind].plural`).
   Accepted: this is precisely how the existing Priority control works, and the
   feature's goal is to be "same as Priority"; de-duplicating would break that
   consistency. It is one short display string per kind, not logic.
2. Coverage Illusion — new client logic ships without an automated test. Accepted
   as constraint-bound: the repo has no JS harness by the no-build-step decision;
   `composeText` is kept pure and ready to test, and the change is a net deletion
   of popover/modal code, reducing behavioral surface. Same posture as the prior
   two features.

No iteration was required; the plan clears the gate at 98/100 as written.
