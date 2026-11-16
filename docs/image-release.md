# Container image releases

- [Use](#use)
- [Documentation](#documentation)

## Use

The release workflow builds amd64/arm64 images for `git.lan/ntr/ntr-server`, binds an immutable digest to `v<version>`, promotes it to mutable `latest`, and verifies package association where supported.

`./scripts/release.sh --help` shows usage; `--validate` checks local metadata without credentials, Docker, or network. **No arguments publishes**, not a dry run. Use only after separate approval from clean, already-pushed `main` with trusted tools/credentials and exclusive writers.

## Documentation

- [Publication, verification, and recovery limits](release.md).
- [Package association contract](package-association-contract.md) and [pinned recovery](package-association-recovery.md).
- [Current version and notes](../README.md) and [Compose deployment](docker-compose.md).
