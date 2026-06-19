# Plan: my-ntr-server (branch `new`)

A single-binary Rust todo server with no user/auth concept. It stores tasks in a
plain `todo.txt` file (following the todo.txt format spec), exposes a JSON HTTP
API for use by other programs, and serves a small self-contained WebUI.

## 1. Goals and scope

In scope:

- Parse and serialize the todo.txt format with round-trip fidelity.
- Persist to a single `todo.txt` file (configurable path), as the canonical store.
- JSON HTTP API: list, get, add, update, complete/uncomplete, delete, and
  raw-file get/replace.
- WebUI: list, filter, add, toggle complete, edit, delete.
- Configurable bind address and file path.
- Unit tests (parser round-trip, store ops) and API integration tests.

Out of scope (explicit non-goals):

- Users, authentication, authorization, multi-tenancy (requirement: "without user").
- Recurrence, reminders, sync to external services.
- A build step / framework for the WebUI (kept as plain embedded HTML/CSS/JS).

## 2. todo.txt format support

Reference: https://github.com/todotxt/todo.txt

Each line is one task. Supported elements:

- Completion: a line starting with `x ` (lowercase x + space) is complete.
- Completion date: the date immediately after `x ` (only valid when complete).
- Priority: `(A)`..`(Z)` in parentheses at the start, followed by a space.
- Creation date: `YYYY-MM-DD` after the priority (or at line start if no priority).
- Projects: whitespace-preceded `+project` tokens.
- Contexts: whitespace-preceded `@context` tokens.
- Key/value tags: `key:value` tokens (e.g. `due:2026-06-30`).

Parsing rules and edge cases to handle:

- Dates are parsed as `chrono::NaiveDate`; tokens that look like dates but are
  invalid stay as plain text in the description.
- For complete tasks, two leading dates mean `completion_date creation_date`;
  one leading date means `completion_date` only.
- Priority on a complete task: the spec drops the leading `(A)`; we preserve a
  dropped priority as a `pri:A` tag on completion and restore it on
  un-completion. This keeps round-trips lossless.
- Blank lines and lines that are only whitespace are ignored on load (not
  treated as tasks), and are not re-emitted.
- The serializer must reproduce a semantically equivalent line; we keep the
  original `description` substring verbatim so projects/contexts/tags embedded
  in it are preserved in place rather than reordered.

## 3. Domain model

`struct Task` (in `src/task.rs`):

- `completed: bool`
- `priority: Option<char>` (validated `A`..=`Z`)
- `completion_date: Option<NaiveDate>`
- `creation_date: Option<NaiveDate>`
- `description: String` (the free-text remainder, verbatim)
- Derived fields `projects`, `contexts`, `tags`: the `description` is the
  authoritative source, but to avoid re-scanning it three-plus times per task
  per response (review Suggestion R1), a single `Task::derived() -> Derived`
  helper scans the description once and returns all three together. DTO
  construction calls it once; nothing caches stale copies.

Functions:

- `Task::parse(line: &str) -> Option<Task>` (None for blank lines).
- `impl Display for Task` -> canonical todo.txt line.
- `Task::complete(today)`, `Task::uncomplete()` with priority preservation.

## 4. Store

`struct Store` (in `src/store.rs`), holds `path: PathBuf`, `tasks: Vec<Entry>`,
and `next_id: u64`, where `Entry { id: u64, task: Task }`.

- IDs: process-stable. At load, each non-blank line is assigned a monotonically
  increasing `id` from `next_id`; `add` assigns the next id and never reuses one
  for the lifetime of the process. The todo.txt file stays pure (no synthetic id
  is written into it) — ids live only in memory and are derived from load order.
  Rationale (resolves review Warning R2): the API has two concurrent clients by
  requirement (WebUI + external programs). With line-index ids, a read-then-act
  sequence has a time-of-check/time-of-use gap: client A reads id 3, client B
  deletes id 1, and A's next mutation on id 3 hits the wrong task. Stable ids
  close that gap — a deleted id simply 404s instead of silently aliasing another
  task. The file is still canonical; ids are an in-memory index over it.
- `GET /api/tasks` returns ids in file order; deleting an id removes its entry
  without shifting other ids. On `PUT /api/raw` (whole-file replace) the store
  reloads and reassigns ids from `next_id` (old ids become invalid); this is the
  one documented case where ids rotate, and it is an explicit bulk operation.
- `load(path)`: read file (create empty if missing), parse non-blank lines,
  assign ids.
- `list(filter)`: optional filter by completed/priority/project/context.
- `get(id)`, `add(text)`, `update(id, text)`, `complete(id)`, `uncomplete(id)`,
  `delete(id)`.
- `raw()` / `replace_raw(text)`: get/replace whole file content.
- Persistence: every mutation rewrites the file atomically (write to
  `todo.txt.tmp` in the same dir, then `rename` over the target) to avoid
  partial writes. Returns `io::Error` on failure (mutation is still applied
  in memory only after a successful write — write first, then swap in-memory).

Concurrency: the store is wrapped in `Arc<RwLock<Store>>` in app state. Reads
take a read lock, mutations take a write lock; file writes happen under the
write lock so the file and memory never diverge.

## 5. HTTP API

Framework: `axum` on `tokio`. JSON via `serde`/`serde_json`. All task responses
serialize to a stable JSON shape:

