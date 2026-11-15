# Live updates

- [Origin filtering](#origin-filtering)
- [Overflow recovery](#overflow-recovery)
- [Client approval](#client-approval)

`GET /api/events` streams Server-Sent Events without polling. Resource mutations and out-of-band edits to `todo.txt` or `Note.md` emit `event: change` with the unchanged string payload `tasks` or `note`. Keep-alive comments arrive every 15 seconds. The stream has no authentication.

## Origin filtering

HA mode polls full item snapshots at `HA_POLL_SECONDS`, including edits that leave the incomplete count unchanged. One coordinator orders ntr polls and mutations. External pre-write deltas and additional/uncertain readback deltas emit unfiltered `tasks` rescans; only proven self-only changes use origin filtering. Failed readbacks do not consume the published baseline. Recovery may issue extra safe rescans. Notes retain their independent file watcher. See [HA safety](home-assistant.md#safety-and-compatibility).

Clients may supply the same optional `X-Ntr-Client-Id` on task, raw-task, and note mutations and their SSE subscription. Successful mutations attach this advisory origin; a subscriber with the matching ID does not receive its own events. Clients without an ID retain all notifications. Missing, empty, invalid, or over-128-byte IDs have no origin. Other clients and file-watcher changes still notify normally. The ID is not authentication, authorization, or a durable revision.

## Overflow recovery

Each subscriber has a 16-event broadcast buffer. If it falls behind, the stream emits both existing `tasks` and `note` tokens before resuming retained changes. These synthetic rescans bypass origin filtering, including when a lost foreign change is followed entirely by self-originated events. The receiver and queued second token survive separate polls and repeated lag. No additional mutation or reconnect is required to trigger recovery. There is no replay log.

Legacy clients parse the same event names and payloads, but can automatically reload both resources after lag. This recovery behavior is intentional.

## Client approval

Updated clients use signals only for side-effect-free detection. A differing snapshot becomes an in-app Update / Not now question; panes stay unchanged until approval. Dismissal performs no writes and retains Review update. Equal-content rescans and self echoes do not prompt. Reconnect detects missed changes without automatically applying them. See the [client behavior](../../app/docs/live-sync.md).

Precise self-event filtering requires server `0.2.9` and client `0.15.19` or later. Older servers cannot identify authors, but updated clients still enforce approval and compare snapshots. Storage remains last-write-wins: already-issued writes and post-validation races are not transactional conflict protection.
