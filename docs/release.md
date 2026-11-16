# Server releases

- [Configuration](#configuration)
- [Credentials and tools](#credentials-and-tools)
- [Local checks](#local-checks)
- [Publication](#publication)
- [Retries and recovery](#retries-and-recovery)
- [Package association](#package-association)

## Configuration

From the server directory, review `scripts/release.json` for repository/image, username, protected token-file path, notes, and ordered `linux/amd64`, `linux/arm64` targets. `VERSION` is canonical; Cargo/README mirrors, notes link, and configured notes must match. Notes start with `# Release v<version>`.

The tag binds source, configuration/scripts/notes, and the image digest. Changed inputs cannot claim an existing release identity.

## Credentials and tools

Use the configured token file or `GIT_TOKEN` path override, not a token value. It must be user-owned, readable, regular, non-symlink, mode `0600` or `0400`. `GIT_USERNAME` may select the token owner's login. Keep credentials out of Git, `.env`, arguments, and shell history; Git authentication is separate.

Publication needs Bash, Python 3.11+, Git, curl, Docker/Buildx, a selected two-platform builder, repository/package/release permissions, and trusted HTTPS/tool configuration. The script does not install tools, prepare builders, or disable TLS.

## Local checks

```sh
./scripts/release.sh --validate
```

Also run [development checks](development.md). Validation is offline and needs no credentials or Docker; fixture tests do not certify live permissions/APIs. Report unavailable tools honestly.

## Publication

After separate approval, use clean, already-pushed `main`, exact configured destinations, and exclusive release/package/association writers across machines. The local lock is not distributed. Run `./scripts/release.sh` only when ready for real writes.

The script verifies both platforms/digest, pushes only the annotated version tag without force, promotes `latest`, creates a notes-only release, and checks package association. It never pushes source or deletes accepted objects. Curated stderr phases/waiting updates hide raw tool output and credentials.

## Retries and recovery

Completed releases are reverified against bound identity and `latest`. Partial/conflicting/changed evidence stops rather than repairs. A lost response may follow an accepted write: inspect actual state before separately approved recovery. Do not delete records, force-push tags, roll back aliases, or retry blindly.

For an older accepted publication's missing association, use [pinned recovery](package-association-recovery.md), not republishing with changed tools.

## Package association

Association affects every version of a package. Correct links verify read-only; conflicts/ambiguity refuse. A new link needs reviewed server behavior, exclusive writers, and exact readback. See the [contract](package-association-contract.md). Publication may already be complete when association fails; no automatic rollback follows.
