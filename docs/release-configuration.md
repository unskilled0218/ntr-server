# Release configuration and migration

- [Settings and versions](#settings-and-versions)
- [Credentials and tools](#credentials-and-tools)
- [Migration from the old publisher](#migration-from-the-old-publisher)

## Settings and versions

The supported files are `scripts/release.sh`, `release-common.sh`, `release-data.py`, and strict schema-1 `release.json`. The upstream pin is `3c73528231ac1cbdd43600a5ca723390db804bac`; no managed updater or network self-update runs during release. See [local hardening](release-publication.md#local-hardening) for intentional deviations.

The settings have exactly `schema`, `gitea`, `repository`, `image`, `username`, `token_file`, `notes`, and `platforms`. Current targets are `https://git.lan`, `ntr/ntr-server`, and `git.lan/ntr/ntr-server`, with `linux/amd64` and `linux/arm64` in that order. Do not copy the template's sample project/image. Unknown/duplicate fields, credentials in URLs, unsafe paths, and inconsistent owner/host scopes are refused.

Root `VERSION` is canonical. Cargo's `my-ntr-server` manifest and lockfile versions are checked mirrors, not independent release versions. Update them together with the README prepared version/link and configured committed notes. Notes begin `# Release v<version>` and contain project changes, not image instructions or machine identity. A release's body is exactly the normalized committed note text; its distinct annotated Git tag binds source/configuration/scripts/notes and image identity. Validation reads TOML with Python's standard library and never invokes Cargo or rewrites metadata.

`--validate` checks local settings, version mirrors, notes, and active paths without credentials, locks, temporary state, Docker, or network calls. `--help` requires only Bash. No arguments performs publication; the explicit `--associate-existing <tag> --expected-tag-oid <oid> --expected-source <commit>` mode performs [pinned association recovery](package-association-recovery.md). Invocation from the parent or another directory still selects the server root containing the script. Unsupported, incomplete, duplicated, or out-of-order arguments and deprecated `FORGEJO_*` release overrides refuse before effects; errors never print their supplied values.

## Credentials and tools

Provision a user-owned, readable non-symlink regular token file outside the project, mode `0600` or `0400`. The default is `~/.config/git/token`. `GIT_TOKEN` optionally overrides the file path, never the token value. `GIT_USERNAME` optionally overrides the configured `dev` login, which follows the checkout's effective Git username. The login must be the token owner, not necessarily the `ntr` organization. Account access is not established by local configuration.

Do not put tokens in Git, `.env`, command arguments, documentation, or shell history. Publication needs package/repository read-write access, release creation, and the target repository's package permissions, plus independent Git authentication. Token scopes do not grant account access to inaccessible resources.

Recovery uses the same protected current token convention, independent Git read authentication, API/repository/registry read access, and package-link write permission. It does not use historical credentials or Docker login. A read-only token cannot perform a missing association POST; correct existing links are verified without writes.

Local tooling requires Bash, Python 3.11+ standard library, Git, curl, and standard Linux utilities. Publication additionally needs Docker with a selected existing Buildx builder advertising both platforms. The publisher does not create/select builders, install packages/emulation, change host configuration, or weaken TLS. Prepare Git/curl/Docker/BuildKit CA trust separately; missing trust is an error. Missing tools or platform support require operator preparation, not a fallback to a single platform.

Registry credentials live only in the run-owned private Docker configuration and are cleaned on ordinary exit, failure, and catchable signals. SIGKILL cannot run cleanup. The original registry auth and credential-helper inventory are not copied. Docker host/context/TLS selection, Buildx state, approved selected-plugin discovery, and build proxy settings are retained independently. The publisher checks the original and isolated Buildx path, version, and file hash before any publication write. It does not change global context/builder selection or log the caller in permanently.

## Migration from the old publisher

- The old Python engine and inline `FORGEJO_TOKEN`/`FORGEJO_USERNAME`/target/CA overrides are retired. Configure `release.json`, normal HTTPS trust, and the external token file instead. A legacy override fails loudly, including when a default token file exists.
- Automatic `ntr-release` builder creation is retired. Select and prepare your builder explicitly outside the release workflow; normal Docker/Buildx connection variables retain their selection meanings.
- New releases use verified staging/digest identity plus mutable `latest`. They do not create `:<version>` container aliases or upload `img.tar.gz`/checksum attachments. Existing older aliases, tags, releases, archives, and packages are not deleted or rewritten.
- Compose now selects `latest`, which may not exist until a separately approved first publication. Prefer a verified index digest for reproducible deployment. Do not assume `:0.2.8` is published by this workflow.
- Application tests run separately before committing/pushing release source. The publisher has no arbitrary test hooks, updater, retention stage, archive compatibility layer, or old-checkpoint migration. Preserve the original checkout/pin for historical transactions; a new script cannot claim ownership of old-format evidence.
- The [package-link contract](package-association-contract.md) permits new links only on stable Gitea `1.25.x` or exact stable `28.2.0`; observed correct links are read-only on other compatible API deployments. Unsupported/unverified servers cannot authorize new association writes. The deployed family and real permissions still require separate verification; a publication can remain incomplete at association without any rollback. Pinned recovery verifies historical identity as data and does not adopt arbitrary old checkpoints or execute old scripts.
