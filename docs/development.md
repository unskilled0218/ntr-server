# Development

- [Application checks](#application-checks)
- [Release tooling](#release-tooling)

## Application checks

HA adapter tests require Python 3 and OpenSSL as well as Rust. They start generated-certificate loopback HTTPS fixtures without live HA writes. Use a writable temporary directory, such as `TMPDIR=/tmp/opencode` in the agent container.

Run commands from the server repository root.

```sh
cargo test     # unit + integration tests
cargo fmt      # 2-space indent (see rustfmt.toml)
cargo clippy --all-targets
```

## Release tooling

Requires Bash and Python 3.11+ standard library, with no jq or pip packages. Run `python3 -B -m unittest discover -s tests -p 'test*.py' -v`, syntax-check both release shell scripts with `bash -n`, then run `./scripts/release.sh --validate`. See [release checks](release.md#local-checks).

Run application tests before committing and separately pushing approved release source. The publisher does not run application test hooks or push source. No-argument `release.sh` performs real publication; local development verification uses `--validate` only. `VERSION` is the canonical release version; keep Cargo/README/notes mirrors consistent.

Historical association recovery is an explicit write-capable operator mode, not a development dry run. Its [source/tag pins and authority](package-association-recovery.md) are tested only in disposable fixtures. Fixed progress/heartbeat messages use stderr; machine-readable helper output and final references remain on stdout. Do not enable raw child-output streaming for debugging authenticated commands.
