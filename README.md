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
| `NOTE_FILE`              | `Note.md`     | Path to the free-form Markdown note file.          |
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
last-write-wins, the same as `PUT /api/raw` for the task file. The WebUI softens this for
its own editor with a conflict prompt (see below), but the HTTP endpoint itself stays a
plain overwrite.

## WebUI

The page at `/` is split into two panes: the **note** and the **todo**. The page fills the
viewport without scrolling — each pane keeps its chrome fixed (the todo add and filter
controls and the note status bar stay put) while the task list and the note editor scroll
internally. On wide screens the two panes sit side by side. On narrow screens the app shows
one pane at a time: a **bottom navigation bar** (Note / Todo) switches between them, and a
horizontal **swipe** does the same — each view fills the screen above the bar. The note pane is a
single **in-place live editor**: one surface where you type raw Markdown over `Note.md` and
it renders as you type — headings grow, `**bold**` shows bold, `-` lists indent — while the
syntax markers stay visible (dimmed) so the text is still raw Markdown you can edit. The
client-side renderer supports a practical subset — headings, bold/italic, inline and fenced
code, ordered and unordered lists (one level), blockquotes, horizontal rules, and links;
unsupported syntax (tables, images, nested lists, raw HTML) shows as literal text. The
rendering is purely a view: the stored file stays raw Markdown (the editor's text is exactly
what is saved).

The editor **auto-saves**: there is no Save button. Edits are written shortly after you
stop typing (a short debounce), immediately when you leave the field, and best-effort
when the page closes, so the note keeps itself saved. A status line tracks state —
`Editing…`, `Saving…`, `Saved`, or a red load/save error (a failed save is retried on the
next edit).

**Live updates.** Every open page subscribes to `GET /api/events`, so a change made on one
device (or to `todo.txt` / `Note.md` directly by another program) shows up on the others
without a reload — the task list re-fetches, and an idle note editor adopts the new text.
If the note changed elsewhere **while you have unsaved edits**, the editor cannot silently
pick a winner, so it asks: a dialog shows your unsaved version next to the incoming one and
you choose **Keep mine** (save your text over theirs) or **Use theirs** (discard your edits).
Dismissing the dialog keeps your unsaved work. When there are no unsaved edits, saving is
still last-write-wins, the same as `PUT /api/note`.

The todo pane lists, filters, and edits tasks. The add form has a free-text
description plus four native dropdown/picker controls beneath it, all styled
alike — a priority dropdown (`A`–`C`), a due-date picker (written as a `due:`
tag), and single-select `+project` and `@context` dropdowns — so common fields
can be set by picking options instead of typing todo.txt syntax. The project and
context dropdowns are populated from the labels already present in the file (a
new label appears as an option once a task using it is saved; todo.txt has no
separate label store), and each shows only its `None` (no-selection) option and
"add" entry until the file contains at least one label of that kind.

Each project/context dropdown begins with an `+ Add project...` / `+ Add
context...` entry; choosing it opens a small modal with a single text input for a
new label name. A valid name is staged client-side and selected immediately so it
can be used on the next task add — it becomes a permanent label the moment a task
using it is saved (todo.txt has no place to persist a label that no task uses, so
a staged-but-unused name simply disappears on reload).

Beneath the add form a row of dropdowns filters and sorts the list. A **Status**
dropdown (`All` / `Active` / `Completed`) filters by completion, and **Project**
and **Context** dropdowns — populated from the labels already in the file, each
with an `All projects` / `All contexts` option — narrow the list to one label
(both combine with AND). These three drive the server's `completed`, `project`,
and `context` query params. A **Sort** dropdown reorders the list in the browser
(no server round-trip): `File order` (the default, the server's order),
`Priority`, `Due date`, `Created`, and `A-Z`; every key sorts ascending, sends
tasks missing that field to the bottom, and breaks ties by file order.

Two picker limits are UI-only and do not restrict the data model. The priority
picker offers `A`–`C`, and the project/context dropdowns set at most one label
each. Tasks created or edited as raw todo.txt (via the edit prompt or
`PUT /api/raw`) may carry any `(A)`–`(Z)` priority and any number of `+projects`
or `@contexts`; all of them are parsed, round-tripped, and displayed as-is.

## Development

```sh
cargo test     # unit + integration tests
cargo fmt      # 2-space indent (see rustfmt.toml)
cargo clippy --all-targets
```
