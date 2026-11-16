# HTTP API

- [Task responses](#task-responses)
- [Routes](#routes)
- [HA list routing](#ha-list-routing)
- [Safety](#safety)

## Task responses

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

## Routes

| Method & path | Description |
| --- | --- |
| `GET /api/health` | Liveness check. |
| `GET /api/capabilities` | Non-secret `task_backend` (`file`/`homeassistant`) and `raw_task_replace` boolean. |
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
| `GET /api/lists` | HA catalog: `lists` with `id`, `name`, `is_default`, `can_rename`, `can_delete`; global `can_create` and nullable `pending_move` diagnostics. |
| `POST /api/lists` | Create Local To-do list with `{ "name": "Shared", "confirmed": true }`; admin opt-in required. |
| `PUT /api/lists/{entity_id}` | Rename display with `{ "name": "New", "expected_name": "Old", "confirmed": true }`. |
| `DELETE /api/lists/{entity_id}` | Delete verified empty Local To-do list with `{ "expected_name": "Old", "confirmed": true }`; default protected. |
| `POST /api/list-move/recovery` | Explicit metadata-only `{ "action": "reconcile" or "abandon", "operation_id": 1, "confirmed": true }`; requires the current journal ID and never replays HA writes. |

See [Task IDs](task-ids.md) for ID lifetime and [Live updates](live-updates.md) for event behavior.

## HA list routing

In HA mode, task DTOs also contain `list_id`; `projects` is `[list_id]`. Description/raw keep literal `+tokens` unchanged. `project` filtering uses the list entity ID. Create/update bodies accept optional `list_id`: omission creates in the default or preserves the update's source; an empty value selects the configured default. Different-list updates additionally require `confirm_move: true`, `expected_source` equal to the current entity ID, and `expected_raw` equal to the fresh raw projection. Token-only changes never imply moves. File-backed servers reject explicit HA routing and list operations; their DTOs omit `list_id`.

List mutation responses return the refreshed catalog. Pending diagnostics expose only the monotonic `operation_id`, `task_id`, source/target list IDs, and whether the target copy was identified, never credentials or private registry/config-entry payloads. Recovery must echo the current operation ID so a stale confirmation cannot clear a newer move. Only one move journal can be pending at a time. While pending, both involved lists reject ordinary writes. See [management and recovery](home-assistant.md#lists-and-projects) for non-atomic HA limits and required exclusive writer access.

## Safety

`/api/raw` exports task text without list/native-description restore fidelity; replacing it in HA mode returns 400 before writes. Unknown external completion dates project both lifecycle dates as null without erasing private creation history. Native due values can be dates or offset-bearing datetimes. See [HA mapping](home-assistant.md).

HA failures use 502/503, timeouts use 504, and uncertain writes use 409 (or 504 for timed-out actions). Upstream writes can commit before readback/metadata failure; reload before another mutation instead of replaying automatically. Error responses omit tokens and upstream bodies. `/api/health` attests process liveness, not HA availability.

Task, raw-task, and note mutations accept an optional `X-Ntr-Client-Id` header. Supplying the same ID to `GET /api/events` filters only that subscriber's own mutation notifications. The header is advisory, not authentication. Watcher events and lag-triggered `tasks` / `note` rescans bypass origin filtering. Existing event names and string payloads remain compatible; updated clients detect and ask before applying changes.

Priority is absent from task DTOs and stores. Any `priority` query parameter returns HTTP 400 with `{ "error": "Priority filtering is no longer supported" }`, including a bare key, empty, invalid, repeated, or URL-encoded key. Supported filters keep their existing validation. Older clients relying on priority are unsupported.

`raw` preserves hidden leading legacy tokens; `description` excludes only that leading token and lifecycle dates. `pri:` tokens remain visible, editable description text. Complete/reopen never convert these tokens. New clients must validate raw completion state, dates, and description before reconstructing a hidden token during editing. See [todo.txt support and mixed-version limits](todo-txt.md).
