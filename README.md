# my-ntr-server

- [Quick start](#quick-start)
- [Documentation](#documentation)

A Rust HTTP server for tasks, Markdown notes, and live change notifications. Tasks can use local storage or Home Assistant.

Version 0.2.16. See [release notes](docs/releases/0.2.16.md).

## Quick start

From this directory, run `cargo run --locked`. For HA, first prepare [connection settings](docs/home-assistant.md#setup). For Docker, use the single [Compose file](docs/docker-compose.md).

The server defaults to `http://127.0.0.1:8080` and has no authentication. Keep it on loopback or behind an authenticating reverse proxy.

## Documentation

- [Configuration](docs/configuration.md), [HA lists and recovery](docs/home-assistant.md), and [Docker Compose](docs/docker-compose.md).
- [HTTP API](docs/api.md), [task IDs](docs/task-ids.md), [notes](docs/note.md), and [live updates](docs/live-updates.md).
- [Development](docs/development.md) and [container publication](docs/image-release.md).
