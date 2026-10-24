# Container image releases

- [Use](#use)
- [Documentation](#documentation)

## Use

Version `0.2.6` is prepared, not published. The server uses the Bash workflow adapted from [container-release-template](https://git.lan/dev/container-release-template/src/commit/3c73528231ac1cbdd43600a5ca723390db804bac), version `0.5.0`. It builds AMD64/ARM64 images for `git.lan/ntr/ntr-server`, binds an immutable digest in Git tag `v<version>`, promotes it to `latest`, creates a notes-only release, and verifies the repository package association where supported.

```sh
./scripts/release.sh --help
./scripts/release.sh --validate
```

No arguments performs real publication, not a dry run. Run it only after separate approval from clean, already-pushed server `main`, with prepared tools, trusted HTTPS, credentials, and exclusive release/package writers. Local validation needs no credentials, Docker, or services.

## Documentation

- [Configuration and migration](release-configuration.md)
- [Publication, recovery, and package association](release-publication.md)
- [Verification and behavior mapping](release-verification.md)
- [Server changes](releases/0.2.6.md)
- [Compose deployment](docker-compose.md)
