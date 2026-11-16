# Task IDs

- [File backend](#file-backend)
- [Home Assistant backend](#home-assistant-backend)

## File backend

Ids are assigned at load time and are stable for the lifetime of the running process: deleting a task does not renumber the others, and a deleted id returns 404 rather than silently aliasing another task. Ids are not written into the todo.txt file (it stays portable to other todo.txt tools) and are reassigned on restart or on a `PUT /api/raw` bulk replace.

## Home Assistant backend

The sidecar configured by `HA_METADATA_FILE` persists numeric IDs by HA instance/list/item UID across restarts and reordering. Duplicate UIDs in different lists remain distinct. New items receive globally monotonic IDs within JavaScript's safe integer range; deleted IDs are not reused. A verified confirmed cross-list move explicitly transfers the source numeric ID to the new target UID. Retained partial-move copies have separate IDs until completion is proven. A durable operation counter prevents stale recovery confirmations from releasing another move. Back up the sidecar before its atomic schema 1-to-2 migration. Invalid maps or a different HA source fail closed rather than reassigning IDs. Whole-file replacement is disabled. See [HA setup and limits](home-assistant.md).
