# Run and configuration

- [Run](#run)
- [Dotenv file](#dotenv-file)
- [Environment variables](#environment-variables)
- [Security](#security)

## Run

From the server directory, run `cargo run --locked`. Check process liveness with `curl -fsS http://127.0.0.1:8080/api/health`. For HA, follow [setup](home-assistant.md#setup); for containers, use [Compose](docker-compose.md).

## Dotenv file

Direct startup reads optional `.env` only in the working directory. Existing process values win, even when empty; duplicate file keys keep their first value. Paths are relative to that directory. No file means environment-only startup. Restart after changes.

Use quoted literal values, not shell commands. Invalid, unreadable, or non-UTF-8 files fail before stores open, without printing contents. Store the HA token in a protected file, never inside `.env`. Compose separately requires `.env` and injects it through `env_file`.

## Environment variables

| Setting | Default / purpose |
| --- | --- |
| `HOST`, `PORT` | Direct startup: `127.0.0.1`, `8080`; the image binds `0.0.0.0` internally. |
| `TASK_BACKEND` | `file`; choose `homeassistant` for HA tasks. |
| `NOTE_FILE`, `LABELS_FILE` | `Note.md`, `labels.json`; independent note and remembered-label stores. |
| `HA_URL` | Required HTTPS base URL without credentials/query/fragment. |
| `HA_TOKEN_FILE` | Required protected server-side token file. |
| `HA_TODO_ENTITY` | Required default `todo.*` list; all visible lists are discovered. |
| `HA_MANAGE_LISTS` | `false`; explicit `true` plus admin rights enables list administration. |
| `HA_CA_FILE` | Optional private CA PEM; otherwise built-in web-PKI trust. Hostnames are always verified. |
| `HA_METADATA_FILE` | `ha-metadata.json`; persistent private identity/recovery sidecar. |
| `HA_POLL_SECONDS` | `10`; valid range 1–3600 seconds. |

## Security

`.env` variants and private `secrets/` / `certificates/` directories are ignored by Git, not access-controlled by it. Restrict token files to the server user. Secure the unauthenticated API before enabling administrative privileges. Invalid HA configuration fails rather than selecting another backend. See [HA safety](home-assistant.md#safety-and-compatibility).
