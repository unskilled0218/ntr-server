# Pinned package association recovery

- [When to use recovery](#when-to-use-recovery)
- [Approval and trusted pins](#approval-and-trusted-pins)
- [Verification and allowed effects](#verification-and-allowed-effects)
- [Refusals and outcomes](#refusals-and-outcomes)

## When to use recovery

Use this explicit mode only after separate approval to finish package association for an existing accepted publication. The ordinary no-argument publisher binds its current source/configuration/scripts/notes to its release tag. Upgrading those tools or preparing `0.2.7` does not grant ownership of the old `v0.2.6` identity; changing hashes, recreating tags, or publishing again is not recovery.

Recovery needs Bash, Python 3.11+, Git, curl, normal HTTPS trust, and current protected external credentials. It does not need Docker, Buildx, a running daemon, emulation, or a registry login. API/repository/registry reads and the single missing-link write still require real account permissions; a read-only token cannot create a link. See [credentials](release.md#credentials-and-tools) and the [reviewed server contract](package-association-contract.md).

## Approval and trusted pins

Use clean, already-pushed current server `main` with the exact configured fetch/push destination and target image/repository. Source pushing is a separate operator action; the tool never pushes source. The historical source must be an ancestor of current main. Exclude other publication/package/association writers, UI changes, and in-flight requests until recovery completes.

Independently review the accepted publication and obtain its exact annotated tag object ID and full source commit. A mutable name or copied API field alone is not a trust decision. Fresh Git advertisements can be inspected read-only with `git ls-remote --tags origin refs/tags/v0.2.6 'refs/tags/v0.2.6^{}'`; verify both values against the accepted publication's provenance, not merely their shape.

For reference, unauthenticated planning observations reported tag object `48ef6113e6fb296554193c11a147abcdb0aa002e`, source `7bc9b8c344f4654f90f494e1fe7966d13526bfb5`, and index `sha256:0c61b7b7019da16b683c772ecc68118c58837fe356b1f9119ab0f762e32dc925` for `v0.2.6`. These are historical observations, not built-in authority, a claim of present state, or permission to mutate the live package. Its unauthenticated package API returned 401; association state and real token rights were not established by planning.

After approval and independent verification, set `EXPECTED_TAG_OID` and `EXPECTED_SOURCE` to your reviewed full 40-character lowercase Git IDs, then invoke from the server checkout:

```sh
./scripts/release.sh --associate-existing v0.2.6 --expected-tag-oid "$EXPECTED_TAG_OID" --expected-source "$EXPECTED_SOURCE"
```

All three flags and their values are required in this order. Missing, malformed, duplicated, extra, or reordered arguments refuse before credentials or effects. Help and local validation retain their existing no-credential behavior.

## Verification and allowed effects

The tool captures current source/settings/scripts, checks clean/pushed main and destinations, reads complete bounded inventories, and requires the exact supplied historical tag/source. It may fetch that object without moving local refs. It reads historical settings, version mirrors, notes, and scripts as bounded regular committed blobs; historical code is never executed, sourced, or imported. Current credentials are used, not old token-file settings.

Historical raw settings and ordered script hashes, normalized notes digest, canonical annotated-tag bytes/OID, version/source/target scope, release notes/state, immutable two-platform manifests/configuration labels, and current `latest` must all agree. Current main/tool identity and old publication evidence are checked again immediately before a missing link POST and after association. Only the [reviewed one-link contract](package-association-contract.md#write-authority-and-limits) can authorize a write.

No snapshot, build, Docker bootstrap/login, staging lookup, tag push, release creation, image promotion, upload, unlink, reassignment, rollback, or remote cleanup is performed. A correct existing link and successful repeat require no remote write. Existing image objects, tag, release, aliases, current working tree/configuration, and local refs remain unchanged; local Git object population and private lock/temporary credentials are the only ordinary local state effects.

## Refusals and outcomes

Wrong pins, non-ancestor source, unknown/noncanonical identity, unsafe committed inputs, partial tag/release evidence, historical hash/notes mismatch, changed current tools, missing/foreign immutable image, label mismatch, latest drift, newer/ambiguous publication inventory, unsupported server, failed reads, or conflicting association stop without automatic repair. A changed alias is not promoted back to the old digest. Any accepted link remains intact if later verification fails.

Phase messages and bounded waiting updates are flushed to stderr; raw tool output, credentials, headers, and private response bodies are not relayed. `Association verified` is printed only after final publication/link checks. Nonzero exit means incomplete or uncertain recovery even if some accepted state exists. A lost POST response permits readback, not another blind POST. Do not delete or republish artifacts in response to a refusal; inspect exact evidence and obtain separate approval for any next action.

This workflow does not prove native ARM64 runtime compatibility, eliminate the separate v8 compatibility warning, attest the installed service binary, or certify live account/UI behavior. Local fixture success is not live recovery approval.
