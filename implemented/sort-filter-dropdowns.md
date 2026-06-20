# Implemented: sort and filter dropdown menus (branch `sort-filter-dropdowns`)

Implements `plans/sort-filter-dropdowns.md` (brooks-review 100/100). WebUI-only; no
Rust/API/store/test change. The `#filters` row is now four `<select>` menus —
Status, Project, Context, Sort — replacing the old All/Active/Completed buttons and
the free-text `+project`/`@context` input.

## Changes

- `web/index.html` — `#filters` body replaced by four `<label>`-wrapped selects
  (`filter-status`, `filter-project`, `filter-context`, `filter-sort`).
- `web/app.js`
  - `state`: `filter`/`tag` → `status`/`project`/`context`/`sort`.
  - `STATUSES` keyed map carries each status's `completed` query value beside its
    label (single source for the mapping; same shape as `SORTS`); `buildQuery`
    reads `STATUSES[state.status].completed` and the `project`/`context` state.
  - `SORTS` config + `cmpMissingLast` (absent keys sort last) + `sortTasks`
    (sorts a copy; `file` order is a no-op). Keys: File order, Priority, Due date,
    Created, A-Z.
  - `refresh` split into `refresh` (fetch → `render`) and `render` (sort + paint),
    over a cached `lastTasks`, so a sort change re-renders without refetching while
    a filter change refetches.
  - `filterEls` registry keyed by kind; `populateFilterSelect` (no add entry, no
    staged labels, an `All <kind>s` option, writes back `state[kind]` if a label
    vanished) fed from `loadLabels`' existing kind loop alongside the add selects.
  - `initFilterOptions` populates Status/Sort and seeds the filter label selects;
    select-change wiring replaces the removed button-click and debounced-tag-input
    handlers.
- `web/style.css` — add-form label/select rules broadened to also cover `#filters`
  (shared styling); dead `#filters button.active` and `#tag-filter` rules removed;
  `#filters` set to `align-items: flex-end`.
- `README.md` — WebUI section documents the Status/Project/Context filter dropdowns
  and the client-side Sort dropdown.

## Verification

- `cargo fmt --check`, `cargo build`, `cargo test` (7 API + store/lib unit tests),
  `cargo clippy --all-targets` — all green; web assets revalidated via
  `include_str!`.
- Manual smoke test against `cargo run`: `/api/health` ok; combined filter
  `?completed=false&project=asdf` returns the matching task; served `/` and
  `/static/app.js` contain the new filter/sort markup and logic.
- No JS test harness (deliberate no-build-step decision, consistent with prior
  WebUI features); sort/filter behavior verified manually per plan section 8.
