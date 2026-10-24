# Publication and recovery

- [Sequence and identity](#sequence-and-identity)
- [Retries and partial failures](#retries-and-partial-failures)
- [Package association](#package-association)
- [Local hardening](#local-hardening)
- [Resource limits and authority](#resource-limits-and-authority)

## Sequence and identity

After [configuration](release-configuration.md), a separately approved `./scripts/release.sh` invocation publishes real remote changes. It requires clean, already-pushed server `main`, exactly one verified effective URL in each fetch/push direction, a prepared two-platform builder, and exclusive writers to the target repository/image/package. Multiple destinations refuse, since Git can push to every configured push URL. A checkout filesystem lock is not distributed writer exclusion. Source is never pushed automatically.

The workflow validates committed notes, isolates authentication, and completes bounded release/tag inventories before publication. For a fresh version it captures predecessor `latest`, reuses matching verified staging or builds from a private committed snapshot, verifies both manifests/configuration labels by immutable digest, pushes the exact deterministic annotated Git tag without force, promotes the prepared digest to `latest` without rebuilding, creates the notes-only repository release, then verifies publication and package association before success. Ignored/untracked secrets are excluded from the committed snapshot and narrow Docker context.

The `container-release-bash/1` annotation binds version, captured commit, repository, configuration/script hashes, platforms, image digest, predecessor, and committed notes path/hash. Canonical bytes and fixed tagger identity/time make the intended object reproducible. Image labels alone or a matching version string do not establish ownership. A release API call may not silently create or repair a missing confirmed Git tag.

As local hardening beyond the pinned reference, the explicit release-tag push uses `--no-follow-tags` for that invocation. An enabled `push.followTags` preference cannot add unrelated annotated tags to the push. Caller Git configuration remains unchanged; non-force publication and lost-response reconciliation retain their existing behavior.

## Retries and partial failures

Absent tag and release may permit reuse of verified staging. A lost build response without recoverable staging may require another explicit build attempt; exactly-once builds are not promised. A running invocation can reconcile its own lost tag/promotion/release response from fresh exact evidence, never by blindly repeating a confirmed write.

A complete matching publication is reverified without staging lookup, build, promotion, tag push, or release POST. Its immutable bound content and `latest` must still match. A completed retry's only allowed write is finishing a proven missing package association under the rules below. Changed source/config/scripts/notes, missing immutable content, foreign final aliases, and conflicting local/remote tags refuse rather than repair.

After tools/configuration/version change, a no-argument invocation retains those strict identity rules. To finish only an older verified publication's association, use [explicit source/tag-pinned recovery](package-association-recovery.md). It reads historical committed inputs as bounded data, never executes them, verifies old canonical identity and current tool stability, and permits only the scoped association POST. It does not require Docker or republish content.

Tag-only and release-only startup states are ambiguous without a durable journal. They stop before publication writes even if an earlier process was interrupted. Inspect the exact remaining objects with the original source/configuration; manual completion requires separate approval. Never delete records, force-push tags, roll back aliases, reassign packages, or change identity and retry to manufacture success.

Failure returns nonzero and identifies the incomplete/uncertain phase, preserving accepted remote state. Traps clean only run-owned temporary authentication files, not remote objects. Bounded command stderr is intentionally not relayed raw because it may contain secrets; this replaces the old publisher's streaming build-output behavior. Inspect trusted local tool diagnostics separately when necessary.

Progress is flushed to stderr at phase transitions with elapsed time. Long bounded operations emit a fixed waiting message at most once every ten seconds while pipes remain drained and deadlines/byte limits enforced. No raw arguments, credentials, API bodies, or child stdout/stderr are relayed. A lost write response is reported as uncertain until exact readback confirms accepted state; publication verification is distinct from completed association. Final references remain on stdout.

## Package association

Only after publication verification, read the exact target repository and `GET /api/v1/packages/{owner}/container/{image-name}/latest`. This is literal `latest`, not the `/-/latest` selector. Require exact owner/name/type/version, positive IDs, and an explicit `repository` field. An exact existing link causes no POST. A conflicting link refuses; source labels and display URLs do not authorize reassignment.

JSON null can conceal an inaccessible existing link. A new link therefore additionally requires `GET /api/v1/version` to identify supported stable Gitea `1.25.x` or exact stable `28.2.0`, whose reviewed services refuse already-linked packages. See the [source-pinned contract](package-association-contract.md). Other families, nearby unreviewed versions, development/build-suffixed versions, malformed/failed reads, and missing fields never authorize a POST. Refresh identities, send at most one bodyless `POST /api/v1/packages/{owner}/container/{image-name}/-/link/{repo-name}`, require empty HTTP 201, then read back the exact association. A lost transport response permits one read-based reconciliation, not another POST. Input/resource/HTTP-status refusals are fatal, not replay authority.

Association affects every version of the package. Operators must exclude other association writers, UI actions, and in-flight requests through completion; preflight checks and the local lock are not atomic compare-and-set. New server families require separate refusal-semantics analysis, not a relaxed gate. Final publication/link checks precede `Publication verified`. Failure leaves any accepted image/tag/release/link intact and can leave a completed publication awaiting supported association. Local tests do not certify account permissions, deployed API behavior, or Packages-tab rendering.

## Local hardening

Upstream pin `3c73528231ac1cbdd43600a5ca723390db804bac` documents two unresolved warnings. This server port addresses both locally; it does not claim upstream changes:

- N1: promotion confirmation is one-way. Before promotion, guards accept the captured predecessor or prepared digest. After confirmation, they require only the prepared digest. A predecessor reappearance, missing/foreign alias, or failed read stops before release creation or association without repromotion or rollback.
- N2: before private authentication, capture the Docker CLI's selected Buildx executable path, version, and hash. Reference that executable through a run-owned private `cli-plugins/docker-buildx` symlink and verify the same executable under the private configuration before publication writes. The executable is not copied or installed globally. Config-only and symlinked installations remain usable; a changed or unidentifiable selection refuses instead of silently switching to a system plugin. This adds a Docker client-info query and readable local executable as explicit prerequisites.

Rust-specific local validation also enforces `VERSION`/Cargo/README/notes agreement and deprecated publishing overrides fail before effects. See the [verification boundaries](release-verification.md); fake command-port tests alone do not certify native Docker routing or executable discovery on every installation.

## Resource limits and authority

API responses and OCI index/manifests are limited to 4 MiB, OCI configuration/token responses to 1 MiB, notes/annotations/HTTP headers to 64 KiB, HTTP status to 16 bytes, stderr to 1 MiB, and Git ref advertisements to 16 MiB. Bounds apply while receiving. Complete exact-limit input is accepted; one extra byte refuses rather than parsing a truncated prefix.

An inventory shares limits of 1,002 pages, 100,000 records, 64 MiB cumulative payload, and 600 seconds. Each listing needs a terminal empty page; repetition, duplicate identities, missing termination, or exhausted budgets block the next write. Reads have 60-second deadlines and builds 7,200 seconds. These are decoded evidence/process limits, not Docker storage, arbitrary Git pack, runtime, or power-loss guarantees.

TLS stays enabled. Authenticated requests disable ambient curl configuration, do not follow redirects, and require same-origin registry token endpoints. No retention, archive uploads, package reassignment, privileged setup, local-image removal, or managed updates run. The workflow cannot make Git, registry, and repository APIs atomic or exclude writers in other checkouts; operator serialization and accurate partial-state reporting remain prerequisites.
