# Plan: sort and filter dropdown menus (branch `sort-filter-dropdowns`)

Replace the WebUI's ad-hoc filter controls with dropdown menus and add a sort
dropdown. Today the task list is filtered by three buttons (All/Active/Completed)
plus a free-text `+project`/`@context` input, and is never sorted (it always
renders in file order). After this change the filter row is four `<select>`
menus — Status, Project, Context, Sort — all styled like the existing add-form
dropdowns.

## 1. Goal and scope

Requirements (from `agents/ADD_NEW_FEATURE.md`):

> - sort with dropdown menu
> - filter with dropdown menu

Interpretation (confirmed with the user before planning):

- **Filter with dropdown menu** = replace the `All`/`Active`/`Completed` buttons
  with a single **Status** `<select>`, and replace the free-text tag input with
  **Project** and **Context** `<select>`s populated from the labels already in the
  file. This reuses the existing label-gathering machinery (`loadLabels`) and the
  server's existing `completed` / `project` / `context` query params — no new API.
- **Sort with dropdown menu** = a new **Sort** `<select>`, applied **client-side**
  (no server change), offering: `File order` (default, the server's order),
  `Priority`, `Due date`, `Created`, `A–Z`.

In scope (WebUI only):

- `web/index.html` — swap the buttons + tag input inside `#filters` for four
  label-wrapped `<select>` controls.
- `web/app.js` — new filter/sort state, query building from it, a client-side sort
  step, a filter-select populator, and the wiring; remove the button/tag handlers.
- `web/style.css` — style `#filters` selects like the add-form dropdowns (sharing
  the existing rules), drop the now-dead `#filters button.active` and `#tag-filter`
  rules.
- `README.md` — document the new filter/sort dropdowns in the WebUI section.

Out of scope (explicit non-goals):

- **No server/API/Rust change.** Filtering uses the existing `completed`,
  `project`, and `context` query params (`src/api.rs` `ListQuery`,
  `src/store.rs` `Filter`); sorting is done in the browser. `Cargo.toml`, the
  store, the task parser, and the tests are untouched.
- **No multi-select filtering.** Each filter select picks at most one value, and
  Project + Context combine with AND (the server already ANDs them). This mirrors
  the single-select add-form dropdowns.
- **No new sort directions or secondary-sort UI.** Every sort key is ascending
  with a fixed tie-break (file order); there is no per-key asc/desc toggle.
- **No change to label freshness triggers.** Filter dropdowns are repopulated by
  the existing `loadLabels` calls (initial load, after add, after edit), exactly
  like the add-form dropdowns. The pre-existing minor staleness after a *delete*
  (a label whose last task was deleted lingers until the next `loadLabels`) is
  retained as-is for both dropdown families — fixing it is a separate concern.
- No build step / framework — stays vanilla embedded HTML/CSS/JS served via
  `include_str!`.

## 2. Filter and sort state

Replace the current two-field `state` —

```
const state = { filter: "all", tag: "" };
```

— with four fields. The Project/Context keys are deliberately named to match the
`KINDS` keys (`project`, `context`) so the existing kind loop can read
`state[kind]`, exactly as it already reads `els[kind]`:

```
const state = {
  status: "all",  // all | active | completed  -> completed query param
  project: "",    // "" = any project, else an exact project name
  context: "",    // "" = any context, else an exact context name
  sort: "file",   // key into SORTS
};
```

`buildQuery` is rewritten to read the new state. It looks the status mapping up in
the `STATUSES` config (section 5) rather than re-deciding what each value means, so
the value→`completed` decision has a single source. The project/context params are
already supported by the server and already AND-combine there, so they are a direct
substitution of the old `state.tag` sigil-parsing:

```
function buildQuery() {
  const params = new URLSearchParams();
  const { completed } = STATUSES[state.status];
  if (completed !== null) params.set("completed", String(completed));
  if (state.project) params.set("project", state.project);
  if (state.context) params.set("context", state.context);
  const qs = params.toString();
  return qs ? `/api/tasks?${qs}` : "/api/tasks";
}
```

## 3. Client-side sort

The server returns file order; sorting happens after the fetch, in the browser.
The sort orders live in one config so the dropdown labels and the comparators have
a single source:

```
// Client-side sort orders. compare:null keeps the server's file order. Every
// comparator sorts ascending and sends tasks missing the key to the bottom
// (cmpMissingLast); JS's stable sort then keeps file order among ties.
const SORTS = {
  file:     { label: "File order", compare: null },
  priority: { label: "Priority",   compare: (a, b) => cmpMissingLast(a.priority, b.priority) },
  due:      { label: "Due date",   compare: (a, b) => cmpMissingLast(a.tags.due, b.tags.due) },
  created:  { label: "Created",    compare: (a, b) => cmpMissingLast(a.creation_date, b.creation_date) },
  alpha:    { label: "A-Z",        compare: (a, b) => a.description.localeCompare(b.description, undefined, { sensitivity: "base" }) },
};
```

One shared comparator handles the "absent value sinks to the bottom" rule so it is
not re-implemented per key:

```
// Compare two optional sort keys ascending, with absent values (null/undefined)
// always ordering after present ones — so "no priority" / "no due date" sink to
// the bottom. ISO date strings and single-char priorities compare correctly with
// <, so no parsing is needed.
function cmpMissingLast(a, b) {
  if (a == null && b == null) return 0;
  if (a == null) return 1;
  if (b == null) return -1;
  return a < b ? -1 : a > b ? 1 : 0;
}
```

Why these keys are read directly off the DTO (no parsing): `priority` is a
nullable single-char string, `creation_date` a nullable `YYYY-MM-DD` string, and
`tags.due` is the `due:` value (a `YYYY-MM-DD` string) or `undefined` when absent
— all already in the JSON shape returned by `/api/tasks` (see `TaskDto` in
`src/api.rs`). ISO dates and priority letters sort correctly as plain strings, so
`cmpMissingLast` needs no date/number coercion.

`sortTasks` returns a sorted copy (never mutating the fetched array) and is a
no-op for `file` order:

```
function sortTasks(tasks) {
  const compare = SORTS[state.sort].compare;
  return compare ? [...tasks].sort(compare) : tasks;
}
```

### Render vs. refetch

Sorting needs no server round-trip, so changing the sort must not refetch. Split
the current `refresh` into a fetch step and a render step, caching the last fetch:

```
let lastTasks = [];  // most recent fetch, re-sorted on a sort change without refetching

function render() {
  els.list.replaceChildren(...sortTasks(lastTasks).map(taskElement));
  els.empty.hidden = lastTasks.length !== 0;
}

async function refresh() {
  showError("");
  try {
    lastTasks = await api("GET", buildQuery());
    render();
  } catch (err) {
    showError(`Could not load tasks: ${err.message}`);
  }
}
```

A **filter** change (status/project/context) refetches via `refresh()` because the
filtering is server-side; a **sort** change calls `render()` only. `taskElement`,
`renderMeta`, `toggleComplete`, `deleteTask`, `editTask` are unchanged — they
still call `refresh()` (and, where they already do, `loadLabels()`).

## 4. Filter label selects

The add-form Project/Context selects and the new filter Project/Context selects
are different controls with different option sets, so they get a second, simpler
populator rather than overloading `populateLabelSelect` with flags:

- The **add-form** select (`populateLabelSelect`, unchanged) has the `+ Add ...`
  sentinel entry, a `None` empty option, and unions in client-`staged` labels not
  yet on any task.
- The **filter** select has neither a sentinel nor staged labels (you can only
  filter by a label some task actually has) — just an "all" option then the sorted
  names.

```
// Rebuild one filter label select from the labels currently in the store. Unlike
// the add-form select it has no "add" entry and no staged labels — just an "all"
// option, then the sorted names. The active choice is kept across reloads while it
// still exists; if its label is gone the select falls back to "all" and the
// matching filter state is cleared so the dropdown and the query never disagree.
function populateFilterSelect(kind, names) {
  const select = filterEls[kind];
  const previous = select.value;
  select.replaceChildren();
  select.add(new Option(`All ${KINDS[kind].singular}s`, ""));
  for (const name of [...names].sort()) select.add(new Option(name, name));
  const kept = names.has(previous) ? previous : "";
  select.value = kept;
  state[kind] = kept;
}
```

`All ${KINDS[kind].singular}s` composes `All projects` / `All contexts` from the
existing `singular` field (both pluralize with a bare `s`), so the change does not
reintroduce the `plural` field removed by the dropdown-menu-cleanup change.

`loadLabels` already computes `server = { project: Set, context: Set }` from one
unfiltered `/api/tasks` fetch. Reuse that single computation to feed both dropdown
families in its existing kind loop — the add select gets server∪staged (unchanged),
the filter select gets server-only labels:

```
for (const kind of Object.keys(KINDS)) {
  populateLabelSelect(kind, new Set([...server[kind], ...staged[kind]]));  // add form (unchanged)
  populateFilterSelect(kind, server[kind]);                               // filter (new)
}
```

`populateFilterSelect` writing `state[kind]` is what keeps the visible dropdown and
the query in sync when a filtered label disappears (only possible via an edit that
removes a label's last use; `loadLabels` runs after edit). The pre-existing
post-delete staleness noted in section 1 is unchanged.

## 5. Element registry and markup

`web/index.html` — replace the contents of `#filters`. Status and Sort options are
built in JS (kept beside their config/state), so the markup is four empty selects:

```
<div id="filters">
  <label>Status  <select id="filter-status"  aria-label="Status filter"></select></label>
  <label>Project <select id="filter-project" aria-label="Project filter"></select></label>
  <label>Context <select id="filter-context" aria-label="Context filter"></select></label>
  <label>Sort    <select id="filter-sort"    aria-label="Sort order"></select></label>
</div>
```

(The label text/select pairs follow the same `<label>`-wraps-`<select>` shape as
`.add-fields`; final indentation/line breaks match the file's 2-space style.)

`web/app.js` `els` changes: remove `tagFilter` and `filterButtons`; add `status`
(`#filter-status`) and `sort` (`#filter-sort`). Add a `filterEls` registry keyed by
kind, parallel to how `els[kind]` keys the add-form selects:

```
const filterEls = {
  project: document.getElementById("filter-project"),
  context: document.getElementById("filter-context"),
};
```

Status options come from a config keyed by `<select>` value, carrying the
`completed` query meaning beside the label — the same label-plus-behavior shape as
`SORTS` — so `buildQuery` reads the mapping instead of re-deciding it:

```
// Status filter options keyed by their <select> value. `completed` is the query
// param the option maps to (null = omit it), kept beside the label so buildQuery
// never re-decides what a status means (same shape as SORTS: label + behavior).
const STATUSES = {
  all:       { label: "All",       completed: null },
  active:    { label: "Active",    completed: false },
  completed: { label: "Completed", completed: true },
};

function initFilterOptions() {
  for (const [value, s] of Object.entries(STATUSES)) els.status.add(new Option(s.label, value));
  for (const [key, s] of Object.entries(SORTS)) els.sort.add(new Option(s.label, key));
  els.status.value = state.status;
  els.sort.value = state.sort;
  // Seed each filter label select with just its "all" option so it is never an
  // empty <select> before the async loadLabels fills in the real labels.
  for (const kind of Object.keys(KINDS)) populateFilterSelect(kind, new Set());
}
```

Wiring (replaces the `filterButtons.forEach` click handlers and the debounced
`tagFilter` input handler, both removed along with `tagTimer`):

```
els.status.addEventListener("change", () => { state.status = els.status.value; refresh(); });
for (const kind of Object.keys(KINDS))
  filterEls[kind].addEventListener("change", () => { state[kind] = filterEls[kind].value; refresh(); });
els.sort.addEventListener("change", () => { state.sort = els.sort.value; render(); });
```

Startup sequence at the bottom of the file gains `initFilterOptions()`:

```
initPriorityOptions();
initFilterOptions();
refresh();
loadLabels();
```

## 6. Styling

`web/style.css`:

- Broaden the existing add-form label/select rules to also cover `#filters`, so the
  two filter dropdowns and the add dropdowns share one styling source rather than
  duplicating it:
  - `.add-fields label` -> `.add-fields label, #filters label`
  - `.add-fields select, .add-fields input[type="date"]` ->
    `.add-fields select, #filters select, .add-fields input[type="date"]`
- Keep the `#filters` flex container (gap, margin, wrap); set `align-items:
  flex-end` so the selects' bottoms line up under their stacked captions.
- Remove the now-dead `#filters button.active` rule (no buttons remain) and the
  `#tag-filter` rule (the input is gone).

No other selectors change; `.add-fields` itself is untouched.

## 7. Files touched

- `web/index.html` — `#filters` body swapped for the four selects (section 5).
- `web/app.js`
  - `state`: `filter`/`tag` -> `status`/`project`/`context`/`sort` (section 2).
  - Add `STATUSES` (keyed map, carries the `completed` mapping), `SORTS`,
    `cmpMissingLast`, `sortTasks` (sections 2–3, 5).
  - `els`: drop `tagFilter`/`filterButtons`, add `status`/`sort`; add `filterEls`
    (section 5).
  - `buildQuery`: read the new state (section 2).
  - Split `refresh` into `refresh` + `render` with `lastTasks` (section 3).
  - Add `populateFilterSelect`; feed it from `loadLabels`' existing loop (section 4).
  - Add `initFilterOptions`; replace button/tag wiring with select wiring; call
    `initFilterOptions()` at startup (section 5).
- `web/style.css` — share label/select rules with `#filters`, drop dead rules
  (section 6).
- `README.md` — WebUI section: describe the Status/Project/Context filter dropdowns
  and the Sort dropdown (and that sort is client-side, file order by default).
- `implemented/sort-filter-dropdowns.md` — written after implementation.

No Rust source, no API surface, no `Cargo.toml`/store/task/test change.

## 8. Verification

The frontend has no automated test harness (the suite is Rust API/unit tests, none
of which this touches), consistent with every prior WebUI feature
(`add-task-selects`, `project-context-dropdowns`, `label-add-modal`,
`dropdown-menu-cleanup`). Feature verification is manual: `cargo run`, open
`http://127.0.0.1:8080`, then confirm:

- The filter row shows four dropdowns: Status, Project, Context, Sort.
- Status `All`/`Active`/`Completed` filters by completion exactly as the old
  buttons did (`Active` hides completed tasks, `Completed` shows only them).
- Project and Context list the labels in the file (after at least one task uses
  them) plus an `All projects` / `All contexts` option; picking one narrows the
  list, and picking both ANDs them; choosing `All ...` clears that filter.
- Sort reorders the visible list without a refetch: `Priority` puts `(A)` first and
  unprioritised tasks last; `Due date` orders by `due:` ascending with no-due last;
  `Created` orders by creation date with undated last; `A-Z` orders by description
  case-insensitively; `File order` restores the server order. Ties keep file order.
- Changing the sort while a filter is active keeps the filter (no refetch); changing
  a filter keeps the sort.
- After adding or editing a task, new labels appear in both the add-form and the
  filter dropdowns; an in-flight filter selection survives a reload while its label
  still exists.

`cargo test`, `cargo clippy --all-targets`, and `cargo fmt --check` are run to
confirm the unchanged backend is green; the web assets are embedded via
`include_str!`, so `cargo build` revalidates they are present.

## 9. Risks

- **Sort refetch.** A naive implementation would refetch on sort change. Mitigated
  in section 3 by caching `lastTasks` and splitting `render` from `refresh`, so a
  sort change only re-renders.
- **Filter/dropdown disagreement.** If a filtered label vanishes, the dropdown
  could show a stale selection while the query still sends it. Mitigated in
  section 4: `populateFilterSelect` falls back to `""` and writes `state[kind]` so
  the two never diverge after a `loadLabels`. The post-*delete* case (no
  `loadLabels` call) is pre-existing and explicitly out of scope (section 1).
- **Stable-sort assumption.** Tie-breaking on file order relies on
  `Array.prototype.sort` being stable, which it is in every engine since ES2019;
  acceptable for a browser UI with no IE target.
- **Two label populators.** `populateLabelSelect` and `populateFilterSelect` look
  similar. Kept separate deliberately (section 4): merging them would need
  add-entry/staged/empty-label flags — flag-driven complexity worse than two short,
  single-purpose functions. The shared knowledge (the label *set*, the kind config)
  is already centralised in `loadLabels`/`KINDS`.
- **No JS test harness** (deliberate no-build-step decision, consistent with prior
  WebUI features), so this is constraint-bound to manual verification plus the
  green Rust gates.

## 10. brooks-review outcome

`reviews/sort-filter-dropdowns.md` first scored the plan 99/100 with one
Suggestion (R3 Knowledge Duplication): the status values were enumerated in a
`STATUSES` table but their `completed`-param meaning was re-decided by bare literal
inside `buildQuery`, splitting one decision across two places and diverging from
the sibling `SORTS` config (which already pairs each label with its behavior).
Adopted: sections 2 and 5 now make `STATUSES` a keyed map whose entry carries the
`completed` value beside the label, and `buildQuery` reads
`STATUSES[state.status].completed` — a single source for the mapping and the same
label-plus-behavior shape as `SORTS`. Re-review scored 100/100, clearing the 95
gate. The only remaining note is the constraint-bound absence of a JS test harness
(no-build-step decision, consistent with every prior WebUI feature), which is not a
deduction.
