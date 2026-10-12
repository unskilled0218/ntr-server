# Docker Compose

Run commands from the server repository root.

`docker-compose.yml` runs the server straight from source using the official Rust image — no Dockerfile to build:

```sh
docker compose up                  # API on http://localhost:8080/api/...
NTR_PORT=9000 docker compose up     # use a different port
NTR_BIND=0.0.0.0 docker compose up  # reach it from other devices on your network
```

The server is published on `127.0.0.1` only by default, matching the app's loopback default — it is unauthenticated, so exposing it to the network (`NTR_BIND=0.0.0.0`) is a deliberate choice; do it only on a trusted network or behind an authenticating proxy.

`todo.txt`, `Note.md`, and `labels.json` live in `./data` on the host, so they persist across restarts. The server watches out-of-band edits to `todo.txt` and `Note.md` and pushes those changes to open clients. The first `up` compiles the dependencies (a few minutes); the cargo build/registry caches are kept in named volumes, so later starts are fast.
