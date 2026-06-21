# Brooks-Lint Review — code (live-sync vs main)

## Score 94/100

- 🟡 Coverage Illusion: the core new behavior (a mutation broadcasts a `change`
  event) had no automated test — only `events_stream_opens` (route-opens) plus
  manual e2e. This is the exact failure mode (poller-only emitting nothing for API
  writes) that manual testing already caught once.
  Remedy: integration test subscribing to state.tx, asserting POST -> Change::Tasks,
  PUT note -> Change::Note.
- 🟢 Knowledge Duplication: "reload, notify on change, log on error" written twice in
  watch.rs reload_and_notify (per resource). Remedy: ReloadFromDisk trait + generic
  poll() helper.

Fix plan: review-plan.md (re-reviewed 100/100). Implementing.
