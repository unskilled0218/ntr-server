# Live updates

- [Origin filtering](#origin-filtering)
- [Overflow recovery](#overflow-recovery)
- [Client approval](#client-approval)

`GET /api/events` streams `event: change` with `tasks` or `note` payloads. Keep-alive comments arrive every 15 seconds. The stream has no authentication; proxies should avoid buffering.

## Origin filtering

Mutations and external storage changes signal the affected resource. HA polls complete snapshots at `HA_POLL_SECONDS`; one coordinator orders polling/writes. Only expected task results with an unchanged catalog may carry the author's origin. External catalog changes, additional task changes, and uncertainty notify the author too. Failed readbacks do not consume the baseline.

Supply the same optional `X-Ntr-Client-Id` on mutations and SSE to suppress proven own events. Missing/empty/invalid or over-128-byte IDs have no origin. This header is advisory, not authentication, authorization, or a revision. Notes keep their independent watcher.

## Overflow recovery

Each subscription buffers 16 events. Lag emits both resource tokens before resuming retained events, bypassing origin filtering even when retained events are all self-events. This is a rescan signal, not a replay log. Older clients may reload automatically.

## Client approval

Current clients detect changes and ask **Update / Not now** before applying them. Dismissal writes nothing and keeps **Review update** available. Equal-content rescans do not prompt. Reconnect detects missed changes, not automatic acceptance. Storage remains last-write-wins; see [client sync](../../app/docs/live-sync.md).
