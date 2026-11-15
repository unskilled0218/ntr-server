# Home Assistant tasks

- [Setup](#setup)
- [Container deployment](#container-deployment)
- [Task mapping](#task-mapping)
- [Safety and compatibility](#safety-and-compatibility)
- [Verification](#verification)

## Setup

Clients connect to ntr server, not directly to HA. Configure one existing `todo.*` entity on the server. HA Local to-do is recommended. The adapter uses HA's standard `todo.get_items`, `todo.add_item`, `todo.update_item`, and `todo.remove_item` actions; due clearing uses `due_date: null` or `due_datetime: null`. The adapter targets these APIs as documented in HA 2026.11; validate your installed version on a disposable list before deploying.

Create a long-lived access token in your HA profile's Security page. Store it in a file readable only by the server user. Do not commit it or include it in client settings. Supply your actual URL and entity; these are examples, not discovered deployment values.

```sh
TASK_BACKEND=homeassistant \
HA_URL=https://homeassistant.lan:8123 \
HA_TOKEN_FILE=/path/to/ha-token \
HA_TODO_ENTITY=todo.personal_tasks \
HA_CA_FILE=/path/to/homeassistant.lanCA.crt \
HA_METADATA_FILE=/path/to/data/ha-metadata.json \
cargo run
```

`HA_CA_FILE` is optional with system-trusted certificates. For a private CA or self-signed certificate, supply its PEM trust certificate. Hostname verification remains enabled: the URL hostname must match the certificate. There is no insecure TLS option. Redirects, URL credentials, query parameters, and fragments are rejected. Individual HA requests time out after 10 seconds, keeping two-request reads below the client's 30-second deadline; response bodies are limited to 4 MiB.

`HA_POLL_SECONDS` defaults to 10 and accepts 1–3600. `TASK_BACKEND` defaults to `file`; HA mode never opens, watches, imports, or modifies `TODO_FILE`. Notes and remembered labels keep their existing configuration.

Client-facing HA operations wait at most 5 seconds for the coordinator and run for at most 20 seconds, returning an HTTP error before the client's 30-second transport deadline. Canceling a mutation deadline marks its outcome uncertain and forces an unfiltered recovery rescan; it never triggers an automatic retry.

## Container deployment

Add this override to the normal Compose service after selecting a verified image containing the adapter. Keep `./data` writable by container UID 1000. The secret file must be readable by that user; incompatible permissions fail rather than disabling authentication.

```yaml
services:
  ntr-server:
    environment:
      TASK_BACKEND: homeassistant
      HA_URL: https://homeassistant.lan:8123
      HA_TODO_ENTITY: todo.personal_tasks
      HA_TOKEN_FILE: /run/secrets/ha_token
      HA_CA_FILE: /run/certificates/homeassistant.lanCA.crt
      HA_METADATA_FILE: /data/ha-metadata.json
      HA_POLL_SECONDS: "10"
    secrets:
      - ha_token
    volumes:
      - ./homeassistant.lanCA.crt:/run/certificates/homeassistant.lanCA.crt:ro
secrets:
  ha_token:
    file: /path/outside/repository/ha-token
```

The container must resolve the HA hostname and reach its HTTPS port. Back up the metadata sidecar with notes and labels; it owns stable client IDs and ntr-only lifecycle dates. Do not share a sidecar between server processes or point it at another HA instance/list. Source mismatches and malformed identity maps fail closed. Ntr still has no authentication: keep the loopback port mapping or use an authenticating reverse proxy.

## Task mapping

HA owns item UID, summary, status, native due, and description. Projects, contexts, and custom tokens stay in the summary. Native due values become `due:` tokens in ntr's task DTO/raw export. HA descriptions remain untouched but are not standalone Markdown notes. Multiple lists, list creation, and automatic migration are not included.

Numeric IDs persist against the configured HA source/entity/UID, not summary or list position. Duplicate summaries have separate IDs. Deleted IDs are not assigned to different UIDs. If one item's UID matches another item's summary, ntr refuses that ambiguous HA service address; rename the conflicting item in HA first.

Creation/completion dates and hidden legacy prefixes live in the sidecar. Completed items without known completion dates expose both dates as null to satisfy the current editor/raw contract. Known creation history stays private and returns on reopen. External lifecycle transitions invalidate old completion claims. Restart or a polling outage conservatively invalidates completion dates whose continuity is unknown. An external reopen/recomplete cycle entirely between reads cannot be observed; this is not an audit log.

The date-only editor shows a datetime's date in its supplied offset and explains its retained time/offset. Unrelated edits preserve the original token. Changing the date retains its time and fixed offset; clearing the input or `Clear due` removes the whole due value. This is not named-timezone/DST rescheduling. HA can normalize submitted offsets to its timezone; readback accepts an equivalent instant, not a different deadline.

Summaries that cannot round-trip through ntr's single-line task format are rejected, not flattened or reinterpreted. This includes multiline/empty summaries, leading todo.txt lifecycle/legacy-prefix syntax, and embedded `due:` tokens conflicting with native due ownership. Correct these items in HA. Unsupported entity capabilities are rejected before writes.

## Safety and compatibility

`GET /api/capabilities` reports `{"task_backend":"homeassistant","raw_task_replace":false}`. `GET /api/raw` exports deterministic task text; `PUT /api/raw` is rejected without upstream writes. Updated clients validate capabilities before reconnect writes and always adopt HA tasks regardless of conflict strategy. Local fallback edits are never automatically uploaded to HA. Note reconciliation keeps its existing policy.

Existing task files are not imported, merged, or deleted. File-backed servers and local-only mode remain available. Older clients may fail on raw reconciliation and must be upgraded for HA mode. A capability endpoint returning 404 identifies an older file server; malformed responses or other errors never authorize a downgrade.

HA failures return 502/503 and timeouts return 504, not client-to-ntr connection failures that activate fallback. Writes can be uncertain: HA actions, readback, and sidecar writes are not transactional. Never automatically replay them; reload actual state before deciding what to do. An ambiguous create returns an error rather than guessing its UID.

One coordinator orders ntr polls and mutations. External pre-write deltas, additional readback deltas, and uncertain outcomes trigger unfiltered rescans, including for the initiating client. Only proven self-only deltas use origin filtering. HA's other writers are not locked; last-write-wins races remain possible. Client panes still require remote-update approval.

## Verification

Server tests require Rust, Python 3, and OpenSSL. They use disposable loopback HTTPS fixtures and generated test certificates/tokens, not deployment credentials or real HA. After configuration is supplied, perform an operator-approved smoke test on a disposable HA list: duplicate names, CRUD, due values, completion/reopening, external edits, restart, and unchanged Markdown notes.
