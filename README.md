# my-ntr-server

A single-binary Rust todo server with no user/auth concept. Tasks are stored in
a plain [todo.txt](https://github.com/todotxt/todo.txt) file (the canonical
store), exposed over a JSON HTTP API for use by other programs.

## Run

```sh
cargo run
# then call the API, e.g. curl http://127.0.0.1:8080/api/tasks
```

Configuration (environment variables):

| Variable                 | Default       | Meaning                                            |
| ------------------------ | ------------- | -------------------------------------------------- |
| `TODO_FILE`              | `todo.txt`    | Path to the todo.txt file.                         |
| `NOTE_FILE`              | `Note.md`     | Path to the free-form Markdown note file.          |
| `HOST`                   | `127.0.0.1`   | Bind address.                                      |
| `PORT`                   | `8080`        | Bind port.                                         |
| `TODO_ADD_CREATION_DATE` | `true`        | Stamp a creation date on new incomplete tasks.     |

> Security: there is no authentication by design. Keep the default loopback
> bind, or put it behind an authenticating reverse proxy before exposing it.

### With Docker Compose

`docker-compose.yml` runs the server straight from source using the official Rust
image — no Dockerfile to build:

```sh
docker compose up                  # API on http://localhost:8080/api/...
NTR_PORT=9000 docker compose up     # use a different port
NTR_BIND=0.0.0.0 docker compose up  # reach it from other devices on your network
```

The server is published on `127.0.0.1` only by default, matching the app's loopback
default — it is unauthenticated, so exposing it to the network (`NTR_BIND=0.0.0.0`)
is a deliberate choice; do it only on a trusted network or behind an authenticating
proxy.

`todo.txt` and `Note.md` live in `./data` on the host, so they persist across
restarts and can be edited out of band (the server watches them and pushes the
changes to open clients). The first `up` compiles the dependencies (a few minutes);
the cargo build/registry caches are kept in named volumes, so later starts are fast.

## API

All task responses use this JSON shape:

```json
{
  "id": 1,
  "completed": false,
  "priority": "A",
  "completion_date": null,
  "creation_date": "2026-06-19",
  "description": "buy milk +groceries @store due:2026-06-30",
  "projects": ["groceries"],
  "contexts": ["store"],
  "tags": { "due": "2026-06-30" },
  "raw": "(A) 2026-06-19 buy milk +groceries @store due:2026-06-30"
}
```

| Method & path                    | Description                                       |
| -------------------------------- | ------------------------------------------------- |
| `GET /api/health`                | Liveness check.                                   |
| `GET /api/tasks`                 | List tasks. Filters: `completed`, `priority`, `project`, `context`. |
| `POST /api/tasks`                | Create from `{ "text": "<todo.txt line>" }`.      |
| `GET /api/tasks/{id}`            | Fetch one task.                                   |
| `PUT /api/tasks/{id}`            | Replace from `{ "text": "<todo.txt line>" }`.     |
| `POST /api/tasks/{id}/complete`  | Mark complete.                                     |
| `POST /api/tasks/{id}/uncomplete`| Mark incomplete.                                  |
| `DELETE /api/tasks/{id}`         | Delete (204 on success).                          |
| `GET /api/raw`                   | Whole file as `text/plain`.                       |
| `PUT /api/raw`                   | Replace the whole file from a `text/plain` body.  |
| `GET /api/note`                  | The Markdown note as `text/markdown`.             |
| `PUT /api/note`                  | Replace the note from a text body (`204`).        |
| `GET /api/events`                | Server-Sent Events stream of `change` events (`tasks` / `note`) for live updates. |

### Task ids

Ids are assigned at load time and are stable for the lifetime of the running
process: deleting a task does not renumber the others, and a deleted id returns
404 rather than silently aliasing another task. Ids are not written into the
todo.txt file (it stays portable to other todo.txt tools) and are reassigned on
restart or on a `PUT /api/raw` bulk replace.

## todo.txt support

Priorities `(A)`–`(Z)`, completion (`x`), completion/creation dates,
`+projects`, `@contexts`, and `key:value` tags. When a task is completed its
priority is preserved as a `pri:X` tag and restored on un-completion.

## Note (Note.md)

Alongside the task file the server keeps a single free-form Markdown note in
`NOTE_FILE` (default `Note.md`). It is userless and unstructured — one block of
text, the scratchpad counterpart to the structured todo.txt — stored in its own
file, cached in memory, and written atomically (temp file + rename) just like the
task store, so the file and memory never diverge. A missing file is treated as an
empty note. There is no per-task note and no versioning; `PUT /api/note` is
last-write-wins, the same as `PUT /api/raw` for the task file — a plain overwrite.

## Live updates

`GET /api/events` is a Server-Sent Events stream for clients that want to stay in sync
without polling. Each time a resource changes it emits one `change` event whose payload is
`tasks` or `note`; a subscriber reacts by re-fetching that resource through the normal GET
endpoints. Changes are announced whether they come from the API itself (any mutation) or
from an out-of-band edit to `todo.txt` / `Note.md` by another program — the server watches
the files and broadcasts the difference. A subscriber that falls behind the buffer skips the
missed events and re-syncs on its next event or on reconnect. The stream carries no auth and
no payload beyond the resource name, matching the rest of the API.

## Development

```sh
cargo test     # unit + integration tests
cargo fmt      # 2-space indent (see rustfmt.toml)
cargo clippy --all-targets
```
