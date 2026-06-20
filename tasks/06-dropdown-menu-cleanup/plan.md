# Plan: dropdown menu cleanup — add entry on top, "None" empty option (branch `dropdown-menu-cleanup`)

Tidy the add form's three option dropdowns (Priority, Projects, Contexts). The
"add" entry in the project/context dropdowns moves from the bottom to the top of
the option list, and the empty/no-selection placeholder in all three dropdowns
stops repeating the field caption — it becomes a neutral `None` label instead.

## 1. Goal and scope

Requirements (from `agents/ADD_NEW_FEATURE.md`):

> - Move add button to top of dropdown menu
> - Remove Priority option from Priority dropdown menu
> - Remove Projects option from Projects dropdown menu
> - Remove Contexts option from Contexts dropdown menu

Interpretation (confirmed with the user before planning):

- The three "Remove X option" items target the **placeholder option** that today
  sits first in each `<select>` and whose visible text is the field caption
  (`Priority` / `Projects` / `Contexts`). That option is *also* the empty
  ("no selection", `value=""`) state of the control, and that empty state must be
  kept: most tasks carry no priority, and many carry no project/context, so the
  control needs a way to express "none". The redundant part is only the text — the
  field name is already shown by the `<label>` wrapping each select in
  `index.html`. So the fix is: keep one option with `value=""`, but relabel it
  `None` rather than deleting it. The user explicitly chose the `None` label over a
  blank line or removing the empty option entirely (which would force a value).
- "Move add button to top" = the existing sentinel add `<option>`
  (`+ Add project...` / `+ Add context...`) is appended **first** in the option
  list instead of last. Priority has no add entry, so only the two label selects
  reorder.

In scope (WebUI only):

- `web/app.js` — relabel the Priority placeholder to `None`; in
  `populateLabelSelect`, emit the add entry first, then the `None` empty option,
  then the sorted labels; drop the now-unused `plural` field from `KINDS`.
- `web/index.html` — no markup change (the `<label>` captions stay; the dropdowns
  are populated by JS).
- `web/style.css` — no change (option order/text needs no styling).
- `README.md` — update the WebUI paragraph wording (placeholder is now `None`; the
  add entry is at the top, not the end).

Out of scope (explicit non-goals):

- No server/API/Rust change. This only adjusts option text and order in the
  WebUI; the composed todo.txt line and `POST /api/tasks` contract are untouched.
- No change to the empty-state *semantics*: `value=""` still means "no
  selection", `composeText` still omits an empty field, `resetAddForm` still
  resets to `""`. Only the visible label of that option changes.
- No change to the sentinel mechanism: the add entry is still recognised by its
  `data-add` flag, not by value or by position. Moving it to the top changes only
  where it renders, not how it is detected. (See section 3.)
- No change to staging, validation, the modal, the priority A–C range, the due
  control, inline edit, the tag filter, or `renderMeta`.
- No build step / framework — stays vanilla embedded HTML/CSS/JS.

## 2. The "None" empty option (all three selects)

Today each select's first option is the placeholder, with `value=""` and text set
to the field caption:

- Priority: `els.priority.add(new Option("Priority", ""))` in
  `initPriorityOptions` (`web/app.js:205`).
- Project/Context: `select.add(new Option(KINDS[kind].plural, ""))` in
  `populateLabelSelect` (`web/app.js:224`), where `KINDS.project.plural` is
  `"Projects"` and `KINDS.context.plural` is `"Contexts"`.

Both placeholders should read the same word, and that word is a single product
decision ("what the no-selection option is called"). To avoid expressing it as a
bare `"None"` literal in two call sites — which would drift if the wording ever
changes to `—` / `Any` / a localized string — define it once at module scope
alongside `ADD_VALUE`:

```
// Visible text of the empty / no-selection option shared by all three add-form
// dropdowns. value="" is the real no-selection state; this is only its label.
const EMPTY_LABEL = "None";
```

Change both call sites to `new Option(EMPTY_LABEL, "")`. The `value=""` is
unchanged, so every code path that reads or sets the value is unaffected:

- `composeText` only emits a field when `els[kind].value` / `els.priority.value`
  is truthy, so `None` (value `""`) still composes to nothing.
- `resetAddForm` sets the value back to `""`, which still selects the `None`
  option.
- `populateLabelSelect`'s selection-preservation line
  (`select.value = names.has(previous) ? previous : ""`) still falls back to the
  empty option, now shown as `None`.

`KINDS[kind].plural` was the *only* reader of the `plural` field (verified by
grep: the sole other occurrences of "Projects"/"Contexts" are the static
`<label>` captions in `index.html`). Once the placeholder text is hard-coded to
`None`, `plural` is dead. Remove it from both `KINDS` entries so the descriptor
does not carry an unused, now-misleading field:

```
const KINDS = {
  project: { prefix: "+", singular: "project" },
  context: { prefix: "@", singular: "context" },
};
```

`singular` is still used (add-entry text `+ Add ${singular}...` and modal title
`Add ${singular}`); `prefix` is still used (`composeText`). Only `plural` goes.

## 3. Add entry moves to the top (project/context selects)

`populateLabelSelect` currently builds the option list as: placeholder, sorted
labels, then the sentinel add entry last. Reorder so the add entry is first:

