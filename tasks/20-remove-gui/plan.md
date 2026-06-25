# 20 - Remove the GUI part

## Goal

Strip the browser WebUI from `my-ntr-server`, leaving a clean JSON/HTTP API server. After
this change the binary serves only `/api/*` (and the live-update SSE stream); there is no
HTML/CSS/JS frontend served from the process and no `web/` directory in the tree.

## Scope: what "GUI part" means here

The GUI is the self-contained browser frontend the server ships and serves:

- `web/index.html`, `web/app.js`, `web/style.css` (the page, its script, its styles).
- The three routes/handlers that serve those static assets: `GET /`, `GET /static/app.js`,
  `GET /static/style.css` (`index`, `app_js`, `style_css` in `src/api.rs`).
- The documentation describing the WebUI (README `## WebUI` section and the intro/`Note`
  mentions; the doc comments that say "WebUI").

## What stays (and why it is not "GUI")

- **The whole `/api/*` surface stays**, including `GET /api/events` (SSE). The events stream
  is a documented part of the HTTP API (it is listed in the README API table), consumed over
  HTTP by *any* client, not just a browser — programs subscribe and re-`GET` the changed
  resource. Removing it would be removing an API feature, not the GUI.
- **`watch.rs` + the broadcast channel + `Change`/`notify` stay.** They power out-of-band
  file-edit detection and the `/api/events` broadcast — server-side API behavior, unrelated to
  the browser frontend. The only GUI-coupled thing about them is a doc-comment cross-reference
  to `web/app.js`, which the plan updates.
- **`CorsLayer::permissive()` stays** — it exists so other programs/origins can call the API,
  which is now the server's entire reason to exist.

## Changes

### Delete

- `web/` directory in full (`index.html`, `app.js`, `style.css`).

### `src/api.rs`

- Remove the three static-asset routes from `router`:
  `.route("/", get(index))`, `.route("/static/app.js", ...)`, `.route("/static/style.css", ...)`.
- Remove the `index`, `app_js`, and `style_css` handler functions (and their `include_str!`s,
  which are the only thing tying the binary to `web/`).
- Drop the now-unused `use axum::response::Html` import. Keep `IntoResponse`/`Response`
  (still used by `AppError` and the API handlers) and `header` (still used by
  `get_raw`/`get_note` content types).
- Doc comments: module header `//! HTTP API and WebUI routes.` -> `//! HTTP API routes.`;
  `router`'s `/// Build the router with all API and WebUI routes wired up.` ->
  `... with all API routes wired up.`; the `Change` enum comment that points at the JS
  `change` handler in `web/app.js` -> re-point it at SSE API clients generally (the wire
  contract is unchanged; only its consumer description changes).

### `src/lib.rs`

- Crate doc comment: drop "and serving a small self-contained WebUI" so it reads as a JSON
  HTTP API server over a todo.txt file.

### `src/main.rs`

- Entry-point doc comment: `... serve the API and WebUI.` -> `... serve the API.`

### `README.md`

- Intro: drop "and served through a small self-contained WebUI" wording.
- Remove the entire `## WebUI` section (page layout, editor, live-update dialog, dropdowns,
  picker limits) — all of it describes the deleted frontend. The `GET /api/events` row stays
  in the API table; the live-update *mechanism* is still real, just no longer has a browser UI
  to narrate.
- `## Note (Note.md)`: trim the sentence that says "The WebUI softens this for its own editor
  with a conflict prompt (see below) ..." since there is no WebUI/below section anymore; keep
  the statement that `PUT /api/note` is plain last-write-wins.

### `Cargo.toml`

- Version bump `0.1.0` -> `0.2.0` (per ADD_NEW_FEATURE.md "Update version"; removing a shipped
  surface is a notable, behavior-visible change — a minor bump pre-1.0).

## Tests / verification

- `tests/api.rs` already tests only `/api/*` (health, tasks, raw, note, events, broadcast); it
  never exercised `/`, `/static/app.js`, or `/static/style.css`, so no test is removed and the
  suite must stay green unchanged. `cargo test` (10 tests) is the regression gate.
- `cargo fmt --check` and `cargo clippy --all-targets` must be clean — in particular clippy's
  unused-import lint confirms the `Html` import removal is complete and nothing else became
  dead.
- Grep check: after the change, `grep -rin "webui\|web/\|/static\|index.html\|app.js\|style.css\|include_str"`
  over `src/`, `tests/`, and `README.md` returns nothing (the one remaining acceptable hit
  would be none — the `Change` comment is re-pointed away from `web/app.js`).

## Out of scope

- Removing or changing any `/api/*` endpoint, the watcher, or live-sync behavior (these are
  API, not GUI — see "What stays").
- `docker-compose.yml` / Docker docs: they run the server and hit `/api/health`; nothing there
  references the GUI, so they are unaffected.
