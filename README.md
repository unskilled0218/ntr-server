# my-ntr-server

- [Quick start](#quick-start)
- [Documentation](#documentation)

A single-binary Rust HTTP server for tasks and a Markdown note. Uses plain files by default, or Home Assistant To-do for tasks, and sends live updates to connected clients.

Version 0.2.10 adds the optional [Home Assistant task backend](docs/home-assistant.md), with server-only credentials and private-CA trust. Markdown notes stay file-backed. See [release notes](docs/releases/0.2.10.md); local preparation does not mean publication.

## Quick start

```sh
cargo run
```

In another terminal:

```sh
curl http://127.0.0.1:8080/api/tasks
```

The server listens on `127.0.0.1:8080` by default and has no authentication. Keep it on loopback or use an authenticating reverse proxy.

## Documentation

- [Run and configuration](docs/configuration.md)
- [Home Assistant task backend](docs/home-assistant.md) — server-only credentials, private-CA trust, and safe reconnect behavior; Markdown notes remain unchanged.
- [Docker Compose](docs/docker-compose.md)
- [Container image releases](docs/image-release.md)
- [HTTP API](docs/api.md)
- [Task IDs](docs/task-ids.md)
- [todo.txt support](docs/todo-txt.md)
- [Markdown note](docs/note.md)
- [Live updates](docs/live-updates.md)
- [Development and testing](docs/development.md)
