# HTTP API

All task responses use this JSON shape:

```json
{
  "id": 1,
  "completed": false,
  "completion_date": null,
  "creation_date": "2026-06-19",
  "description": "buy milk +groceries @store due:2026-06-30",
  "projects": ["groceries"],
  "contexts": ["store"],
  "tags": { "due": "2026-06-30" },
  "raw": "(A) 2026-06-19 buy milk +groceries @store due:2026-06-30"
}
```

| Method & path | Description |
| --- | --- |
| `GET /api/health` | Liveness check. |
| `GET /api/tasks` | List tasks. Filters: `completed`, `project`, `context`. |
| `POST /api/tasks` | Create from `{ "text": "<todo.txt line>" }`. |
| `GET /api/tasks/{id}` | Fetch one task. |
| `PUT /api/tasks/{id}` | Replace from `{ "text": "<todo.txt line>" }`. |
| `POST /api/tasks/{id}/complete` | Mark complete. |
| `POST /api/tasks/{id}/uncomplete` | Mark incomplete. |
| `DELETE /api/tasks/{id}` | Delete (204 on success). |
| `GET /api/raw` | Whole file as `text/plain`. |
| `PUT /api/raw` | Replace the whole file from a `text/plain` body. |
| `GET /api/note` | The Markdown note as `text/markdown`. |
| `PUT /api/note` | Replace the note from a text body (`204`). |
| `GET /api/labels` | Remembered projects and contexts as JSON. |
| `PUT /api/labels` | Replace the remembered labels document (`204`). |
| `GET /api/events` | Server-Sent Events stream of `change` events (`tasks` / `note`) for live updates. |

See [Task IDs](task-ids.md) for ID lifetime and [Live updates](live-updates.md) for event behavior.

Priority is absent from task DTOs and stores. Any `priority` query parameter returns HTTP 400 with `{ "error": "Priority filtering is no longer supported" }`, including a bare key, empty, invalid, repeated, or URL-encoded key. Supported filters keep their existing validation. Older clients relying on priority are unsupported.

`raw` preserves hidden leading legacy tokens; `description` excludes only that leading token and lifecycle dates. `pri:` tokens remain visible, editable description text. Complete/reopen never convert these tokens. New clients must validate raw completion state, dates, and description before reconstructing a hidden token during editing. See [todo.txt support and mixed-version limits](todo-txt.md).
