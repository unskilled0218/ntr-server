# Implemented: my-ntr-server (branch `new`)

Implements the plan in `plans/new.md` (cleared brooks-review at 99/100, see
`reviews/new.md`).

## What was built

A userless Rust todo server over a single todo.txt file, with a JSON HTTP API
and an embedded WebUI.

### Files

- `Cargo.toml` — lib + bin targets (lib `my_ntr_server` so integration tests can
  import modules). Deps: axum 0.8, tokio, tower-http (cors), serde/serde_json,
  chrono.
- `rustfmt.toml` — `tab_spaces = 2` (honors AGENTS.md).
- `src/task.rs` — `Task` model: `parse`, `Display` (round-trip), `derived()`
  (single-pass projects/contexts/tags), `complete`/`uncomplete` with `pri:X`
  priority preservation. Unit tests included.
- `src/store.rs` — `Store` over the todo.txt file. Process-stable ids, atomic
  persistence (temp file + rename, write-then-swap). Unit tests included.
- `src/config.rs` — env-only `Config` (`TODO_FILE`, `HOST`, `PORT`,
  `TODO_ADD_CREATION_DATE`) with a `Default` impl.
- `src/api.rs` — axum router, `TaskDto`, `AppError` (-> proper HTTP statuses),
  handlers for all endpoints, permissive CORS, embedded static assets.
- `src/main.rs` — thin binary: load config, open store, serve.
- `web/index.html`, `web/style.css`, `web/app.js` — vanilla WebUI, no build step,
  no external assets.
- `tests/api.rs` — integration tests driving the router via `tower::oneshot`.

## Review findings, all applied

- R2 (Pass 1 Warning): line-index ids replaced with process-stable in-memory
  ids; deletes no longer renumber, missing ids 404.
- R4 (Pass 1 Suggestion): config is env-var-only (no bespoke arg parser).
- R1 (Pass 1 Suggestion): derived fields parsed once via `Task::derived()`.

## Deviations from the plan

- `thiserror` was listed as a dependency but not used; errors are small hand
  -rolled enums (`StoreError`, `AppError`), so the dependency was dropped.

## Verification

- `cargo test` — 21 tests pass (8 task unit, 6 store unit, 7 api integration).
- `cargo clippy --all-targets` — clean.
- `cargo fmt --check` — clean.
- Manual smoke test (curl against a running instance): health, add (with
  creation-date stamping), list, complete (priority -> `pri:A`, two dates),
  `completed=false` filter, `GET /api/raw`, `DELETE` (204), WebUI index, and
  on-disk file contents all verified correct.

Note for the environment: `clippy` and `rustfmt` rustup components were added
(`rustup component add clippy rustfmt`); no system packages were needed beyond
the existing cargo 1.96 toolchain.
