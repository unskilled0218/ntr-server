# 20 - Remove the GUI part — implemented

The browser WebUI is gone; `my-ntr-server` is now an API-only server. It serves `/api/*`
only — there is no `web/` directory, no static-asset routes, and no HTML/CSS/JS in the binary.
The live-update SSE stream and the file watcher are kept (they are API, not GUI).

## Files

- `web/` (deleted): `index.html`, `app.js`, `style.css` removed via `git rm`. These were the
  only things the binary pulled in with `include_str!`.
- `src/api.rs`:
  - Removed the three static-asset routes (`GET /`, `GET /static/app.js`,
    `GET /static/style.css`) and their handlers (`index`, `app_js`, `style_css`).
  - Dropped the now-unused `use axum::response::Html` import. Kept `IntoResponse`/`Response`
    (still used by `AppError` and the API handlers) and `header` (still used by
    `get_raw`/`get_note`).
  - Doc comments: `//! HTTP API routes.`, `router` "all API routes wired up", and the `Change`
    enum comment re-pointed from `web/app.js` to SSE clients generally (wire contract
    unchanged).
- `src/lib.rs`: crate doc no longer mentions "serving a small self-contained WebUI".
- `src/main.rs`: entry-point doc now says "serve the API".
- `Cargo.toml`: version `0.1.0` -> `0.2.0`.
- `README.md`: dropped the WebUI wording in the intro; replaced the whole `## WebUI` section
  with a concise `## Live updates` section describing `GET /api/events` in API terms; updated
  the `Run` and Docker examples from "open <url>" to API/curl usage; trimmed the `Note` section
  sentence that referenced the WebUI conflict prompt.

## What stayed (API, not GUI)

- All `/api/*` endpoints, including `GET /api/events` (SSE).
- `watch.rs`, the broadcast channel, and `Change`/`notify` (out-of-band file-edit detection +
  event broadcast).
- `CorsLayer::permissive()` (the API is now the server's only surface).

## Verification

- `cargo fmt --check`: clean.
- `cargo clippy --all-targets`: no warnings (confirms the `Html` import removal left nothing
  dead).
- `cargo test`: 10 integration tests pass unchanged — they only ever exercised `/api/*`, so
  no test referenced a removed route.
- Grep over `src/`, `tests/`, `README.md` for `webui|web/|/static|index.html|app.js|style.css|include_str`:
  no matches.
