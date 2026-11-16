# Home Assistant tasks

- [Setup](#setup)
- [Container deployment](#container-deployment)
- [Task mapping](#task-mapping)
- [Lists](#lists)
- [Move recovery](#move-recovery)
- [Metadata durability](#metadata-durability)
- [Safety and compatibility](#safety-and-compatibility)
- [Verification](#verification)

## Setup

Clients connect to ntr, not HA. Keep the HA URL, token, CA, and metadata on the server. The adapter targets HA 2026.11; validate your installed version on a disposable list before production.

From the server directory, copy `.env.example` to `.env`. Set `TASK_BACKEND=homeassistant`, your HTTPS `HA_URL`, and default `HA_TODO_ENTITY`. Save a long-lived token to `secrets/ha-token`, readable only by the server user. For private trust, save the CA PEM at `certificates/homeassistant.lanCA.crt`. Existing process variables override `.env`; remove stale exports before switching settings.

```sh
mkdir -p secrets certificates
chmod 700 secrets
# Save token/CA and edit .env before continuing.
chmod 600 secrets/ha-token
cargo run --locked
```

`HA_CA_FILE` is optional with built-in public trust. HTTPS and WSS verify hostnames; there is no insecure option. Redirects and credential/query/fragment URLs are rejected. Requests/WSS commands have 10-second deadlines and 4 MiB response/message limits. Client-facing operations wait at most five seconds for the coordinator and run at most 20 seconds. Timeouts can mean a write committed; never retry automatically.

## Container deployment

Use the single [Compose setup](docker-compose.md). It requires `.env`, `data/`, `secrets/`, and `certificates/`. The private directories mount read-only at `/data/secrets` and `/data/certificates`; missing sources are refused. Ensure UID 1000 can read the token and write `data/`.

Service `env_file` injects `.env`; the file is not mounted or baked into the image. Relative paths resolve under `/data`, and `./ha-metadata.json` persists on `./data:/data`. Explicit service environment values win. Select an image containing the required support; local commits do not update published images.

## Task mapping

HA owns list entity/item UID, summary, status, native due, and description. The sidecar owns stable numeric IDs and ntr-only lifecycle history. Labels in summary text are separate from list routing. HA descriptions are not ntr Markdown notes. Only representable single-line summaries are accepted; conflicting reserved metadata is rejected rather than reinterpreted.

Unknown external completion dates expose unknown lifecycle dates; retained private creation history returns on reopen. Restart/outages/external transitions can invalidate completion continuity. This is not an audit log. Datetime edits retain time/fixed offset; HA may normalize an equivalent instant. Clear removes the whole deadline, not only its date.

<a id="lists-and-projects"></a>

## Lists

All visible HA lists appear in selectors, including empty lists. Entity IDs define identity; duplicate names remain distinct. Unselected creates use the configured default, and updates without routing preserve the current list. A failed/unavailable visible list blocks the whole snapshot rather than appearing empty. Removed entities leave identity tombstones.

Administration defaults off. Enable `HA_MANAGE_LISTS=true` only with an HA admin token and trusted ntr access or an authenticating proxy. It exposes administrative privileges through an otherwise unauthenticated API. Other tokens/providers retain supported task operations without destructive capabilities.

Create uses HA Local To-do config flow; rename changes the entity display name, not its ID. Delete requires confirmation, one unambiguous Local To-do config entry owning exactly one entity, and a fresh active/completed emptiness check. The default cannot be deleted. HA has no atomic empty-check/delete: stop other writers, and inspect uncertain results instead of retrying blindly.

## Move recovery

Changing lists requires explicit confirmation of the current source/revision. ntr journals intent, copies native fields, identifies a unique target UID, verifies content, then deletes the source. It transfers the numeric ID only after proven completion. HA offers no cross-list transaction or compare-and-delete; stop other writers during moves.

Failures can leave both copies. One journal may be pending at a time; writes to both involved lists remain blocked, including after restart. **Reconcile** repairs identity only when the source is absent and the known target matches. **Abandon** clears the block without copying/deleting; retained copies keep separate IDs. Neither replays HA mutations. A later move needs new confirmation. Never delete the sidecar to clear a block.

## Metadata durability

HA saves require Unix file/directory synchronization. Every sidecar transition, including migration and recovery, writes/syncs the temporary file, renames it, then syncs the parent directory before dependent HA writes. Other stores keep their existing atomic replacement behavior.

Use persistent storage that honors file/directory fsync. Unsupported platforms or sync failures return errors; hardware/network/virtual storage that ignores flushes cannot provide the guarantee. A failure after rename may expose new disk state while memory remains conservatively blocked. Restart can recover the old or new transition; inspect and explicitly reconcile/abandon instead of replaying writes. This is local durability, not an HA transaction or proof against physical power loss.

## Safety and compatibility

Back up `HA_METADATA_FILE` before upgrading. Matching schema 1 metadata migrates atomically to schema 2 with IDs/counters/history retained. Schema 2 binds the normalized HA URL and entity-qualified keys; corrupt/mismatched maps fail closed. Older servers need a matching pre-upgrade backup for rollback. Do not share the sidecar between server processes. Journals can contain task summaries/descriptions, so keep them private.

Raw exports and local fallback copies lack list/native-description restore fidelity. HA raw replacement is rejected. Reconnect always adopts authoritative HA tasks; local edits never upload automatically. Notes retain their independent reconciliation policy. No existing task data is automatically imported, merged, or deleted.

One coordinator orders ntr reads/polls/writes. External task or catalog deltas and uncertain outcomes notify every client, including the author; only proven self-only results are filtered. Clients still require approval before adopting changed task content. See [API errors](api.md#safety) and [client connections](../../app/docs/server-connection.md).

## Verification

Run [development checks](development.md). Fixtures use generated-CA loopback HTTPS/WSS, not deployment credentials. After separate approval, validate CRUD, multiple lists, external edits, due values, restart, moves/recovery, and unchanged notes against a disposable real HA list. Fixture success is not live compatibility approval.
