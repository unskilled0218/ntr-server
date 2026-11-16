# Docker Compose

- [Start](#start)
- [Configuration](#configuration)
- [Home Assistant configuration](#home-assistant-configuration)
- [Upgrade and rollback](#upgrade-and-rollback)

## Start

From the server directory, prepare `.env`: copy/edit `.env.example` for HA, or set `TASK_BACKEND=file` for local task storage. Create `data/`, `secrets/`, and `certificates/`; private directories are required even when empty in file mode.

```sh
mkdir -p data secrets certificates
docker compose config --quiet
docker compose pull
docker compose up -d
docker compose ps
curl -fsS http://127.0.0.1:8080/api/health
```

These commands start the service. If the image is private, first authenticate using `docker login git.lan --username your-login --password-stdin < /path/to/token`. The token needs package-read permission; never place its value in arguments.

## Configuration

One `docker-compose.yml` injects `.env` and selects `git.lan/ntr/ntr-server:latest`. Docker chooses amd64/arm64 automatically. The runtime user is `1000:1000`; `./data:/data` must be writable by that user. Compose does not repair ownership. Notes, labels, and HA metadata persist there.

The host port is loopback-only: `127.0.0.1:8080:8080`. The image binds internally to `0.0.0.0:8080`. ntr has no authentication; exposing it requires trusted access or an authenticating proxy.

Missing `.env` fails validation. The private mounts are read-only and missing source directories are not auto-created. Shell exports alone do not inject service settings; explicit service `environment` entries override `env_file`. Run `config --quiet` to avoid printing settings.

## Home Assistant configuration

Prepare the protected token and optional private CA using [HA deployment paths](home-assistant.md#container-deployment). The working directory is `/data`, so example relative paths work inside the container. HA tasks are not imported from the old local store.

## Upgrade and rollback

Back up `./data` and private configuration first. `latest` is mutable; for reproducibility, set `image` to `git.lan/ntr/ntr-server@sha256:<verified-index-digest>` before pull/start. An index digest retains automatic architecture selection.

Rollback selects a previously verified digest while keeping the same data mount/user. Check [metadata compatibility](home-assistant.md#safety-and-compatibility) before rolling back an HA server. Never delete data, rewrite release records, or roll back registry aliases as part of a container upgrade. Local preparation does not [publish an image](image-release.md).
