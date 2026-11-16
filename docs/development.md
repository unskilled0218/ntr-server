# Development

- [Application checks](#application-checks)
- [Dependency updates](#dependency-updates)
- [Release tooling](#release-tooling)

## Application checks

HA adapter tests require Python 3 and OpenSSL as well as Rust. They start generated-certificate loopback HTTPS fixtures without live HA writes. Use a writable temporary directory, such as `TMPDIR=/tmp/opencode` in the agent container.

The multi-list fixture implements authenticated WSS registry commands and Local To-do config flows without external Python packages. Multi-list tests cover schema migration, duplicate UIDs, default/provider/admin guards, confirmed moves, cancellation, ambiguous/lost responses, concurrent source edits, journal-save failures, restart, and metadata-only reconcile/abandon. No recovery test automatically replays an upstream copy/delete. HA's final external-writer race cannot be made transactional by the fixture; see the documented exclusive-writer requirement.

Catalog notification tests inject one-shot empty-list rename/add/remove changes during an otherwise self-only write and check actual author SSE delivery, self-only filtering, and silent later polls. On Unix, durable-write tests exercise write/file-sync/rename/directory-sync ordering and failures without changing global working directories or permissions. Private per-store persistence injection covers post-rename failures at move intent, verified target, completion, reconcile, and abandon. These tests verify admission and restart behavior, not physical power-loss resilience; see [metadata durability](home-assistant.md#metadata-durability).

Dotenv unit tests use explicit files/environment maps, and startup integration tests run the binary in isolated directories with child-only variables. Read-failure tests use a directory at `.env` rather than privilege-dependent permission bits. Run `python3 -B -m unittest discover -s tests -p 'test_env_compose.py' -v` for single-file HA/file-mode rendering and missing-dotenv checks; it uses temporary configuration without a daemon or container startup, and reports a skip when Compose is unavailable.

Run commands from the server repository root.

```sh
cargo test     # unit + integration tests
cargo fmt      # 2-space indent (see rustfmt.toml)
cargo clippy --all-targets
```

## Dependency updates

The 0.2.14 refresh uses tower-http 0.7, reqwest 0.13, tokio-tungstenite 0.30, and current compatible locked crates; validation uses Rust 1.100.0. The HA client selects the ring crypto provider explicitly and shares web-PKI/custom CA roots between HTTPS and WSS rather than inheriting changed reqwest TLS defaults. Keep redirect rejection, hostname checks, bounded operations, provenance, and journal-failure tests intact when upgrading transport dependencies.

Use `cargo update` deliberately, then `cargo test --locked`, `cargo fmt --check`, strict Clippy, Compose rendering, and the sibling app's HA serializer contract. Run `cargo audit` separately when installed; successful dependency resolution/builds do not establish absence of Rust vulnerabilities. Do not claim cross-platform or live-HA validation from the disposable Linux fixture.

## Release tooling

Requires Bash and Python 3.11+ standard library, with no jq or pip packages. Run `python3 -B -m unittest discover -s tests -p 'test*.py' -v`, syntax-check both release shell scripts with `bash -n`, then run `./scripts/release.sh --validate`. See [release checks](release.md#local-checks).

Run application tests before committing and separately pushing approved release source. The publisher does not run application test hooks or push source. No-argument `release.sh` performs real publication; local development verification uses `--validate` only. `VERSION` is the canonical release version; keep Cargo/README/notes mirrors consistent.

Historical association recovery is an explicit write-capable operator mode, not a development dry run. Its [source/tag pins and authority](package-association-recovery.md) are tested only in disposable fixtures. Fixed progress/heartbeat messages use stderr; machine-readable helper output and final references remain on stdout. Do not enable raw child-output streaming for debugging authenticated commands.
