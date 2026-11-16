# Pinned package association recovery

- [When to use recovery](#when-to-use-recovery)
- [Approval and trusted pins](#approval-and-trusted-pins)
- [Verification and allowed effects](#verification-and-allowed-effects)
- [Refusals and outcomes](#refusals-and-outcomes)

## When to use recovery

Use only after separate approval to finish association for an accepted older publication. Changed publisher inputs cannot claim its release identity; rebuilding or recreating tags is not recovery.

Needs Bash, Python 3.11+, Git, curl, trusted HTTPS, and current protected credentials with real API/repository/registry permissions. It does not need Docker/Buildx or registry login. A read-only token cannot create a link. See [credentials](release.md#credentials-and-tools) and the [server contract](package-association-contract.md).

## Approval and trusted pins

Use clean, already-pushed current server `main` with exact configured destinations. Historical source must be an ancestor. Exclude all other publication/package/association writers and in-flight requests; source pushing remains separate.

Independently verify the accepted annotated tag object and full source commit. `git ls-remote --tags origin refs/tags/v0.2.6 'refs/tags/v0.2.6^{}'` reads advertisements, not trust authority. Set reviewed lowercase 40-character IDs, then run:

```sh
./scripts/release.sh --associate-existing v0.2.6 --expected-tag-oid "$EXPECTED_TAG_OID" --expected-source "$EXPECTED_SOURCE"
```

Replace the example version as appropriate. Flags/values are required in that order; malformed, reordered, duplicated, or extra arguments refuse before credentials/effects. Help and local validation remain offline.

## Verification and allowed effects

Current tools check clean/pushed main, destinations, bounded inventories, exact supplied pins, historical settings/scripts/notes, canonical tag bytes, release metadata, immutable two-platform manifests/labels, and current `latest`. Historical code is never executed/imported. Current credentials are used, not historical token settings. Fetching the accepted object may populate Git objects without moving refs.

Fresh identity/publication checks repeat immediately before the permitted missing-link POST and after readback. Only the [one-link contract](package-association-contract.md#write-authority-and-limits) authorizes it. Correct existing links/repeats are read-only.

No build, tag push, release creation, promotion, upload, unlink, reassignment, rollback, or cleanup occurs. Accepted images/tags/releases/aliases and working files remain unchanged; private temporary/lock state and Git object population are expected local effects.

## Refusals and outcomes

Wrong pins, non-ancestor source, unsafe/noncanonical inputs, changed tools, partial/conflicting inventories, digest/label/latest drift, unsupported servers, failed reads, or ambiguous association stop without automatic repair. A changed alias is not promoted back.

`Association verified` appears only after final checks. Nonzero may follow an accepted link; inspect actual state before another separately approved action. A lost POST allows readback, never blind replay. Do not delete or republish artifacts. Curated stderr stages/waiting events hide raw output, credentials, headers, and private bodies. Fixtures do not attest live permissions, installed binaries, UI behavior, or native ARM64 runtime compatibility.
