# my-ntr-server

- [Quick start](#quick-start)
- [Documentation](#documentation)

A single-binary Rust HTTP server for todo.txt tasks and a Markdown note. Stores data in plain files and sends live updates to connected clients.

Version 0.2.6 is prepared locally. See [release notes](docs/releases/0.2.6.md) and the [container release workflow](docs/image-release.md); preparation does not mean publication.

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
- [Docker Compose](docs/docker-compose.md)
- [Container image releases](docs/image-release.md)
- [HTTP API](docs/api.md)
- [Task IDs](docs/task-ids.md)
- [todo.txt support](docs/todo-txt.md)
- [Markdown note](docs/note.md)
- [Live updates](docs/live-updates.md)
- [Development and testing](docs/development.md)
