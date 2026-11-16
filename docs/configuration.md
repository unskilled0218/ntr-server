# Run and configuration

- [Run](#run)
- [Dotenv file](#dotenv-file)
- [Environment variables](#environment-variables)
- [Security](#security)

## Run

Run commands from the server repository root.

```sh
cargo run
```

In another terminal:

```sh
curl http://127.0.0.1:8080/api/tasks
```

## Dotenv file

Startup reads the optional `.env` file in the process working directory. It does not search parent directories, change directories, or export file values into the process environment. Existing process variables win, including explicitly empty values. Missing keys use `.env` values, then the defaults below; duplicate file keys keep their first value. `Config::from_env()` remains environment-only for library callers.

For HA setup, run `cp .env.example .env`, edit the URL/entity/path settings, provide the protected token and CA files, then run `cargo run --locked`. See [HA setup](home-assistant.md#setup). Other existing server variables can also be set in `.env`. No file means existing environment-only startup is unchanged. Restart to apply edits.

Paths are relative to the working directory, not the executable's directory. Quote values containing spaces. Use literal values as in the example rather than shell commands; `.env` is parsed, not executed. Malformed, non-UTF-8, or unreadable files stop startup before stores open, with a sanitized error that does not print the file contents. Never put the access token itself in `.env`; use `HA_TOKEN_FILE`.

## Environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `TODO_FILE` | `todo.txt` | Path to the todo.txt file. |
| `NOTE_FILE` | `Note.md` | Path to the free-form Markdown note file. |
| `LABELS_FILE` | `labels.json` | Path to the remembered project/context labels. |
| `HOST` | `127.0.0.1` | Bind address. |
| `PORT` | `8080` | Bind port. |
| `TODO_ADD_CREATION_DATE` | `true` | Stamp a creation date on new incomplete tasks. |
| `TASK_BACKEND` | `file` | `file` or `homeassistant`; HA mode does not open `TODO_FILE`. |
| `HA_URL` | None | Required HTTPS base URL in HA mode, without credentials/query/fragment. |
| `HA_TOKEN_FILE` | None | Required server-only long-lived access-token file. |
| `HA_TODO_ENTITY` | None | Required existing default `todo.*` entity; all visible HA lists are projects. |
| `HA_MANAGE_LISTS` | `false` | Explicit `true` enables admin-token list creation, display renaming, and empty-only deletion; secure the unauthenticated ntr API first. |
| `HA_CA_FILE` | System trust | Optional PEM trust certificates; hostname verification stays enabled. |
| `HA_METADATA_FILE` | `ha-metadata.json` | Persistent stable-ID/lifecycle sidecar; parent directory must be writable. |
| `HA_POLL_SECONDS` | `10` | Full-item refresh interval, from 1 to 3600 seconds. |

## Security

Real `.env` files, `.env.*` local variants, `secrets/`, and `certificates/` are Git-ignored; `.env.example` remains tracked. Ignore rules are not access control: keep local configuration private and the token file readable only by the server user. There is no ntr API authentication by design. Keep the default loopback bind, or put it behind an authenticating reverse proxy before exposing it.

See [HA setup and safety](home-assistant.md) for private certificates, secret-file permissions, and container mounts. Invalid HA configuration fails startup without falling back to task files.
