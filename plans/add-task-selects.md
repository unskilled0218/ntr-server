# Plan: structured add controls (branch `add-task-selects`)

Add select-style controls for priority, due date, and tags under the task
input box in the WebUI, so users can set these fields by picking options
instead of typing todo.txt syntax by hand.

## 1. Goal and scope

Requirement (from `agents/ADD_NEW_FEATURE.md`):

> Set priority, date, tags with select options not text — under input text.

In scope:

- A control row rendered directly under the existing `#add-input` text field:
  - Priority: a `<select>` (none, `A`..`Z`).
  - Due date: a native `<input type="date">` (a picker, not free text).
  - Projects & contexts: a `<select multiple>` whose options are the
    `+project` and `@context` labels already present in the file. (Labelled
    "Projects & contexts" rather than "Tags" so it matches todo.txt
    vocabulary — projects, contexts, and key:value tags are distinct concepts.)
- Compose the todo.txt line from these controls plus the free-text
  description, then submit through the existing `POST /api/tasks` endpoint.
- Reset all controls (not just the text field) after a successful add.

Out of scope (explicit non-goals):

- No server/API changes. The contract already accepts a full todo.txt line via
  `{ "text": "..." }`; this feature only changes how the WebUI builds that line.
  Keeping the line as the single submission format avoids leaking a second
  task-construction path into the API.
- The inline edit flow (`window.prompt` with raw todo.txt) is unchanged. It
  already exposes the full line and is a separate concern from add ergonomics.
- No build step / framework — stays vanilla embedded HTML/CSS/JS.
- No free-text key:value tag editor. "Tags" here means the existing
  project/context labels; arbitrary `key:value` tags can still be typed in the
  description field, so no capability is removed.

## 2. Why "date" means the due date

The model has both a creation date and a `due:` tag. The server already stamps
the creation date automatically (`TODO_ADD_CREATION_DATE`, default on), so
exposing it as an input would be redundant and could fight the server stamp.
The due date (`due:YYYY-MM-DD`) is the date a user actually wants to set when
adding a task, so the date control maps to the `due:` tag. Documented here so
the choice is explicit rather than implied by the code.

## 3. Tag options: source and the empty-state tradeoff

The tag `<select>` is populated from the distinct `+project` and `@context`
values currently in the store. Each option's value is the full token
(`+work`, `@phone`) so composition is a direct concatenation with no
re-prefixing logic.

- Source of truth: a single `GET /api/tasks` with no filter, collected into a
  sorted, de-duplicated list. List responses already carry `projects` and
  `contexts` arrays, so no parsing happens in the WebUI.
- Refresh timing: tag options are loaded once on init and re-loaded after a
  successful add or edit (the mutations that can introduce a new label).
  Complete/uncomplete/delete do not create labels, so they do not trigger a
  reload — this keeps the extra request off the common toggle path. This is the
  one deliberate extra request; it is bounded (one unfiltered list fetch per
  add/edit) and documented here so it is not mistaken for an accidental N+1.
- Empty state: a brand-new file has no labels, so the tag select renders empty.
  That is expected — there is nothing to pick yet. Users create the first
  occurrence of a label by typing it in the description; thereafter it appears
  as a selectable option. The control is given a visible label/hint so an empty
  select is not read as a bug. No capability is lost in the empty state.

## 4. Composition: one function, one source of truth

A single `composeText()` builds the submitted line, so the "how an add becomes
a todo.txt line" decision lives in exactly one place (information hiding; no
duplication between submit handling and any preview). It is kept a pure,
side-effect-free function (reads control values, returns a string) so it stays
trivially testable if a frontend test harness is ever added:

```
function composeText() {
  const parts = [];
  const pri = priority value;          // "" | "A".."Z"
  if (pri) parts.push(`(${pri})`);
  const desc = input.value.trim();
  if (desc) parts.push(desc);
  for (const tok of selectedTagTokens) parts.push(tok);  // "+work" | "@phone"
  const due = dueInput.value;          // "" | "YYYY-MM-DD"
  if (due) parts.push(`due:${due}`);
  return parts.join(" ").trim();
}
```

Ordering note: priority is emitted first because todo.txt requires the leading
`(A)` priority to be the first token of the line. The remaining order
(description, then tags, then `due:`) is cosmetic; the parser is order-
independent for the description remainder.

