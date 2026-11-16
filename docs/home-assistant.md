# Home Assistant tasks

- [Setup](#setup)
- [Container deployment](#container-deployment)
- [Task mapping](#task-mapping)
- [Lists and projects](#lists-and-projects)
- [Move recovery](#move-recovery)
- [Metadata durability](#metadata-durability)
- [Safety and compatibility](#safety-and-compatibility)
- [Verification](#verification)

## Setup

Clients connect to ntr server, not directly to HA. Set `HA_TODO_ENTITY` to an existing default `todo.*` entity; all visible HA to-do entities become projects. HA Local To-do is recommended. The adapter uses HA's standard `todo.get_items`, `todo.add_item`, `todo.update_item`, and `todo.remove_item` actions; due clearing uses `due_date: null` or `due_datetime: null`. List management additionally uses the Local To-do config flow and entity registry. The adapter targets HA 2026.11; validate your installed version on a disposable list before deploying.

Create a long-lived access token in your HA profile's Security page. Store it in `secrets/ha-token`, readable only by the server user. Put your private CA trust certificate at `certificates/homeassistant.lanCA.crt`. Neither file is committed or sent to clients. Run from the server directory:

```sh
cp .env.example .env
mkdir -p secrets certificates
chmod 700 secrets
# Edit .env with your HA URL and entity; save the token and CA files before starting.
chmod 600 secrets/ha-token
cargo run --locked
```

The example configures `TASK_BACKEND=homeassistant`, an example HTTPS URL/entity, token path `./secrets/ha-token`, CA path `./certificates/homeassistant.lanCA.crt`, metadata path `./ha-metadata.json`, and a 10-second poll interval. Replace example deployment values. Startup loads only the working directory's `.env`; existing process variables override it, so remove stale exports when switching to file configuration. Relative paths are resolved from that working directory. Restart after edits. `.env`, its local variants, and the private setup directories are Git-ignored, while `.env.example` stays tracked.

`HA_CA_FILE` is optional with publicly trusted certificates. For a private CA or self-signed certificate, supply its PEM trust certificate. Hostname verification remains enabled for HTTPS and WSS: the URL hostname must match the certificate. There is no insecure TLS option. Redirects, URL credentials, query parameters, and fragments are rejected. Individual requests/WSS commands time out after 10 seconds; HTTP bodies and WSS messages/frames are limited to 4 MiB. Multi-list operations retain the overall deadline below.

`HA_POLL_SECONDS` defaults to 10 and accepts 1–3600. `TASK_BACKEND` defaults to `file`; HA mode never opens, watches, imports, or modifies `TODO_FILE`. Notes and remembered labels keep their existing configuration.

Client-facing HA operations wait at most 5 seconds for the coordinator and run for at most 20 seconds, returning an HTTP error before the client's 30-second transport deadline. Canceling a mutation deadline marks its outcome uncertain and forces an unfiltered recovery rescan; it never triggers an automatic retry.

## Container deployment

Select a verified image containing version 0.2.12's multi-list support; local code changes do not publish `latest`. Copy/edit `.env` and prepare the same `secrets/` and `certificates/` directories as above. Keep `./data` writable by container UID 1000. The token must be readable by that user; incompatible permissions fail rather than disabling authentication. The read-only mount source directories must already exist.

```sh
mkdir -p data secrets certificates
docker compose config --quiet
docker compose up -d
```

The single `docker-compose.yml` uses service `env_file: .env`; Compose's normal interpolation-only `.env` loading does not itself pass settings to a container. It mounts host `./secrets` and `./certificates` read-only at `/data/secrets` and `/data/certificates`, without automatically creating missing source directories. With the image working directory `/data`, example relative token/CA paths work in the container, and `./ha-metadata.json` persists on `./data:/data`. The `.env` itself is not mounted or baked into the image. Explicit service `environment` settings in a custom override take precedence over `env_file`; image defaults such as `HOST=0.0.0.0` remain unless overridden. `.env` and both private directories are required for this Compose setup; for file-backed tasks use `TASK_BACKEND=file` and empty private directories.

The container must resolve the HA hostname and reach its HTTPS/WSS port. Back up the metadata sidecar with notes and labels; it owns stable client IDs, ntr-only lifecycle dates, and pending move recovery. Do not share a sidecar between server processes or point it at another HA instance. Source mismatches and malformed identity maps fail closed. Ntr still has no authentication: keep the loopback port mapping or use an authenticating reverse proxy.

## Task mapping

HA owns item UID, summary, status, native due, and description. List membership is explicit routing metadata, separate from literal `+project` summary tokens. Existing tokens stay editable and are never automatically converted to lists. Contexts and custom tokens remain in the summary. Native due values become `due:` tokens in task DTO/raw exports. Native descriptions remain untouched during edits and are preserved during moves; they are not standalone Markdown notes.

Numeric IDs persist against the normalized HA URL, list entity ID, and item UID, not summary or position. Duplicate UIDs in different lists and duplicate summaries have separate IDs. Deleted IDs are not reused. A completed move explicitly transfers the source ID to its verified target. If one item's UID matches another item's summary in the same list, ntr refuses that ambiguous HA address; rename the conflicting item in HA first.

Creation/completion dates and hidden legacy prefixes live in the sidecar. Completed items without known completion dates expose both dates as null to satisfy the current editor/raw contract. Known creation history stays private and returns on reopen. External lifecycle transitions invalidate old completion claims. Restart or a polling outage conservatively invalidates completion dates whose continuity is unknown. An external reopen/recomplete cycle entirely between reads cannot be observed; this is not an audit log.

The date-only editor shows a datetime's date in its supplied offset and explains its retained time/offset. Unrelated edits preserve the original token. Changing the date retains its time and fixed offset; clearing the input or `Clear due` removes the whole due value. This is not named-timezone/DST rescheduling. HA can normalize submitted offsets to its timezone; readback accepts an equivalent instant, not a different deadline.

Summaries that cannot round-trip through ntr's single-line task format are rejected, not flattened or reinterpreted. This includes multiline/empty summaries, leading todo.txt lifecycle/legacy-prefix syntax, and embedded `due:` tokens conflicting with native due ownership. Correct these items in HA. Unsupported entity capabilities are rejected before writes.

## Lists and projects

Use app 0.15.32 or later. Project selectors use entity IDs internally and friendly names for display; duplicate names show the ID. Empty lists remain visible. Unselected creates use `HA_TODO_ENTITY`; edits without explicit routing keep the current list. Task DTOs include `list_id`, and their `projects` array contains that list ID, not summary-token projects. Legacy `+project` text stays in `raw` and `description`. Raw exports/local fallback files cannot preserve native list routing or descriptions and must never be used as an HA restore/upload source.

List administration is disabled by default. Set `HA_MANAGE_LISTS=true` and use an administrator-owned HA token to enable it. This extends ntr's unauthenticated API to administrative actions: enable only on a trusted connection or behind an authenticating proxy. An ordinary HA token retains supported task operations. A failed catalog read blocks routing instead of silently falling back to remembered labels. An unavailable visible HA list fails the entire task snapshot; removed entities disappear from discovery without dropping their identity tombstones.

Create project creates a Local To-do list through HA's config flow. Rename changes the entity's display name through the registry; it does not rename the entity ID, storage key, or config-entry title. Delete is available only for a non-default Local To-do list whose config entry owns exactly one entity. It requires confirmation and a fresh check of active and completed tasks. Other providers cannot be deleted through ntr. HA has no atomic empty-check-and-delete: stop HA automations and other writers during deletion. Never retry an uncertain list creation/deletion automatically; inspect actual HA state first.

## Move recovery

Choosing another list in Edit requires a separate confirmation. ntr checks the expected source and raw task revision, journals intent, copies all task fields, identifies a unique target UID, and verifies content before deleting the source. The original numeric ID transfers only after verified completion. HA's `todo/item/move` only reorders one list and is not used. HA offers neither atomic cross-list moves nor compare-and-delete; stop other writers during moves to avoid the final verification/deletion race.

Failures can leave both copies. ntr persists the journal and blocks writes to both involved lists, including after restart; an uncertain target UID never permits source deletion. Review both lists in HA. **Reconcile completed move** only repairs identity metadata when the source is absent and the known target matches. **Abandon move** clears the block without copying or deleting anything; retained copies keep distinct numeric IDs. Neither action replays HA mutations. A later move requires a new confirmation. Do not delete the sidecar to clear a pending move.

## Metadata durability

Server 0.2.13 requires Unix file and directory synchronization for HA sidecar saves. Each save writes the sibling temporary file, synchronizes the file, renames it over the destination, then synchronizes the parent directory. Schema migration, move intent, verified target identity, completion, and recovery all use this boundary. The next dependent HA write is admitted only after persistence succeeds. File-backed tasks, notes, and labels retain their existing atomic-replacement behavior.

Keep the sidecar on a persistent local filesystem that correctly implements file and directory fsync, backed by storage that honors flush requests. Unsupported platforms and sync failures return an error rather than silently weakening the guarantee. Network/virtual filesystems or hardware that ignores flushes do not provide this guarantee; validate storage semantics before enabling HA. A relative `HA_METADATA_FILE` uses the process working directory.

A failure after rename can already expose the new sidecar on disk even though directory synchronization failed. The operation still reports failure and does not advance dependent HA writes; existing pending state remains conservatively blocked in memory. Restart can find the previous or new journal transition, so inspect actual state and use explicit reconcile/abandon rather than replay. Local durability does not make HA mutations transactional or remove the external-writer races described above. Tests inject persistence errors; they do not simulate physical power loss.

## Safety and compatibility

Back up `HA_METADATA_FILE` before upgrading. A matching schema 1 single-list map migrates atomically to schema 2 before reads/writes, preserving IDs, lifecycle data, and the counter. Schema 2 binds the normalized HA URL and entity-qualified keys. Old servers cannot read it: stop ntr and restore the matching pre-upgrade backup for rollback. The sidecar is not a task backup, but its pending move journal can include summaries and native descriptions; keep it private and persistent.

`GET /api/capabilities` reports `{"task_backend":"homeassistant","raw_task_replace":false}`. `GET /api/raw` exports deterministic task text; `PUT /api/raw` is rejected without upstream writes. Updated clients validate capabilities before reconnect writes and always adopt HA tasks regardless of conflict strategy. Local fallback edits are never automatically uploaded to HA. Note reconciliation keeps its existing policy.

Existing task files are not imported, merged, or deleted. File-backed servers and local-only mode remain available. Older clients may fail on raw reconciliation and must be upgraded for HA mode. A capability endpoint returning 404 identifies an older file server; malformed responses or other errors never authorize a downgrade.

HA failures return 502/503 and timeouts return 504, not client-to-ntr connection failures that activate fallback. Writes can be uncertain: HA actions, readback, and sidecar writes are not transactional. Never automatically replay them; reload actual state before deciding what to do. An ambiguous create returns an error rather than guessing its UID.

One coordinator orders ntr polls and mutations. External pre-write deltas, additional readback deltas, and uncertain outcomes trigger unfiltered rescans, including for the initiating client. A self-only result requires both the expected task snapshot and an unchanged catalog; concurrent empty-list creation, rename, or removal cannot inherit the author's filtered origin. HA's other writers are not locked; last-write-wins races remain possible. Client panes still require remote-update approval.

## Verification

Server tests require Rust, Python 3, and OpenSSL. They use disposable loopback HTTPS fixtures and generated test certificates/tokens, not deployment credentials or real HA. After configuration is supplied, perform an operator-approved smoke test on a disposable HA list: duplicate names, CRUD, due values, completion/reopening, external edits, restart, and unchanged Markdown notes.
