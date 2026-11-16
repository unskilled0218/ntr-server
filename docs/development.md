# Development

- [Application checks](#application-checks)
- [Dependency updates](#dependency-updates)
- [Release tooling](#release-tooling)

## Application checks

From the server directory:

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
python3 -B -m unittest discover -s tests -p 'test_env_compose.py' -v
```

HA tests require Python 3/OpenSSL and use generated-CA loopback HTTPS/WSS, not live credentials. They cover routing, admin/provider guards, TLS rejection, moves, cancellation, provenance, migration, persistence faults, and no-replay recovery. Durability tests inject errors, not physical power loss. The Compose test renders HA/file modes and missing-dotenv errors without starting a container; it skips if Compose is unavailable.

Use writable temporary storage, such as `TMPDIR=/tmp/opencode` in the agent container. Dotenv tests use isolated files/maps/processes without mutating global environment. The sibling app provides `npm run test:ha-contract` for the real serializer/server boundary.

## Dependency updates

The current stack uses tower-http 0.7, reqwest 0.13, tokio-tungstenite 0.30, and explicit ring-based HA HTTPS/WSS trust. Dependency validation used Rust 1.100.0. After `cargo update`, rerun checks and the HA contract; keep hostname/no-redirect/bounds/recovery assertions intact. Use `cargo audit` separately when installed. Builds/tests do not prove absence of advisories or live/cross-platform compatibility.

## Release tooling

Run Python 3.11+ release tests and offline validation:

```sh
python3 -B -m unittest discover -s tests -p 'test*.py' -v
./scripts/release.sh --validate
git diff --check
```

No-argument publication and association recovery have real remote effects and need separate approval. `VERSION` is canonical; keep mirrors/current notes aligned. See [release checks](release.md#local-checks) and [recovery authority](package-association-recovery.md).
