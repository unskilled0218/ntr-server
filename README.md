# my-ntr-server

- [Quick start](#quick-start)
- [Documentation](#documentation)

A single-binary Rust HTTP server for tasks and a Markdown note. Uses plain files by default, or Home Assistant To-do for tasks, and sends live updates to connected clients.

Version 0.2.14 refreshes Rust dependencies, including HTTP middleware, reqwest, and the WSS transport. HA HTTPS/WSS retain shared explicit CA roots, hostname verification, bounded operations, and durable recovery. See [HA durability requirements](docs/home-assistant.md#metadata-durability) and [release notes](docs/releases/0.2.14.md); local preparation does not mean publication.

## Quick start

```sh
cargo run
```

For Home Assistant tasks, copy `.env.example` to `.env`, edit it, and provide the token/certificate files described in [HA setup](docs/home-assistant.md#setup). Existing process variables override `.env`. Real `.env` files and private setup directories are Git-ignored.

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