## 5. Submission and validation

- The submit guard still requires a non-empty description (`input.value`). A
  line of only `(A) due:...` with no description is rejected on the client
  (return early, no request) to match today's "empty input does nothing"
  behavior and avoid creating contentless tasks. The free-text description
  remains the one required field.
- On success: clear the description, reset priority to none, clear the due
  input, deselect all tag options, then `refresh()` and reload tag options.
- Errors continue to flow through the existing `showError` path; no new error
  surface is introduced.

## 6. Markup and DOM construction

`web/index.html` — the form becomes two rows inside `#add-form`:

```
<form id="add-form" autocomplete="off">
  <div class="add-row">
    <input id="add-input" ... />
    <button type="submit">Add</button>
  </div>
  <div class="add-fields">
    <label>Priority <select id="add-priority" aria-label="Priority"></select></label>
    <label>Due <input id="add-due" type="date" aria-label="Due date" /></label>
    <label>Projects &amp; contexts <select id="add-tags" multiple aria-label="Projects and contexts"></select></label>
  </div>
</form>
```

- The 26 priority options and the tag options are generated in JS, not written
  out by hand in HTML, to avoid a 26-line literal block and keep the option
  set defined in one place. Priority options: a leading "Priority" placeholder
  with `value=""`, then `A`..`Z`.
- Each control has an `aria-label` (and a visible text label) for
  accessibility, matching the existing labelled inputs.

`web/app.js`:

- Extend the `els` cache with `priority`, `due`, `tags`.
- `initPriorityOptions()`: append the placeholder + `A`..`Z` once at startup.
- `loadTagOptions()`: fetch unfiltered tasks, build a sorted unique token list,
  rebuild the `#add-tags` options while preserving any still-valid current
  selection. Failures here are non-fatal (the form still works without
  suggestions) and surface through `showError` without blocking add.
- `selectedTagTokens()`: read selected `<option>` values from `#add-tags`.
- `composeText()` as in section 4; the submit handler calls it, guards on empty
  description, posts, then resets controls and reloads options.

## 7. Styling (`web/style.css`)

- `#add-form` becomes a column (`flex-direction: column; gap`) so the field row
  sits under the input row.
- `.add-row` keeps the existing input+button flex row layout (the old
  `form#add-form` rule moves here so the button still sits beside the input).
- `.add-fields` is a wrapping flex row of labelled controls, with muted small
  label text consistent with the existing `.meta` style. Reuse existing
  variables; no new color tokens.

## 8. Files touched

- `web/index.html` — restructure the add form into two rows; add three controls.
- `web/app.js` — element cache, option generation, tag loading, `composeText`,
  submit/reset wiring.
- `web/style.css` — two-row form layout and `.add-fields` styling.
- `README.md` — one line under the WebUI description noting the structured add
  controls (priority/due/tags) so docs match behavior.
- `implemented/add-task-selects.md` — written after implementation.

No Rust source, no API surface, no `Cargo.toml` change.

## 9. Verification

- Frontend has no automated test harness (the test suite is Rust API/unit
  tests, none of which this touches), so verification is manual:
  `cargo run`, open `http://127.0.0.1:8080`, then confirm:
  - Priority select prefixes `(X)`; due input appends `due:YYYY-MM-DD`;
    selecting tags appends the chosen `+`/`@` tokens; the composed task parses
    and renders as expected.
  - Newly introduced labels appear in the tag select after the add completes.
  - Empty description (with only priority/due set) is a no-op.
  - All controls reset after a successful add.
- `cargo test`, `cargo clippy --all-targets`, `cargo fmt` are still run to
  confirm the (unchanged) backend is green and nothing regressed. The web
  assets are embedded via `include_str!`, so `cargo build` revalidates that
  they are present and compiled in.

## 10. Risks

- Tag suggestions depend on existing labels, so the select is empty on a fresh
  file (section 3) — accepted and signposted in the UI; not a regression.
- One extra unfiltered list fetch per add/edit (section 3) — bounded and
  documented; off the toggle/delete path.
- A `<select multiple>` is a slightly clunky multi-pick control; it is the
  native, dependency-free way to honor "select options not text" and keeps the
  no-build-step constraint. Accepted for v1.
