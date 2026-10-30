# Server releases

- [Configuration](#configuration)
- [Credentials and tools](#credentials-and-tools)
- [Local checks](#local-checks)
- [Publication](#publication)
- [Retries and recovery](#retries-and-recovery)
- [Package association](#package-association)

Run commands from the server repository root. The publisher creates real remote changes. Do not run it without separate approval.

## Configuration

`scripts/release.json` selects the server repository, image, username, token file, notes, and both `linux/amd64` and `linux/arm64` platforms. Keep URLs free of credentials and use normal HTTPS trust.

`VERSION` is canonical. Cargo files, the README version and notes link, and the configured release notes must match. Notes start with `# Release v<version>` and become the release body. The release tag binds the source commit, settings, scripts, notes, and image digest. A later source change cannot claim an existing release identity.

## Credentials and tools

Use a user-owned, readable, non-symlink token file outside the repository, mode `0600` or `0400`. The default is `~/.config/git/token`. `GIT_TOKEN` overrides the file path, not the token value. `GIT_USERNAME` overrides the configured login; use the token owner's account.

Never put tokens in Git, `.env`, command arguments, or shell history. Publication requires repository and package permissions, release creation rights, and separate Git authentication.

Install Bash, Python 3.11+, Git, curl, Docker, and Buildx. Prepare a selected builder supporting both platforms and trusted Git, curl, Docker, and BuildKit connections. The publisher does not install tools, prepare builders, or disable TLS. Unsupported environment overrides are refused.

## Local checks

```sh
python3 -B -m unittest discover -s tests -p 'test*.py' -v
bash -n scripts/release.sh
bash -n scripts/release-common.sh
./scripts/release.sh --validate
cargo test --locked
cargo clippy --all-targets --locked
cargo fmt --check
docker compose config
git diff --check
```

`--help` displays usage. `--validate` checks local configuration and version mirrors without credentials, Docker, or network access. It does not verify live permissions or publish anything. The Python tests use temporary data and fake services; they do not certify live APIs or Docker routing. Record unavailable tools and device checks.

## Publication

After separate approval, use a clean, already-pushed server `main`. Verify the configured repository and image, exactly one fetch and push destination, credentials, and the prepared builder. Exclude other release, package, and association writers until completion; the local lock does not protect other checkouts. The publisher does not push source.

```sh
./scripts/release.sh
```

The script verifies the two-platform image and immutable digest, pushes the annotated version tag without force, promotes the digest to `latest`, creates the notes-only release, and checks package association. Existing tags, releases, and packages are not deleted. The Git tag push does not follow unrelated tags.

## Retries and recovery

A completed release is reverified against its bound identity and `latest`. Changed source, settings, scripts, notes, or conflicting evidence cause refusal rather than automatic repair. A tag-only or release-only state is ambiguous and stops publication; inspect the evidence and obtain separate approval before any manual action.

A failed or lost response may leave accepted remote changes. Do not delete records, force-push tags, roll back aliases, or blindly repeat writes. For an older verified publication's missing package association, use the separately approved [pinned recovery procedure](package-association-recovery.md); it does not republish the image.

## Package association

Association applies to every version of the package. An existing correct link requires no write; conflicting or ambiguous evidence is refused. A new link is permitted only on reviewed server versions and requires exclusive writers and exact verification. See the [package association contract](package-association-contract.md).

Failures can leave a verified publication awaiting association. Local tests do not establish account permissions, deployed API behavior, or UI rendering.
