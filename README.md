# my-ntr-server

A single-binary Rust todo server with no user/auth concept. Tasks are stored in
a plain [todo.txt](https://github.com/todotxt/todo.txt) file (the canonical
store), exposed over a JSON HTTP API for use by other programs, and served
through a small self-contained WebUI.

## Run

```sh
cargo run
# then open http://127.0.0.1:8080
```

Configuration (environment variables):

| Variable                 | Default       | Meaning                                            |
| ------------------------ | ------------- | -------------------------------------------------- |
| `TODO_FILE`              | `todo.txt`    | Path to the todo.txt file.                         |
| `HOST`                   | `127.0.0.1`   | Bind address.                                      |
| `PORT`                   | `8080`        | Bind port.                                         |
| `TODO_ADD_CREATION_DATE` | `true`        | Stamp a creation date on new incomplete tasks.     |

> Security: there is no authentication by design. Keep the default loopback
> bind, or put it behind an authenticating reverse proxy before exposing it.

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

## WebUI

The page at `/` lists, filters, and edits tasks. The add form has a free-text
description plus structured controls beneath it — a priority dropdown (`A`–`C`),
a due-date picker (written as a `due:` tag), and separate multi-select dropdowns
for `+project` and `@context` labels — so common fields can be set by picking
options instead of typing todo.txt syntax. Each label dropdown carries an "Add"
button that opens a modal for creating a new project or context; a newly added
label is selectable immediately and persists once a task using it is saved
(todo.txt has no separate label store). The dropdowns are empty until the file
contains at least one project or context.

The `A`–`C` limit applies only to the picker: tasks created or edited as raw
todo.txt (via the edit prompt or `PUT /api/raw`) may carry any `(A)`–`(Z)`
priority, and it is displayed as-is.

## Development

```sh
cargo test     # unit + integration tests
cargo fmt      # 2-space indent (see rustfmt.toml)
cargo clippy --all-targets
```
