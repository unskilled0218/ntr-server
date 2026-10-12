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

There is no authentication by design. Keep the default loopback bind, or put it behind an authenticating reverse proxy before exposing it.
