# Docker Compose

- [Start](#start)
- [Configuration](#configuration)
- [Upgrade and rollback](#upgrade-and-rollback)

Run commands from the server repository root. Compose selects `git.lan/ntr/ntr-server:latest`, a mutable shared amd64/arm64 image; Docker selects the host's architecture. No source mount or Rust compiler is needed. A separately approved first publication must establish this alias through the [image release workflow](image-release.md); local preparation is not publication. For reproducible deployments, replace it with a verified immutable index digest.

## Start

If the package is private, authenticate with a Forgejo username and token allowed to read it. Public packages may be pulled without login:

```bash
docker login git.lan --username your-login --password-stdin < "$HOME/.config/git/token"
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

The Compose file uses fixed values with no shell-variable interpolation:

- Image: `git.lan/ntr/ntr-server:latest` (mutable; pin a verified index digest for reproducibility).
- Runtime UID/GID: `1000:1000`.
- Host mapping: `127.0.0.1:8080:8080`.
- Container binding inherited from the image: `HOST=0.0.0.0`, `PORT=8080`.
- Resource paths inherited from the image: `/data/todo.txt`, `/data/Note.md`, and `/data/labels.json`.
- Creation-date behavior uses the server's default: `true`.

For HA tasks, add the environment, token secret, read-only CA mount, and persistent metadata path shown in [HA container deployment](home-assistant.md#container-deployment). Notes and labels stay on the existing mount; the old task file is not migrated or modified.

The Compose service has no `environment` section. Shell variables such as `NTR_IMAGE`, `NTR_BIND`, `NTR_PORT`, `NTR_UID`, `NTR_GID`, and `TODO_ADD_CREATION_DATE` do not change this configuration. The image itself still supports the [server environment settings](configuration.md) when run separately.

## Upgrade and rollback

Back up `./data`, then update the literal `image` reference in `docker-compose.yml` to the desired verified index digest before pulling and starting. New template-derived publications do not create versioned container aliases; older published version aliases remain historical artifacts. Pulling `latest` instead follows whichever release currently owns that mutable alias:

```sh
docker compose pull
docker compose up -d
docker compose ps
```

For a digest-pinned deployment, set the file's `image` value to `git.lan/ntr/ntr-server@sha256:<index-digest>` using the release output. Pin the index digest to retain automatic amd64/arm64 selection; a platform digest pins only one architecture.

To roll back a deployment, change the file's `image` value to a previously verified index digest or an older existing version alias and repeat pull/startup. Do not roll back registry aliases or rewrite release records. Keep the same data mount and runtime UID/GID. Container replacement does not remove the host data directory; never delete it as part of an image upgrade.