```
function populateLabelSelect(kind, names) {
  const select = els[kind];
  const previous = select.value;
  select.replaceChildren();
  const addOption = new Option(`+ Add ${KINDS[kind].singular}...`, ADD_VALUE);
  addOption.dataset.add = "1";
  select.add(addOption);                       // pinned first
  select.add(new Option(EMPTY_LABEL, ""));     // empty / no-selection
  for (const name of [...names].sort()) select.add(new Option(name, name));
  select.value = names.has(previous) ? previous : "";
  priorLabel[kind] = select.value;
}
```

Why this stays correct:

- **The displayed selection is still `None`, not the add entry.** A native
  `<select>` defaults to its first option *only when its value is never set*. This
  function always sets `select.value` explicitly on the last two lines, to a real
  label or `""`. `""` matches the `None` option uniquely (the add entry's value is
  `ADD_VALUE = "+add"`, which `+` makes an impossible real-label value per the
  validator), so the control shows `None` after every populate, exactly as it
  showed the old placeholder before. The first-option-as-default rule never fires.
- **Detection is unchanged.** `onLabelSelectChange` branches on
  `select.selectedOptions[0]?.dataset.add`, i.e. the `data-add` flag, never the
  option's index or value. The add entry being first instead of last does not
  touch that path; choosing it still restores `priorLabel[kind]` and opens the
  modal.
- **`confirmLabel` is unaffected.** It rebuilds the option set from the existing
  options minus `""`/`ADD_VALUE`, re-runs `populateLabelSelect` (which now places
  the add entry first), then selects the new name. Order of construction there is
  irrelevant; it relies on `populateLabelSelect` for layout.

Priority's `initPriorityOptions` is not reordered — it has no add entry — only its
placeholder text changes (section 2).

## 4. Files touched

- `web/app.js`
  - Add the `EMPTY_LABEL = "None"` constant at module scope (section 2).
  - `KINDS`: drop the `plural` field from both kinds (section 2).
  - `initPriorityOptions`: `"Priority"` placeholder text -> `EMPTY_LABEL`.
  - `populateLabelSelect`: emit add entry first, then the `EMPTY_LABEL` option,
    then sorted labels; placeholder text `KINDS[kind].plural` -> `EMPTY_LABEL`
    (section 3).
  - Update the two affected explanatory comments (the `populateLabelSelect`
    header that says the placeholder text is "the field caption" and that the add
    entry is "pinned last") so the prose matches the new behavior.
- `web/index.html` — no change.
- `web/style.css` — no change.
- `README.md` — in the WebUI section: the dropdowns now *begin* with the
  `+ Add ...` entry (not "end with" it), and the empty option reads `None`. Adjust
  the "each shows only its placeholder and 'add' entry" sentence accordingly.
- `implemented/dropdown-menu-cleanup.md` — written after implementation.

No Rust source, no API surface, no `Cargo.toml`/store/task change.

## 5. Verification

Frontend has no automated test harness (the suite is Rust API/unit tests, none of
which this touches), so feature verification is manual: `cargo run`, open
`http://127.0.0.1:8080`, then confirm:

- Priority dropdown lists `None`, `A`, `B`, `C` (no `Priority` text option) and
  defaults to `None`; leaving it on `None` composes a task with no `(X)` priority.
- Projects and Contexts dropdowns show the `+ Add project...` / `+ Add
  context...` entry as the **first** item, then `None`, then any existing labels;
  each defaults to `None`.
- Choosing `None` composes a task with no `+project` / `@context`; picking a real
  label still composes `+name` / `@name`.
- Choosing the add entry (now at the top) still opens the modal and the select
  does not stay stuck on the add entry — it returns to its prior value.
- A staged label added via the modal is still selected immediately and stays
  pickable after the task is saved (no regression to that flow).
- `resetAddForm` after a successful add returns all three selects to `None`.

`cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` are still run
to confirm the unchanged backend is green; the web assets are embedded via
`include_str!`, so `cargo build` revalidates they compile in.

## 6. Risks

- Reordering the add entry to the top risks it being auto-selected as the first
  option. Mitigated and reasoned in section 3: `populateLabelSelect` always sets
  `select.value` explicitly afterwards, so the first-option default never applies
  and the control always shows `None`.
- Relabeling the empty option could be mistaken for removing the empty *value*.
  Section 2 keeps `value=""` and every value-reading/writing path intact; only the
  visible text changes, so optionality is preserved (the user's explicit choice
  over deleting the option).
- Removing the `plural` field is safe only if it has no other reader; verified by
  grep (its sole use was the placeholder text being replaced). `singular` and
  `prefix` remain in use.
- No JS test harness exists (deliberate no-build-step decision, consistent with
  prior WebUI features), so this is constraint-bound to manual verification.

## 7. brooks-review outcome

`reviews/dropdown-menu-cleanup.md` first scored the plan 99/100 with one
Suggestion: the empty-option text `"None"` appeared as a bare literal in two call
sites (`initPriorityOptions` and `populateLabelSelect`) — the same product
decision expressed twice (R3 Knowledge Duplication). Adopted: section 2 now
defines `EMPTY_LABEL = "None"` once at module scope and both call sites reference
it, so the wording has a single source. Re-review scored 100/100, clearing the 95
gate. The only remaining note is the constraint-bound absence of a JS test harness
(no-build-step decision, consistent with every prior WebUI feature), which is not
a deduction.