```
{ "id": 1, "completed": false, "priority": "A",
  "completion_date": null, "creation_date": "2026-06-19",
  "description": "buy milk +groceries @store due:2026-06-30",
  "projects": ["groceries"], "contexts": ["store"],
  "tags": { "due": "2026-06-30" }, "raw": "(A) 2026-06-19 buy milk ..." }
```

Endpoints:

- `GET  /api/tasks` — list; query params `completed`, `priority`, `project`,
  `context` filter the result.
- `POST /api/tasks` — body `{ "text": "<raw todo.txt line>" }`; returns created
  task with its id. Server stamps a creation date if `add_creation_date` is on
  and none is present.
- `GET  /api/tasks/:id` — single task; 404 if missing.
- `PUT  /api/tasks/:id` — body `{ "text": "<raw line>" }`; replace task.
- `POST /api/tasks/:id/complete` and `/uncomplete` — toggle completion.
- `DELETE /api/tasks/:id` — delete; 204 on success.
- `GET  /api/raw` — `text/plain` whole file.
- `PUT  /api/raw` — replace whole file from `text/plain` body.
- `GET  /api/health` — `{ "status": "ok" }`.

Errors: a single `AppError` (`thiserror`) maps to JSON `{ "error": "msg" }`
with status codes: 400 invalid input, 404 not found, 500 io error. IDs out of
range -> 404.

CORS: permissive CORS (`tower-http` `CorsLayer`) so other programs/origins can
call the API. Documented as intentional given the no-auth design.

## 6. WebUI

- `GET /` serves an HTML page embedded via `include_str!` from `web/index.html`;
  `web/app.js` and `web/style.css` served at `/static/*` (also embedded).
- Vanilla JS (no build step). Features: list with filter controls
  (all/active/completed, by project/context), add box (accepts raw todo.txt
  syntax), checkbox to complete/uncomplete, inline edit, delete button,
  priority shown as a badge.
- Calls the same `/api/*` endpoints. No external CDN assets (works offline).

## 7. Configuration

Read at startup from environment variables only (no bespoke arg parser, no clap
dependency — resolves review Suggestion R4; env vars are the standard, trivial,
dependency-free config surface for a single-binary server):

- `TODO_FILE` (default `./todo.txt`)
- `HOST` (default `127.0.0.1`)
- `PORT` (default `8080`)
- `TODO_ADD_CREATION_DATE` (default `true`; accepts `true`/`false`/`1`/`0`)

Default bind is loopback. README notes: there is no auth by design, so exposing
the port to a network means anyone with access can read/modify tasks — run
behind a reverse proxy with auth if exposed.

## 8. Project layout

```
Cargo.toml
src/
  main.rs        # config, build router, start server
  task.rs        # Task model, parse, Display, complete/uncomplete
  store.rs       # Store: load/list/get/add/update/complete/delete/persist
  api.rs         # axum handlers, AppError, JSON DTOs
  config.rs      # Config parsing
web/
  index.html
  app.js
  style.css
tests/
  api.rs         # integration tests against the router
```

## 9. Dependencies

- `tokio` (rt-multi-thread, macros)
- `axum`
- `tower-http` (cors, optionally trace)
- `serde`, `serde_json`
- `chrono` (NaiveDate; default features off where possible)
- `thiserror`
- dev: `tower` (`ServiceExt::oneshot` for integration tests)

Note: `cargo build` fetches these from crates.io, which requires network
access. If the sandbox is offline, that must be enabled before building. No
system packages are required beyond the existing Rust toolchain (cargo 1.96).

## 10. Testing

- `task.rs` unit tests: round-trip parse->Display for representative lines
  (priority, dates, complete/incomplete, projects/contexts/tags, priority
  preservation on complete/uncomplete, invalid-date-as-text).
- `store.rs` unit tests (using a `tempfile` dir or unique temp path): add,
  list+filter, complete/uncomplete, update, delete renumbering, atomic persist
  reloads identically.
- `tests/api.rs`: drive the router with `oneshot` for each endpoint, asserting
  status codes and JSON bodies, including 404 and 400 paths.
- `cargo test` must pass; `cargo clippy` clean; `cargo fmt` applied (2-space
  indent is not cargo-fmt default, so we set `tab_spaces = 2` in `rustfmt.toml`
  to honor AGENTS.md).

## 11. Risks / open questions

- IDs are process-stable but not persistent across restarts or `PUT /api/raw`
  (they are reassigned from load order). This is documented; persistent ids
  would require writing an `id:` tag into todo.txt, which we reject to keep the
  file portable to other todo.txt tools. Flagged, not built.
- todo.txt has many informal extensions; we support the core spec plus `pri:`
  preservation and generic `key:value` tags, which covers common clients.
- No auth by requirement; mitigated by loopback default + explicit docs.

## 12. Execution steps

1. `Cargo.toml`, `rustfmt.toml` (`tab_spaces = 2`), module skeleton.
2. Implement `task.rs` + unit tests; verify round-trip.
3. Implement `store.rs` + unit tests.
4. Implement `config.rs`.
5. Implement `api.rs` (handlers, DTOs, errors) and wire router in `main.rs`.
6. Build WebUI (`web/*`), embed and serve.
7. Integration tests `tests/api.rs`.
8. `cargo fmt`, `cargo clippy`, `cargo test`; update README; write
   `implemented/new.md`; commit.
