# 19 - docker-compose.yml (no Dockerfile)

## Goal

Add a `docker-compose.yml` that runs `my-ntr-server` with one `docker compose up`, **without a
Dockerfile**. Persist the todo/note files on the host so they survive restarts and can be
edited out of band (which the server watches and pushes to clients live, tasks 16-18).

## Why this shape

- **No Dockerfile -> the official `rust` image builds from the mounted source.** Without a
  Dockerfile we cannot bake an image, so the container uses `image: rust:1-bookworm` (full,
  not `-slim`, so the C linker `cargo` needs for the final link step is present) with the
  project bind-mounted and `command: cargo run --release`. Compose is then the single source
  of "how to run this", which is the whole point of the task.
- **`HOST=0.0.0.0`, not the app default.** The server defaults to `127.0.0.1` (container-local,
  unreachable through a published port). The compose sets `HOST=0.0.0.0` so the published port
  actually reaches it — the one non-obvious setting, called out in a comment.
- **The port lives in one place.** `${NTR_PORT:-8080}` is used for the app's `PORT` *and* both
  sides of the `ports` mapping, so the port is defined once (override with `NTR_PORT=...`)
  instead of the literal `8080` drifting across three spots.
- **Data on the host (`./data`), build caches in named volumes.** `todo.txt` and `Note.md` live
  in `./data` (bind mount) so they are editable from the host and persist — the natural home
  given the file-watch/live-sync features. The cargo *target* and *registry* go in named
  volumes mounted over `/usr/src/app/target` and `/usr/local/cargo/registry`, so (a) container
  builds never collide with the host `target/` (different toolchain/platform) and (b) crate
  downloads and compiled artifacts are cached across restarts, so only the first `up` is slow.

## The file (docker-compose.yml)

```yaml
# Run my-ntr-server with no Dockerfile: the official Rust image builds and runs the crate
# from the mounted source. todo.txt and Note.md live in ./data on the host so they persist
# and can be edited outside the container (the server watches them and updates clients live).
# First `up` compiles the dependency tree (a few minutes); the cargo caches below make every
# later start fast.
services:
  my-ntr-server:
    image: rust:1-bookworm
    working_dir: /usr/src/app
    command: cargo run --release
    environment:
      # The app defaults to 127.0.0.1, which a published port cannot reach; bind all
      # interfaces inside the container instead.
      HOST: 0.0.0.0
      PORT: ${NTR_PORT:-8080}
      TODO_FILE: /data/todo.txt
      NOTE_FILE: /data/Note.md
      TODO_ADD_CREATION_DATE: "true"
    ports:
      - "${NTR_PORT:-8080}:${NTR_PORT:-8080}"
    volumes:
      - ./:/usr/src/app                            # source, so cargo can build (no Dockerfile)
      - ./data:/data                               # todo.txt + Note.md, editable from the host
      - ntr-cargo-target:/usr/src/app/target       # build cache, isolated from the host target/
      - ntr-cargo-registry:/usr/local/cargo/registry  # crate download cache
    healthcheck:
      # /api/health is the app's liveness endpoint; the rust image (buildpack-deps base) ships curl.
      test: ["CMD", "curl", "-fsS", "http://localhost:${NTR_PORT:-8080}/api/health"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 300s   # generous: covers the first from-source compile
    restart: unless-stopped

volumes:
  ntr-cargo-target:
  ntr-cargo-registry:
```

## Supporting changes

- **`.gitignore`**: add `/data` (host runtime data, alongside the already-ignored `/Note.md`
  and `/todo.txt`).
- **`.gitkeep`**: commit `data/.gitkeep` so the bind-mount source directory exists in a fresh
  clone (otherwise Docker creates it root-owned on first run). The `/data` ignore uses an
  exception so the placeholder is tracked.
- **README**: a short "Run with Docker Compose" subsection — `docker compose up`, the
  `NTR_PORT` override, and where data lives (`./data`).

## Tests / verification

- No application code changes, so `cargo test` / `fmt` / `clippy` stay green (sanity only).
- `docker compose` is not available in this environment (we are already inside a container
  with no Docker daemon), so the compose file cannot be launched here. Validate what is
  possible:
  - YAML parses (python `yaml.safe_load`).
  - Structural review against the compose schema by inspection (service, image, command,
    env, ports, volumes, healthcheck keys).
  - Confirm the env keys match exactly what `Config::from_env` reads (`TODO_FILE`,
    `NOTE_FILE`, `HOST`, `PORT`, `TODO_ADD_CREATION_DATE`).
- Note for the user: run `docker compose config` then `docker compose up` on a host with
  Docker to do the real end-to-end check.

## Out of scope

- A production multi-stage Dockerfile / slim runtime image (explicitly excluded: "no
  Dockerfile"). This compose favors zero-build-file convenience over image size.
- TLS / auth / reverse proxy: the server is userless and loopback-by-default by design;
  exposing it beyond localhost is the operator's decision (unchanged from today).
