# Docker Compose

- [Start](#start)
- [Configuration](#configuration)
- [Home Assistant dotenv override](#home-assistant-dotenv-override)
- [Upgrade and rollback](#upgrade-and-rollback)

Run commands from the server repository root. Compose selects `git.lan/ntr/ntr-server:latest`, a mutable shared amd64/arm64 image; Docker selects the host's architecture. No source mount or Rust compiler is needed. A separately approved first publication must establish this alias through the [image release workflow](image-release.md); local preparation is not publication. For reproducible deployments, replace it with a verified immutable index digest.

## Start

If the package is private, authenticate with a Forgejo username and token allowed to read it. Public packages may be pulled without login:

```bash
docker login git.lan --username your-login --password-stdin < "$HOME/.config/token/git"
```

Ensure the data directory exists before starting and is writable by the container's fixed UID/GID `1000:1000`. The server's atomic writes need directory access, not merely write access to an existing file. Resolve incompatible permissions with the data owner before deployment; Compose does not change host file ownership:

```sh
mkdir -p data
docker compose config
docker compose pull
docker compose up -d
docker compose ps
curl -fsS http://127.0.0.1:8080/api/health
```

Existing files created by the previous source-build container may need permission preparation for UID/GID `1000:1000`.

Tasks, notes, and remembered labels live in host `./data/todo.txt`, `./data/Note.md`, and `./data/labels.json`. They survive container recreation and are editable outside the container; task/note file watching continues to refresh clients. Curl is included in the image for the service health check. The startup grace period is 30 seconds, with no compilation on startup.

## Configuration

The base Compose file uses fixed values with no shell-variable interpolation:

- Image: `git.lan/ntr/ntr-server:latest` (mutable; pin a verified index digest for reproducibility).
- Runtime UID/GID: `1000:1000`.
- Host mapping: `127.0.0.1:8080:8080`.
- Container binding inherited from the image: `HOST=0.0.0.0`, `PORT=8080`.
- Resource paths inherited from the image: `/data/todo.txt`, `/data/Note.md`, and `/data/labels.json`.
- Creation-date behavior uses the server's default: `true`.

For HA tasks, add the environment, token secret, read-only CA mount, and persistent metadata path shown in [HA container deployment](home-assistant.md#container-deployment). Notes and labels stay on the existing mount; the old task file is not migrated or modified.

The base Compose service has no `environment` or `env_file` section. Shell variables such as `NTR_IMAGE`, `NTR_BIND`, `NTR_PORT`, `NTR_UID`, `NTR_GID`, and `TODO_ADD_CREATION_DATE` do not change this base configuration. The image itself still supports the [server environment settings](configuration.md) when run separately.

## Home Assistant dotenv override

Copy `.env.example` to `.env` and edit it. Prepare `data/`, a protected token in `secrets/ha-token`, and your trust certificate in `certificates/homeassistant.lanCA.crt`, with permissions readable by container UID 1000. Select a verified image containing server 0.2.11 or later, then use:

```sh
docker compose -f docker-compose.yml -f docker-compose.ha.yml config --quiet
docker compose -f docker-compose.yml -f docker-compose.ha.yml up -d
```

The tracked HA override injects `.env` through service `env_file` and mounts the private directories read-only. It does not alter the existing loopback port mapping or persistent data mount. Relative example paths resolve under the image's `/data` working directory. Compose's automatically loaded `.env` is normally only an interpolation source; it needs this explicit `env_file` wiring to reach the service. Explicit `environment` values in another override win over `env_file`. Run `config --quiet` to validate without printing settings. See [HA setup and path rules](home-assistant.md#container-deployment). No container is started during automated tests.

## Upgrade and rollback

Back up `./data`, then update the literal `image` reference in `docker-compose.yml` to the desired verified index digest before pulling and starting. New template-derived publications do not create versioned container aliases; older published version aliases remain historical artifacts. Pulling `latest` instead follows whichever release currently owns that mutable alias:

```sh
docker compose pull
docker compose up -d
docker compose ps
```

For a digest-pinned deployment, set the file's `image` value to `git.lan/ntr/ntr-server@sha256:<index-digest>` using the release output. Pin the index digest to retain automatic amd64/arm64 selection; a platform digest pins only one architecture.

To roll back a deployment, change the file's `image` value to a previously verified index digest or an older existing version alias and repeat pull/startup. Do not roll back registry aliases or rewrite release records. Keep the same data mount and runtime UID/GID. Container replacement does not remove the host data directory; never delete it as part of an image upgrade.
