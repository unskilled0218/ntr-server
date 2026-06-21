# 19 - docker-compose.yml (no Dockerfile) — implemented

`docker compose up` runs `my-ntr-server` from source using the official Rust image, with no
Dockerfile. Data persists on the host and the server is reachable from the host.

## Files

- `docker-compose.yml` (new): one `my-ntr-server` service.
  - `image: rust:1-bookworm` (full image — has the linker cargo needs), `working_dir:
    /usr/src/app`, `command: cargo run --release`.
  - `environment`: `HOST=0.0.0.0` (the one non-obvious override — the app defaults to
    127.0.0.1, unreachable through a published port), `PORT/ports` driven by
    `${NTR_PORT:-8080}` (one source for the port), `TODO_FILE=/data/todo.txt`,
    `NOTE_FILE=/data/Note.md`, `TODO_ADD_CREATION_DATE=true`. Keys verified to match
    `Config::from_env` exactly.
  - `volumes`: `./` -> source (so cargo can build), `./data` -> `/data` (todo.txt + Note.md,
    host-editable / persistent), plus named volumes `ntr-cargo-target` and
    `ntr-cargo-registry` so container builds never touch the host `target/` and crate
    downloads/artifacts are cached across restarts.
  - `healthcheck`: `curl -fsS /api/health` (curl ships in the rust image's buildpack-deps
    base), `start_period: 300s` to cover the first from-source compile.
  - `restart: unless-stopped`.
- `.gitignore`: ignore `/data/*` but keep `!/data/.gitkeep`.
- `data/.gitkeep` (new): so the bind-mount source dir exists in a fresh clone.
- `README.md`: "With Docker Compose" subsection (`docker compose up`, `NTR_PORT` override,
  where data lives, first-build note).

## Verification

- Docker is not available in this environment (already inside a container, no daemon) and
  pyyaml/pip are absent, so the compose file could not be launched or schema-validated here.
  Validated what is possible:
  - No tab characters, no trailing whitespace, all indents are multiples of two.
  - All required keys present (service, image, command, env, ports, volumes, healthcheck,
    named volumes).
  - Env var names match `Config::from_env` (`HOST`, `PORT`, `TODO_FILE`, `NOTE_FILE`,
    `TODO_ADD_CREATION_DATE`).
- `cargo test` (30) / `cargo fmt --check` still green (no application code changed).

Real end-to-end (left to the user, on a host with Docker): `docker compose config` then
`docker compose up`, open `http://localhost:8080`, confirm `./data/todo.txt` + `Note.md`
are created and live-update works.
