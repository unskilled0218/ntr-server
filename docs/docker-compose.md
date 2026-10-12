# Docker Compose

Run commands from the server repository root. Compose pulls `git.lan/ntr/ntr-server:0.2.3`, a shared amd64/arm64 image; Docker selects the host's architecture. No source mount or Rust compiler is needed. The version must first be published through the [image release workflow](image-release.md).

## Start

If the package is private, authenticate with a Forgejo username and token allowed to read it. Public packages may be pulled without login:

```bash
FORGEJO_TOKEN="$(<"$HOME/.config/forgejo/forgejo-token")"
printf '%s' "$FORGEJO_TOKEN" | docker login git.lan --username your-login --password-stdin
unset FORGEJO_TOKEN
```

Ensure the data directory exists before starting. The container defaults to UID/GID `1000:1000`. If your existing directory/files use another identity, set `NTR_UID` and `NTR_GID` to a user that can read them and create/rename files in the directory. The server's atomic writes need directory access, not merely write access to an existing file. Compose does not change host file ownership:

```sh
mkdir -p data
export NTR_UID="$(id -u)" NTR_GID="$(id -g)"
docker compose config
docker compose pull
docker compose up -d
docker compose ps
curl -fsS http://127.0.0.1:8080/api/health
```

Do not select UID `0` to work around permissions. Use a writable directory belonging to your runtime user, or resolve permissions with the data owner before deployment. Existing files created by the previous source-build container may need that preparation.

Tasks, notes, and remembered labels live in host `./data/todo.txt`, `./data/Note.md`, and `./data/labels.json`. They survive container recreation and are editable outside the container; task/note file watching continues to refresh clients. Curl is included in the image for the service health check. The startup grace period is 30 seconds, with no compilation on startup.

## Configuration

- `NTR_IMAGE`: complete image reference, default `git.lan/ntr/ntr-server:0.2.3`. Override with another registry/version or an index digest.
- `NTR_BIND`: host bind address, default `127.0.0.1`. Set `0.0.0.0` when intentional network access is needed; the server has no authentication.
- `NTR_PORT`: host port, default `8080`. Container port remains `8080` and its internal `HOST` remains `0.0.0.0` so Docker port publishing works.
- `NTR_UID` / `NTR_GID`: runtime numeric identity, default `1000` / `1000`.
- `TODO_ADD_CREATION_DATE`: creation-date behavior, default `true`.

```sh
NTR_PORT=9000 docker compose up -d
NTR_BIND=0.0.0.0 docker compose up -d
```

The image itself retains all [server environment settings](configuration.md); the Compose service supplies the `/data` file paths and fixed internal port. Use a Compose override if you need different internal file paths or networking, and update the mount/health check consistently.

## Upgrade and rollback

Back up `./data`, then choose a published version or index digest and use the same `NTR_IMAGE` value for pull and startup:

```sh
export NTR_IMAGE=git.lan/ntr/ntr-server:0.2.3
docker compose pull
docker compose up -d
docker compose ps
```

For a digest-pinned deployment, set `NTR_IMAGE=git.lan/ntr/ntr-server@sha256:<index-digest>` using the release output. Pin the index digest to retain automatic amd64/arm64 selection; a platform digest pins only one architecture.

To roll back, change `NTR_IMAGE` to the previous published version/index digest and repeat pull/startup. Keep the same data mount and runtime UID/GID. Container replacement does not remove the host data directory; never delete it as part of an image upgrade.
