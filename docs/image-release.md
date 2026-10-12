# Container image releases

The publisher builds `linux/amd64` and `linux/arm64`, pushes one versioned image index to Forgejo Packages, and uploads `img.tar.gz` and `img.tar.gz.sha256` to the server repository's Forgejo Release. The default image is `git.lan/ntr/ntr-server:<Cargo version>`, currently `0.2.3`; the release tag is `v0.2.3`. Forgejo creates the remote Git tag at the server revision when necessary. The script does not push source or publish a mutable `latest` alias.

## Prerequisites

- A clean, committed server `main` checkout whose revision is already pushed to the server's Forgejo repository; only the server submodule's state is checked.
- Bash, Git, Cargo, Python 3.9+, curl, Docker, and the Docker Buildx plugin. Skopeo is not required. Deployment also needs Docker Compose.
- An engine supporting a `docker-container` builder with both platforms, either natively or through preconfigured BuildKit emulation. The script creates `ntr-release` if missing. BuildKit's emulation support for building does not necessarily enable running ARM64 containers on an amd64 engine.
- A Forgejo token with `write:package`, `write:repository`, package-owner access, and server-repository release access. A token limited to either packages or repository releases is insufficient.
- Working HTTPS trust and network access from the Docker client, builder, and curl. The script never disables TLS verification, installs system tools/emulation, or changes daemon trust.

The CLI contracts use Docker's documented Buildx commands and the Buildx 0.13+ formatter/source interface. Formatter checks ran with Docker 29.9.0 and Buildx 0.38.0; full publication and live authenticated registry export remain unverified in the implementation environment. Validate the missing-manifest diagnostic against your installed version before the first live release; unrecognized errors stop publication.

## Builder setup

The script always uses `ntr-release`. If missing, it creates a `docker-container` builder without changing your global current builder, then bootstraps and verifies both platforms. It uses `~/.config/buildkit/buildkitd.toml` if that file exists, or Docker's normal defaults otherwise. Set `BUILDKIT_CONFIG` to use a different existing file; a missing explicit file is an error.

For your custom registry CA, the existing BuildKit configuration should contain:

```toml
[registry."git.lan"]
  ca = ["/absolute/path/to/forgejo-ca.crt"]
```

Buildx copies the configured CA into a newly created builder. Existing builders are reused without rewriting their configuration. If an existing builder lacks your CA, recreate it manually once with the correct configuration; removing it discards its build cache:

```sh
docker buildx rm ntr-release
# The next release invocation recreates it using your BuildKit configuration.
```

