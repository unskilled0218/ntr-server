# Package association contract

- [Supported deployments](#supported-deployments)
- [Pinned source evidence](#pinned-source-evidence)
- [Write authority and limits](#write-authority-and-limits)

## Supported deployments

New-link writes support stable Gitea `1.25.x` and exact stable Gitea `28.2.0`. Nearby `28.2.1`, broader `28.x`, development/build-suffixed versions, and other families do not inherit that authority. An already-correct visible association can verify read-only without a version gate. There is no version-check bypass or reassignment option.

The observed `https://git.lan` API reports `28.2.0`, its homepage identifies Gitea, and its public API settings match that family. Upstream Gitea has official annotated tag `v28.2.0` (`1dc1ecdddcdc60ab14ec14e30e7bb4d84242f21f`) at commit `829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1`. These observations and source review establish a matching published contract, not attestation of the installed executable. Operators must disclose custom/forked builds and obtain a separate provenance/behavior review before allowing new-link writes. A version string or footer cannot prove modified-server safety.

## Pinned source evidence

The reviewed `28.2.0` source is pinned to `829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1`:

- [API routes](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/routers/api/v1/api.go): package routes require token/package scopes and package read access; linking additionally requires package write access. The POST route is `/{owner}/{type}/{name}/-/link/{repo_name}`.
- [Package context](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/services/context/package.go): access derives from the actual caller/owner and organization permissions, not merely the token's declared scopes.
- [API handler](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/routers/api/v1/packages/package.go): resolves owner-scoped package/repository names, invokes the service, maps existing-link invalid argument to 400 and permission denial to 403, and sends an empty 201 on success.
- [Link service](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/services/packages/package_update.go): rejects owner mismatch, rejects an already-positive `RepoID` before any link write, and requires write permission on the target repository's package unit. The earlier supported [Gitea 1.25.0 service](https://github.com/go-gitea/gitea/blob/v1.25.0/services/packages/package_update.go) has the same link-refusal checks; its unlink behavior is not used.
- [Serializer](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/services/convert/package.go): may omit the actual linked repository from visible evidence by returning null when the caller lacks repository access.
- [Model update](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/models/packages/package.go): updates `repo_id` by package ID, without compare-and-set against zero.
- [Upstream API tests](https://github.com/go-gitea/gitea/blob/829b49c1ae96015e345f9d2a0eb1ea7acd5f31e1/tests/integration/api_packages_test.go): cover read/write package scopes, link/readback, and permission-hidden association evidence. Source inspection is not a claim that these upstream tests or a live server integration suite ran here.

## Write authority and limits

The client verifies exact repository/package identities, explicit nullable association evidence, and literal container package version `latest`, not `/-/latest`. A visible correct link is read-only; a visible conflicting link refuses. Null is not proof that no hidden link exists: the reviewed service must refuse a hidden already-linked package. Unsupported server evidence therefore cannot authorize a new-link POST.

After fresh identities and publication verification, at most one scoped bodyless POST is allowed. Require empty 201 and exact readback. A lost transport response may reconcile once through readback, never by repeating the POST. Unexpected status, malformed/oversized evidence, uncertain state, or conflicting identity returns nonzero and preserves accepted objects.

Association affects every version of the package. Operators must exclude all other package/association writers, UI actions, and in-flight requests. The local checkout lock is not distributed exclusion; service checks followed by an unconditional model update are not atomic compare-and-set. No client read sequence can remove this operational requirement. Tests do not certify account permissions, installed-server provenance, production TLS, or package UI rendering.
