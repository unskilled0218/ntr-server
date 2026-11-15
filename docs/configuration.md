# Run and configuration

Run commands from the server repository root.

```sh
cargo run
```

In another terminal:

```sh
curl http://127.0.0.1:8080/api/tasks
```

Configuration (environment variables):

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
| `HA_TODO_ENTITY` | None | Required existing `todo.*` entity in HA mode. |
| `HA_CA_FILE` | System trust | Optional PEM trust certificates; hostname verification stays enabled. |
| `HA_METADATA_FILE` | `ha-metadata.json` | Persistent stable-ID/lifecycle sidecar; parent directory must be writable. |
| `HA_POLL_SECONDS` | `10` | Full-item refresh interval, from 1 to 3600 seconds. |

There is no authentication by design. Keep the default loopback bind, or put it behind an authenticating reverse proxy before exposing it.

See [HA setup and safety](home-assistant.md) for private certificates, secret-file permissions, and container mounts. Invalid HA configuration fails startup without falling back to task files.
