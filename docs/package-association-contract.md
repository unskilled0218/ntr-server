# Package association contract

- [Supported deployments](#supported-deployments)
- [Pinned source evidence](#pinned-source-evidence)
- [Write authority and limits](#write-authority-and-limits)

## Supported deployments

New-link writes support stable Gitea `1.25.x` and exact stable `28.2.0`, not adjacent/build-suffixed/other families. Correct visible links may verify read-only without this gate. There is no bypass or reassignment option.

The observed `git.lan` API/footer matched Gitea 28.2.0. Upstream annotated tag `v28.2.0` (`1dc1ecdddcdc60ab14ec14e30e7bb4d84242f21f`) points to `829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1`. This is source/contract evidence, not attestation of the installed binary. Custom/forked builds need separate provenance/behavior review before writes.

## Pinned source evidence

Reviewed source at that commit establishes:

- [Routes](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/routers/api/v1/api.go) and [caller permissions](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/services/context/package.go): scoped package reads/writes and bodyless `POST /{owner}/{type}/{name}/-/link/{repo_name}`.
- [Handler](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/routers/api/v1/packages/package.go): empty 201 on success, 400 for an existing link, 403 for denied permission.
- [Link service](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/services/packages/package_update.go): rejects owner mismatch/existing positive `RepoID` and requires target package-unit write permission. [1.25.0](https://github.com/go-gitea/gitea/blob/v1.25.0/services/packages/package_update.go) has the same refusal checks; unlink is not used.
- [Serializer](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/services/convert/package.go): an inaccessible existing association may appear null. [Model update](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/models/packages/package.go) is not compare-and-set.
- [Upstream tests](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/tests/integration/api_packages_test.go) cover scopes, linking/readback, and hidden evidence. Source review does not mean those tests ran here.

## Write authority and limits

Verify exact repository/package identities, explicit nullable association evidence, and literal package version `latest`, not `/-/latest`. Null alone does not prove absence; the reviewed server must reject a hidden existing link. Unsupported evidence cannot authorize POST.

After fresh identity/publication checks, at most one scoped bodyless POST is permitted. Require empty 201 plus exact readback. A lost response permits readback, not a repeated POST. Unexpected, malformed, oversized, conflicting, or uncertain evidence returns nonzero without removing accepted objects.

Exclude every other package/association writer, including UI actions and in-flight requests. Reads cannot eliminate the service's final non-atomic race. Local fixtures do not certify deployed provenance, permissions, TLS, or UI behavior.