Confirm both `linux/amd64` and `linux/arm64` appear. Multiple native nodes can be added with `docker buildx create --append`; see [Docker's multi-platform instructions](https://docs.docker.com/build/building/multi-platform/). An emulated Rust ARM64 build may be substantially slower than a native build. Missing platform support is an error, never a fallback to a host-only release.

The release script ignores the caller's `BUILDX_BUILDER` and current builder selection. It isolates registry credentials in a private temporary Docker configuration, prevents native credential-helper autodetection, retains the original `BUILDX_CONFIG` state path, exports/imports only the selected engine and node contexts, and preserves user-installed plugin discovery and normal build proxy settings. It does not change global context/builder selection or remove the builder. Builder creation persists for reuse; bootstrapping can start its BuildKit containers and normal builds update its cache and logs.

## Publish

From the parent repository root:

```bash
FORGEJO_TOKEN="$(<"$HOME/.config/forgejo/forgejo-token")" server/scripts/release.sh
```

From the server repository root, use `scripts/release.sh` instead. The entry point locates its own repository, so either absolute invocation also works from another directory.

The token-only command reads the login from `git config --get user.name` in the server checkout, honoring Git's local/global configuration precedence. That value must be the token owner's Forgejo login, not a display name containing spaces. No API lookup or `read:user` permission is needed. To use a different login without changing Git author configuration, supply it explicitly:

```bash
FORGEJO_USERNAME=your-login FORGEJO_TOKEN="$(<"$HOME/.config/forgejo/forgejo-token")" scripts/release.sh
```

The login name is not necessarily the package owner `ntr`. Tokens are passed to `docker login --password-stdin`, never build arguments. Temporary login files are removed on normal exit, failure, Ctrl-C, and termination; an uncatchable process kill cannot run cleanup. Do not enable shell tracing when expanding the token in your caller's shell.

Configuration:

- `FORGEJO_REGISTRY`: lowercase host and optional port, default `git.lan`; no URL scheme.
- `FORGEJO_IMAGE`: untagged `registry/owner/image` repository, default `<registry>/ntr/ntr-server`; it must use the configured registry. Names use lowercase alphanumeric components with single dot, underscore, or hyphen separators. Tags and digests are supplied by the script.
- `FORGEJO_USERNAME`: token owner's login; defaults to the server checkout's effective Git `user.name`. Missing or invalid login names stop publication.
- `FORGEJO_API_URL`: complete repository API URL, default `https://<registry>/api/v1/repos/ntr/ntr-server`. Set this explicitly if the image override belongs to another source repository.
- `FORGEJO_CA_FILE`: optional existing CA certificate bundle used for curl API requests and registry downloads. Installed system CA certificates are used by default; no setting is needed when your custom CA is already trusted there. Otherwise set this to the same CA referenced by BuildKit. Docker login and Buildx inspection still require the Docker client's existing registry trust.
- `BUILDKIT_CONFIG`: existing BuildKit configuration used only when creating a missing builder; default `~/.config/buildkit/buildkitd.toml` when present.
- `BUILDX_BUILDER` is forced to `ntr-release` for the publisher's Docker commands. `BUILDX_CONFIG`, `DOCKER_CONFIG`, and Docker engine/context/TLS environment variables retain their normal connection meanings. The initial supported driver is `docker-container`.

The publisher selects `my-ntr-server` from Cargo metadata, runs `cargo test --locked`, checks the checkout again, and invokes Buildx with `--platform linux/amd64,linux/arm64 --push`. Each platform gets the same OCI version, full Git revision, and source labels. It records the build metadata index digest, validates both platform configurations by immutable digest, and checks the version tag still points to that built index before reporting success.

Success prints the versioned image reference, index digest, both platform manifest digests, version, and revision. Compose uses the shared tag or index digest and selects the correct architecture automatically. Cargo.lock fixes Rust dependency versions, but mutable base-image tags mean identical source revisions do not guarantee byte-identical rebuilds.

With a custom CA:

```bash
FORGEJO_CA_FILE=/absolute/path/to/forgejo-ca.crt \
  FORGEJO_TOKEN="$(<"$HOME/.config/forgejo/forgejo-token")" server/scripts/release.sh
```

## Image archive attachments

After registry verification, curl downloads the immutable index and all referenced manifests, configurations, and layers through the registry's HTTPS API. Python verifies each downloaded digest and writes a compressed OCI image layout without rebuilding or changing manifest bytes. The export includes both architectures and attestation descriptors. No Skopeo or additional image-copy tool is used. Output files:

```text
server/release/<version>/img.tar.gz
server/release/<version>/img.tar.gz.sha256
```

Both files are uploaded to Forgejo Release `v<version>` in `ntr/ntr-server`. The SHA-256 file checks the compressed archive and names `img.tar.gz`. The release description records the image index and Git revision; both the description and resolved remote tag must match before attachments are reused. Existing conflicting Git tags/releases are rejected. Generated local files are ignored by Git and stay outside the Docker build context.

The registry downloader supports Basic authentication and the registry's Bearer-token challenge. Tokens travel through curl configuration on stdin, not command arguments. Token endpoints must use the registry's HTTPS host. HTTPS redirects to external blob storage are followed without forwarding registry credentials; HTTP redirects are rejected. Registry downloads ignore `.curlrc` so it cannot inject credentials or insecure options. Custom registry CA overrides apply to the registry host; external storage uses system trust.

This is a multi-platform OCI archive. Classic Docker image stores cannot reliably load its full index through `docker load`. An OCI-capable Docker image store can import it directly after checksum verification:

```sh
cd server/release/0.2.3
sha256sum -c img.tar.gz.sha256
docker image load -i img.tar.gz
docker image ls
```

Timestamped progress messages go to stderr for checkout validation, builder/context setup, registry login, tag inspection, tests, build/push, platform verification, and cleanup. Cargo tests, builder bootstrap, and Buildx plain-progress output stream as complete lines arrive, including diagnostics from failed commands. The publisher redacts the supplied token and never logs login stdin. Final release references and digests remain on stdout. To save both streams while retaining the publisher's failure status, run from Bash:

```bash
set -o pipefail
FORGEJO_TOKEN="$(<"$HOME/.config/forgejo/forgejo-token")" server/scripts/release.sh 2>&1 | tee /tmp/ntr-release.log
```

## Failure and reruns

Missing tools/credentials, unsupported builder drivers/platforms, tests, login, builds, pushes, and verification failures return nonzero. A failed export may have uploaded intermediate blobs or even committed an index before a connection failure; inspect the registry before deciding what happened.

An existing tag is reused only when it contains both required platforms and both configurations match this checkout's version/revision. Conflicting or incomplete releases are rejected. Bump Cargo.toml/Cargo.lock and the default Compose reference deliberately for a new source revision; same-version overwrites are not an automatic repair path. Concurrent releases for the same version are unsupported because registries do not offer a universal tag compare-and-swap.

Archive/API/upload failures return nonzero even if the registry image is already published. Reruns reuse that index, verify existing archive/checksum attachments by downloading them, and upload only missing files. An archive-only partial upload can resume by generating its checksum. A checksum-only release, invalid checksum, corrupted archive, conflicting release identity, or cross-host download URL stops rather than replacing attachments. Existing artifacts are not deleted on failure. Keep the repository's attachment-size limit and available disk space large enough for both architectures and temporary uncompressed data.

The only accepted absent-reference diagnostic is an exit-1 response with empty stdout and exactly `ERROR: <requested-reference>: not found` or `<requested-reference>: not found` on stderr. The same words in an unrelated or multiline error do not authorize a push. Validate this diagnostic with a known unused tag in the authenticated publishing namespace on your installed Buildx version. Authorization, connectivity, malformed JSON, and unknown errors fail closed. No published package is deleted on failure.

## Verification

Run from the server root:

```sh
bash -n scripts/release.sh
python3 -m unittest discover -s tests -p 'release_script_test.py'
cargo test --locked
```

For local runtime checks, build distinct single-platform smoke images with `--load` (rather than trying to load the combined index into a classic Docker image store):

```sh
docker buildx build --builder ntr-release --platform linux/amd64 --load --build-arg VERSION=0.2.3 --build-arg REVISION="$(git rev-parse HEAD)" -t ntr-server:smoke-amd64 .
docker buildx build --builder ntr-release --platform linux/arm64 --load --build-arg VERSION=0.2.3 --build-arg REVISION="$(git rev-parse HEAD)" -t ntr-server:smoke-arm64 .
```

Run each on a matching native engine or configured emulator with disposable data, check `/api/health`, update tasks/note/labels, recreate the container, and confirm persistence and external task/note edits. Record native versus emulated execution. For live publication, inspect the reported index and explicitly pull both architectures by that same index digest, then repeat the runtime checks and verify a same-revision rerun.

See [Compose deployment](docker-compose.md) and [Forgejo's container registry](https://forgejo.org/docs/latest/user/packages/container/).
