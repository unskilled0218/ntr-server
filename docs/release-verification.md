# Release verification

- [Local checks](#local-checks)
- [Behavior migration map](#behavior-migration-map)
- [Test boundaries](#test-boundaries)
- [Optional image checks](#optional-image-checks)

## Local checks

Run from the server root before committing/pushing release source:

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

Cargo/README mirrors and notes are checked against `VERSION`. Tests additionally check the real project's settings and Docker/Compose contract. `--validate` is offline and does not create transaction/lock state. It is not a live API or permission dry run. Missing Docker/Compose/emulation must be recorded, never worked around by privileged installation or a production daemon.

## Behavior migration map

The retired engine/test file remain in Git history before this task. Their applicable behaviors are mapped to the new real Bash-entry/helper suite, adapted from the pinned template rather than preserving incompatible publication assumptions:

- Parent/server/unrelated-directory invocation and explicit login/settings: fixture invocation from outside the root plus schema/argument/environment validation and configurable token/login handling. Old inline `FORGEJO_*` overrides now refuse explicitly.
- Relative Docker/Buildx state and original config preservation: engine/context/TLS/relative-path/empty-config/builder-override cases, config-only selected-plugin cases, unchanged-original-config assertions, and private-auth sentinel checks. N2 additionally refuses a selected-plugin switch before writes.
- Complete-index reuse and complete-release reruns: completed publication verifies the immutable annotation-bound digest and final alias without staging/build/promotion/release writes; only a missing allowed package link may be completed. This replaces reuse based only on old version-label evidence.
- Build progress/redaction and termination: captured bounded output/stderr, token-free errors, signal-owned cleanup, lock release, and descendant termination tests. Raw streaming build diagnostics are intentionally retired for the reference's bounded secret-safe output contract.
- ARM64 manifests, invalid/conflicting indexes, build metadata, and remote verification: two-platform digest/config/label shape tests, bad metadata refusal, and full publication identity/readback checks.
- Missing-builder creation/default config cases: automatic creation is intentionally retired; unsupported platform/failed builder evidence refuses. The operator prepares the selected builder and trust separately.
- Archive/checksum creation, downloads, resume, layer corruption, and attachment identity/host cases: intentionally retired because the new contract neither downloads layers nor uploads image archives. Existing generated/historical artifacts are preserved. Manifest/config digest, HTTP identity, bounded receiving, and registry authentication checks remain covered independently.
- Registry Basic/Bearer authentication, external token realms, and redirect failures: helper challenge/digest tests, fake-port HTTP failures, same-origin token requirements, no redirect following, private config permissions, and no sensitive response relay. External blob-storage redirects no longer occur because there is no layer/archive downloader.
- Invalid API/CA/configuration, missing credentials/tools, dirty/feature branches, and login/build failures: strict credential-free HTTPS configuration and private-file validation, main/source/remote checks including multiple fetch/push destination refusal, fake command failures, unsupported options, and failed-command classification. Custom CA flags are retired in favor of normal preconfigured HTTPS trust; no insecure fallback is introduced.
- Missing/invalid effective Git login: explicit configured username with validated `GIT_USERNAME` override, rather than deriving authentication authority from author metadata during publication.
- Unknown missing-manifest diagnostics and registry errors: explicit bounded registry HTTP evidence distinguishes actual 404 absence from authentication/transport/JSON/digest refusals; ambiguous evidence does not permit building or overwriting.
- New upstream-contract coverage: exact annotated tags and literal committed notes, snapshot isolation, local locks, startup partial states, lost accepted responses, staging reuse, confirmed-tag loss/replacement, and complete inventory budgets. N1 tests inject predecessor/missing/foreign aliases after confirmed promotion and assert no later write.
- New package coverage: different image/repository names, unrelated-package preservation, correct/null/foreign/permission-hidden links, stable-server write gate, refreshed identity, empty 201, lost transport responses, and post-link publication/readback failures without rollback.

## Test boundaries

The Python suite uses disposable private tokens, temporary real Git/bare remotes, and fake Docker/curl executables. No user token, Docker daemon, real registry login, source push, release creation, or package mutation is performed. Fixtures keep Cargo and README mirrors valid when injecting publication faults, and assert injection markers plus externally visible writes/state, not just a nonzero exit.

Unit receiving tests cover exact byte limits/overflow, unending stdout/stderr, blocked stdin, deadlines, and descendant termination. CLI tests exercise local hardening deviations and the publication/package state machine. These tests model endpoint/plugin discovery and synthetic TLS metadata; they do not certify the real Docker CLI, native routing, account permissions, live APIs, distributed writer exclusion, or Packages-tab rendering. Rust tests protect application behavior separately.

The follow-tags regression enables `push.followTags=true` after fixture source setup and creates a reachable unpublished annotated tag. Actual bare-remote refs must gain only the intended release tag, with unrelated tags staying local and source refs/configuration unchanged. Ordinary and accepted-but-lost tag pushes each occur once; completed retries must remain read-only. The regression fails against the original command because real Git publishes the unrelated tag, not because a fake requires a particular argument. This command-local isolation is an additional server hardening change; it does not resolve the separate ARM64 v8 compatibility warning.

## Optional image checks

Only on a confirmed local non-production Docker context with a prepared builder, build both platforms with local output and release-equivalent labels; never add `--push` or run the publisher without `--validate` as a test. The real image must contain the Rust server, not the template's sample command. OCI labels are supplied by the build invocation, not empty Dockerfile arguments.

For each runnable architecture, use disposable data to check UID/GID `1000:1000`, `/api/health`, task/note access, and persistence across container recreation. Keep runtime file ownership and the loopback host binding intact. Do not pull/deploy the new `latest` alias or touch existing data as part of verification. Native/emulated execution support, image build availability, and live publication are separate results; record any unavailable checks explicitly.
