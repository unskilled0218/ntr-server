# Container image releases

The publisher builds `linux/amd64` and `linux/arm64` and pushes one versioned image index to Forgejo Packages. The default image is `git.lan/ntr/ntr-server:<Cargo version>`, currently `0.2.2`. This is a container registry publication, not a Forgejo release attachment. It creates no Git tag, does not push source, and does not publish a mutable `latest` alias.

## Prerequisites

- A clean, committed server `main` checkout; only the server submodule's state is checked.
- Bash, Git, Cargo, Python 3, curl, Docker, and the Docker Buildx plugin. Deployment also needs Docker Compose.
- An existing `docker-container` builder named `ntr-release` with running nodes supporting both platforms, either natively or through preconfigured BuildKit emulation. BuildKit's emulation support for building does not necessarily enable running ARM64 containers on an amd64 engine.
- A Forgejo token with `write:package` and permission to publish under owner `ntr`. Token-only login-name lookup also requires `read:user`. A repository-release token with only repository-write access is insufficient.
- Working HTTPS trust and network access from the Docker client and builder to the registry. The script does not disable TLS verification, install emulation, create a builder, or change host configuration.

The CLI contracts are based on Docker's documented Buildx commands and the Buildx 0.13+ formatter/source interface. No real Docker/Buildx version has been runtime-tested in the implementation environment, which lacks Docker. Validate the commands and missing-manifest diagnostic against your installed version before the first live release; unrecognized errors stop publication.

## Builder setup

Run these commands yourself once if you do not already have a suitable builder:

```sh
docker buildx create --name ntr-release --driver docker-container
docker buildx inspect ntr-release --bootstrap
```

Confirm both `linux/amd64` and `linux/arm64` appear. Multiple native nodes can be added with `docker buildx create --append`; see [Docker's multi-platform instructions](https://docs.docker.com/build/building/multi-platform/). An emulated Rust ARM64 build may be substantially slower than a native build. Missing platform support is an error, never a fallback to a host-only release.

The release script always uses `ntr-release`, ignoring the caller's `BUILDX_BUILDER` and current builder selection. It isolates registry credentials in a private temporary Docker configuration, retains the original `BUILDX_CONFIG` state path, exports/imports only the selected engine and node contexts, and preserves user-installed plugin discovery and normal build proxy settings. It does not change global context/builder selection or remove the builder. Builder bootstrapping can start its existing BuildKit containers and normal builds update its cache and logs.

## Publish

From the parent repository root, after creating the `ntr-release` builder:

```bash
FORGEJO_TOKEN="$(<"$HOME/.config/forgejo/forgejo-token")" server/scripts/release.sh
```

From the server repository root, use `scripts/release.sh` instead. The entry point locates its own repository, so either absolute invocation also works from another directory.

The token-only command resolves the token user's login through the Forgejo API. To avoid the optional `read:user` permission and API lookup, supply the actual token owner's username:

```bash
FORGEJO_USERNAME=your-login FORGEJO_TOKEN="$(<"$HOME/.config/forgejo/forgejo-token")" scripts/release.sh
```

The login name is not necessarily the package owner `ntr`. Tokens are passed to `docker login --password-stdin`, never build arguments. Temporary login files are removed on normal exit, failure, Ctrl-C, and termination; an uncatchable process kill cannot run cleanup. Do not enable shell tracing when expanding the token in your caller's shell.

Configuration:

- `FORGEJO_REGISTRY`: lowercase host and optional port, default `git.lan`; no URL scheme.
- `FORGEJO_IMAGE`: untagged `registry/owner/image` repository, default `<registry>/ntr/ntr-server`; it must use the configured registry. Names use lowercase alphanumeric components with single dot, underscore, or hyphen separators. Tags and digests are supplied by the script.
- `FORGEJO_USERNAME`: token owner's login; absent means authenticated API lookup.
- `FORGEJO_API_URL`: HTTPS API base, default `https://<registry>/api/v1`; used only for username lookup.
- `BUILDX_BUILDER` is forced to `ntr-release` for the publisher's Docker commands. `BUILDX_CONFIG`, `DOCKER_CONFIG`, and Docker engine/context/TLS environment variables retain their normal connection meanings. The initial supported driver is `docker-container`.

The publisher selects `my-ntr-server` from Cargo metadata, runs `cargo test --locked`, checks the checkout again, and invokes Buildx with `--platform linux/amd64,linux/arm64 --push`. Each platform gets the same OCI version, full Git revision, and source labels. It records the build metadata index digest, validates both platform configurations by immutable digest, and checks the version tag still points to that built index before reporting success.

Success prints the versioned image reference, index digest, both platform manifest digests, version, and revision. Compose uses the shared tag or index digest and selects the correct architecture automatically. Cargo.lock fixes Rust dependency versions, but mutable base-image tags mean identical source revisions do not guarantee byte-identical rebuilds.

## Failure and reruns

Missing tools/credentials, unsupported builder drivers/platforms, tests, login, builds, pushes, and verification failures return nonzero. A failed export may have uploaded intermediate blobs or even committed an index before a connection failure; inspect the registry before deciding what happened.

An existing tag is reused only when it contains both required platforms and both configurations match this checkout's version/revision. Conflicting or incomplete releases are rejected. Bump Cargo.toml/Cargo.lock and the default Compose reference deliberately for a new source revision; same-version overwrites are not an automatic repair path. Concurrent releases for the same version are unsupported because registries do not offer a universal tag compare-and-swap.

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
docker buildx build --builder ntr-release --platform linux/amd64 --load --build-arg VERSION=0.2.2 --build-arg REVISION="$(git rev-parse HEAD)" -t ntr-server:smoke-amd64 .
docker buildx build --builder ntr-release --platform linux/arm64 --load --build-arg VERSION=0.2.2 --build-arg REVISION="$(git rev-parse HEAD)" -t ntr-server:smoke-arm64 .
```

Run each on a matching native engine or configured emulator with disposable data, check `/api/health`, update tasks/note/labels, recreate the container, and confirm persistence and external task/note edits. Record native versus emulated execution. For live publication, inspect the reported index and explicitly pull both architectures by that same index digest, then repeat the runtime checks and verify a same-revision rerun.

See [Compose deployment](docker-compose.md) and [Forgejo's container registry](https://forgejo.org/docs/latest/user/packages/container/).
