# HTTP API

- [Task responses](#task-responses)
- [Routes](#routes)
- [HA list routing](#ha-list-routing)
- [Safety](#safety)

## Task responses

Tasks include `id`, completion/creation dates, `description`, `projects`, `contexts`, `tags`, and `raw`. For example:

```json
{
  "id": 1,
  "completed": false,
  "completion_date": null,
  "creation_date": "2026-06-19",
  "description": "buy milk",
  "projects": [],
  "contexts": [],
  "tags": {},
  "raw": "2026-06-19 buy milk"
}
```

HA tasks also include `list_id`, and `projects` becomes `[list_id]`. Treat `raw` as a validated task projection, not a complete HA backup. See [ID lifetime](task-ids.md).

## Routes

| Method & path | Purpose |
| --- | --- |
| `GET /api/health` | Process liveness, not upstream health. |
| `GET /api/capabilities` | `task_backend` (`file`/`homeassistant`) and `raw_task_replace`. |
| `GET /api/tasks` | List; optional `completed`, `project`, `context` filters. |
| `POST /api/tasks` | Create from `{ "text": "buy milk" }`. |
| `GET /api/tasks/{id}` | Fetch one task. |
| `PUT /api/tasks/{id}` | Replace from a `text` request. |
| `POST /api/tasks/{id}/complete` | Complete. |
| `POST /api/tasks/{id}/uncomplete` | Reopen. |
| `DELETE /api/tasks/{id}` | Delete; success is 204. |
| `GET /api/raw` | Task projection as `text/plain`. |
| `PUT /api/raw` | Bulk task replacement for supported backends; rejected in HA mode. |
| `GET /api/note` / `PUT /api/note` | Read/replace the Markdown note; successful PUT is 204. |
| `GET /api/labels` / `PUT /api/labels` | Read/replace remembered labels; successful PUT is 204. |
| `GET /api/events` | SSE `change` events with `tasks` / `note` payloads. |
| `GET /api/lists` | HA catalog and pending-move diagnostics. |
| `POST /api/lists` | Create with `{ "name": "Shared", "confirmed": true }`. |
| `PUT /api/lists/{entity_id}` | Rename with `name`, `expected_name`, and `confirmed: true`. |
| `DELETE /api/lists/{entity_id}` | Delete with `expected_name` and `confirmed: true`; default/nonempty lists are protected. |
| `POST /api/list-move/recovery` | `{ "action": "reconcile", "operation_id": 1, "confirmed": true }`; action may also be `abandon`. |

## HA list routing

Create/update accept optional `list_id`: omission uses the default for creates or preserves the update's source; empty selects the default. A changed list additionally requires `confirm_move: true`, current `expected_source`, and fresh `expected_raw`. Summary-label edits never imply moves. The `project` filter uses the list ID. Other backends reject explicit HA routing/list operations.

Catalog entries expose `id`, `name`, `is_default`, `can_rename`, and `can_delete`; the response adds `can_create` and nullable `pending_move`. Diagnostics expose operation/task/source/target IDs and whether the copy is identified, never private registry data. Echo the current operation ID during recovery; stale confirmations fail. Pending moves block both participant lists. See [management/recovery requirements](home-assistant.md#lists-and-projects).

## Safety

ntr has no authentication. HA list administration needs server opt-in plus admin rights. HA failures use 502/503, timeouts 504, and uncertain writes 409 or 504. A failure can follow a committed write: inspect actual state instead of replaying. Errors omit credentials/upstream bodies. Unknown completion history stays unknown; due values may be dates or offset-bearing datetimes.

Task/raw/note mutations and SSE may share advisory `X-Ntr-Client-Id`; it filters only proven own notifications, not authentication or external changes. Overflow and watcher rescans bypass filtering. See [events](live-updates.md).

Removed `priority` filters return HTTP 400 rather than being ignored. Edit clients must validate `raw` against completion/dates/description before reconstructing a task; inconsistent data fails without a write. Keep client/server versions aligned when exact preservation matters.
