# RFC 0004: Digest-pinned external bundles and Git provenance

## Preamble

- **RFC number:** 0004
- **Status:** Proposed
- **Created:** 2026-08-26
- **Target:** Later external bundle acquisition and provenance
- **Depends on:** RFC 0002 and RFC 0003

## 1. Summary

This RFC defines the later network boundary for Netsuke manifest bundles. It
adds external Git bundle sources only after repository-relative includes and
versioned local bundles have stable semantics.

An external bundle reference must identify an exact Git object and an expected
canonical bundle digest. A human-friendly tag may be used as the requested
reference, but Netsuke resolves that exact tag to immutable object identifiers,
peels annotated tags deterministically, verifies bundle metadata and content,
and records the result in a lock file.

Branches, default branches, `HEAD`, unqualified symbolic revisions, and mutable
version-only references are not reproducible sources and are rejected by the
initial external resolver.

The central rule is:

> A Git tag is a discovery handle, not the trust anchor. The lock record and
> canonical content digest bind the selected bytes.

The initial supported external source is a bounded Git repository and optional
bundle subdirectory. Acquisition is explicit, cacheable, offline-verifiable,
and observable through redacted provenance metadata.

## 2. Sequencing

This RFC is intentionally third in the composition sequence:

1. RFC 0002 defines deterministic local source composition.
2. RFC 0003 defines bundle identity, versions, parameters, exports, canonical
   content digests, and locks without networking.
3. This RFC adds network acquisition, exact Git tag resolution, cache policy,
   and external provenance.

Implementation must not begin by adding `git clone` to the include resolver.
The local semantic model must exist first so remote acquisition supplies bytes
to an already-defined bundle verifier rather than defining bundle meaning by
accident.

Acceptance of this RFC may occur during v0.2.0 design work. External acquisition
itself may ship later than local includes and local bundles.

## 3. Problem

Reusable quality-gate and workflow bundles become much more valuable when
repositories can consume a reviewed upstream release rather than copying or
vendoring every update manually.

A naïve Git include creates several hazards:

- a branch or tag may move;
- annotated and lightweight tags resolve differently;
- short revision names may be ambiguous;
- a repository may contain multiple bundles or unrelated files;
- submodules can trigger additional network access;
- credentials may leak through URLs or diagnostics;
- a shallow fetch may resolve a different object set from a full clone;
- cached content may become detached from its origin;
- Git object identity alone does not specify path normalization or the selected
  bundle subdirectory; and
- network availability can make graph generation nondeterministic.

A version requirement such as `^1.4` is also insufficient by itself. Semantic
version metadata is authored by the bundle publisher and does not prove which
bytes were selected.

Netsuke needs a resolver that allows convenient tagged releases while binding
execution to immutable, digest-verified content.

## 4. Goals and non-goals

### 4.1 Goals

This RFC aims to:

- resolve exact Git tags, including annotated and lightweight tags;
- support exact commit references as a lower-level alternative;
- peel annotated tags and record both tag-object and commit identities;
- require a canonical bundle digest for external content;
- verify bundle identity, declared semantic version, and compatibility metadata;
- record complete bounded provenance in a versioned lock file;
- provide content-addressed caching and deterministic offline reuse;
- make network access explicit and policy-controlled;
- reject mutable or ambiguous revisions;
- avoid shelling out to Git pipelines; and
- keep credentials and absolute cache paths out of manifests and diagnostics.

### 4.2 Non-goals

The initial external resolver does not:

- resolve branches, remote default branches, `HEAD`, or date-based revisions;
- execute arbitrary Git credential helpers without explicit policy;
- initialize submodules;
- use Git LFS smudge filters;
- run repository hooks, filters, or checkout scripts;
- verify publisher identity solely from a Git tag;
- define a public multi-tenant bundle registry;
- provide dependency solving across unrelated bundle graphs;
- permit network access during `netsuke help targets` without a resolved cache;
- mutate the lock file during an ordinary build; or
- treat transport encryption as content authentication.

Signed-tag and transparency-log policy may be layered on later. The mandatory
initial trust anchor remains the canonical digest in reviewed repository state.

## 5. Manifest syntax

An external bundle import extends RFC 0003's `source` union:

```yaml
bundles:
  - source:
      git:
        url: https://github.com/df12/netsuke-bundles.git
        tag: rust-quality/v1.4.2
        subdir: bundles/rust-quality
        digest: sha256:4e9f2d...
    version: "=1.4.2"
    as: rust
    with:
      features:
        - serde
        - tracing
```

An exact commit may replace the tag:

```yaml
source:
  git:
    url: https://github.com/df12/netsuke-bundles.git
    commit: 8c5e4e6d76d75de67f66f09cb2c63d69d2e14f84
    subdir: bundles/rust-quality
    digest: sha256:4e9f2d...
```

Exactly one of `tag` or `commit` is required.

### 5.1 Git source fields

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `url` | canonical Git URL | required | Repository transport identity. |
| `tag` | exact tag name | one revision required | Human-friendly exact tag to resolve. |
| `commit` | full object ID | one revision required | Exact commit to use directly. |
| `subdir` | relative directory | repository root | Bundle directory within the selected tree. |
| `digest` | algorithm-qualified digest | required | RFC 0003 canonical bundle content digest. |
| `tag_object` | full object ID | absent | Optional reviewed assertion for an annotated tag object. |
| `peeled_commit` | full object ID | absent | Optional reviewed assertion for the resolved commit. |
| `allow_shallow` | Boolean | `true` | Permit a bounded exact-ref fetch when supported. |

Table 1: External Git source fields.

`tag_object` and `peeled_commit` are normally written to the lock file rather
than hand-authored. Supplying them in the manifest is an additional assertion,
not a replacement for the digest.

Unknown keys are errors. Jinja is forbidden in `url`, `tag`, `commit`, `subdir`,
and `digest`.

## 6. Exact tag resolution

### 6.1 Reference normalization

A short tag value such as:

```text
rust-quality/v1.4.2
```

is normalized only to:

```text
refs/tags/rust-quality/v1.4.2
```

It is never searched across branches, remote-tracking refs, notes, pull-request
refs, or other namespaces.

A value already beginning with `refs/tags/` is accepted after validation. Other
`refs/` namespaces are rejected in the `tag` field.

Tag names must satisfy Git's ref-format rules, contain no NUL, and be bounded in
length. Revision expressions such as `^{}`, `~1`, `^1`, `:path`, or reflog
selectors are not tag names and are rejected.

### 6.2 Fetch contract

Netsuke requests the exact normalized tag ref from the configured repository.
It must not fetch every branch or rely on the remote's default ref.

The resolver may use an embedded Git implementation or a tightly controlled Git
subprocess adapter. In either case it must:

- avoid shell command construction;
- pass arguments as an argv vector;
- disable hooks and checkout filters;
- avoid writing into the user's working tree;
- bound object, pack, and total transfer sizes;
- enforce configured transport and host policy;
- redact credentials from logs; and
- report the exact requested ref and repository identity.

### 6.3 Annotated tags

When the fetched tag ref points to a tag object, Netsuke records:

- the tag object ID;
- the tag's declared target type and target object ID;
- the complete peel chain, bounded to reject cycles or unreasonable depth; and
- the final commit ID.

The initial implementation accepts tag chains only when every hop is a tag
object and the final object is a commit. A tag ultimately naming a tree, blob,
or non-commit object is rejected.

### 6.4 Lightweight tags

When the tag ref points directly to a commit, the tag is lightweight. The lock
record stores no tag-object ID and records the direct commit as the peeled
commit.

Human and JSON output distinguish annotated and lightweight tags. Netsuke does
not imply that either form is cryptographically signed.

### 6.5 Retagging

If a requested tag resolves to a different object than the lock record, normal
build and verification fail even when the resulting canonical bundle digest is
unchanged.

An explicit update operation may review and record the new tag and commit
identities. The update still requires the manifest digest to match or an
explicit digest change in reviewed source.

This detects retagging as a provenance event rather than silently accepting it.

## 7. Exact commit resolution

The `commit` form requires the full object ID appropriate to the repository's
hash algorithm. Abbreviated IDs are rejected.

The selected object must be a commit. Netsuke fetches that exact object through
a bounded protocol where the server supports it. If the server refuses
unadvertised object fetches, the diagnostic must explain the limitation rather
than broadening the fetch to mutable branches automatically.

The lock record notes that no tag was requested. Bundle semantic version checks
still apply to the selected subdirectory's descriptor.

## 8. Bundle verification

After resolving a commit, Netsuke reads the selected tree without performing a
normal checkout where practical.

It then:

1. validates `subdir` as a repository-relative, non-escaping path;
2. locates `NetsukeBundle.yaml` in that directory;
3. parses and validates the RFC 0003 descriptor;
4. verifies bundle name and semantic version against the import;
5. verifies Netsuke and manifest-format compatibility;
6. computes the RFC 0003 canonical content digest over reachable bundle
   content; and
7. compares it with the mandatory manifest digest and lock record.

A matching Git commit with a mismatched canonical digest fails. A matching
digest under a different unexpected tag or commit also fails normal lock
verification.

The digest binds the selected bundle content rather than the complete upstream
repository. Unrelated documentation or bundles elsewhere in the repository do
not alter it unless the selected bundle declares them as reachable resources.

## 9. Lock-file provenance

One conceptual external lock record is:

```yaml
bundles:
  rust:
    name: df12.rust-quality
    version: 1.4.2
    source:
      kind: git
      url: https://github.com/df12/netsuke-bundles.git
      requested_tag: refs/tags/rust-quality/v1.4.2
      tag_kind: annotated
      tag_object: 91ca...
      peeled_commit: 8c5e4e6d76d75de67f66f09cb2c63d69d2e14f84
      subdir: bundles/rust-quality
    parameters_sha256: sha256:8b25...
    content_sha256: sha256:4e9f2d...
```

The lock record also stores:

- repository object-format algorithm;
- canonical bundle-tree format version;
- resolver implementation/version;
- acquisition timestamp only as informational metadata excluded from
  reproducibility comparisons; and
- any selected verification policy result.

Normal builds never update this record. Update is an explicit operation whose
human and JSON output shows old and new tag, commit, version, and digest values.

## 10. Network and offline policy

Network access is opt-in at command or configuration level. The conceptual
modes are:

- `offline`: use only verified content-addressed cache entries;
- `locked`: network may fill a missing cache entry, but every identity and
  digest must match the lock record;
- `update`: resolve requested tags anew and propose lock changes; and
- `refresh`: refetch locked identities without changing selection.

`locked` is the normal connected CI mode. `offline` is the preferred release
rebuild mode after caches or vendored artefacts are provisioned.

`netsuke help targets` and other metadata queries may use a verified cached
bundle. They must not initiate network access merely to list targets unless the
caller explicitly selects a connected metadata mode.

A missing cache in offline mode produces a typed diagnostic naming the bundle
namespace and expected digest.

## 11. Content-addressed cache

External bundle content is stored under a content-addressed key derived from:

- canonical content digest;
- canonical bundle-tree format version; and
- bundle metadata schema version.

Repository transport data and Git object packs may use a separate bounded cache
keyed by canonical repository identity and object IDs.

Cache entries are written atomically, owner-protected, and verified before use.
A cache hit never bypasses descriptor, version, digest, or lock validation.

Concurrent processes coordinate through leases rather than trusting partial
files. Stale temporary entries are cleaned through bounded scans.

Cache locations are configuration, not manifest semantics, and never appear as
absolute paths in generated metadata.

## 12. Repository identity and URL handling

Netsuke canonicalizes repository identity without rewriting across unrelated
hosts or protocols. The initial accepted URL forms should be explicit, for
example:

- `https://host/owner/repository.git`;
- `ssh://user@host/path/repository.git`; and
- a configured enterprise Git transport.

SCP-like syntax may be deferred unless a robust parser is available.

Credentials embedded in URLs are rejected. Authentication enters through a
credential provider or transport configuration at the composition root.
Credentials are never written to the lock file, cache metadata, diagnostics,
telemetry, or generated plans.

Redirect policy is bounded and records the final canonical repository identity.
Cross-host redirects require explicit policy.

## 13. Submodules, LFS, and repository features

The initial resolver does not initialize submodules and treats Gitlinks within
the selected bundle content as unsupported reachable entries.

Git LFS pointer files remain ordinary bytes. Netsuke does not run smudge
filters. A bundle requiring LFS materialization is rejected unless a later RFC
defines an explicit, digest-verifiable LFS source contract.

Repository hooks, clean/smudge filters, sparse-checkout configuration, and
attributes that invoke external processes are disabled or ignored during
acquisition.

The canonical bundle digest is computed from selected Git tree content under
Netsuke's own normalization, not from a user checkout affected by filters.

## 14. Trust and verification

The mandatory trust chain is:

1. reviewed manifest source names a repository, exact tag or commit, semantic
   version requirement, subdirectory, and canonical digest;
2. lock source records exact resolved object identities;
3. acquisition retrieves those objects under bounded transport policy;
4. bundle metadata and compatibility are validated; and
5. canonical selected content must match the reviewed digest.

An annotated tag signature may provide additional publisher identity. A future
policy may require:

- a valid signature from an allowed key;
- an allowed signing identity;
- a transparency-log inclusion proof; or
- a repository-host attestation.

Signature verification is additive. It does not replace digest verification,
because a validly signed tag can still point to content different from what the
consumer reviewed.

## 15. Provenance and agent-facing output

Human inspection should show a compact provenance summary:

```text
rust: df12.rust-quality 1.4.2
  git tag rust-quality/v1.4.2
  commit 8c5e4e6d76d75de67f66f09cb2c63d69d2e14f84
  digest sha256:4e9f2d...
  lock verified, cache hit
```

JSON output may include:

- requested and normalized tag;
- annotated/lightweight classification;
- tag-object and peeled-commit IDs;
- repository object format;
- subdirectory;
- bundle identity/version;
- content and parameter digests;
- lock status;
- cache status;
- network mode; and
- verification-policy result.

Metrics use bounded categories only. Repository URLs, tags, commits, bundle
names, paths, and digests must not become unbounded metric labels.

## 16. Failure model

Typed errors should distinguish:

- invalid or unsupported URL;
- disallowed credentials or transport;
- network disabled;
- exact tag absent;
- ambiguous or invalid tag name;
- unsupported tag target or excessive peel depth;
- exact commit unavailable;
- object-format mismatch;
- transfer or object-size limit exceeded;
- unsupported Gitlink/submodule;
- missing bundle descriptor or invalid subdirectory;
- semantic version mismatch;
- bundle compatibility mismatch;
- canonical digest mismatch;
- lock tag or commit mismatch;
- cache corruption; and
- verification-policy failure.

Diagnostics must redact credentials and avoid dumping remote protocol payloads.

## 17. Compatibility and migration

RFC 0002 includes and RFC 0003 local bundles remain entirely local and do not
acquire network behaviour.

A vendored local bundle may migrate to an external source by:

1. publishing the unchanged bundle under a reviewed Git tag;
2. computing its canonical RFC 0003 digest;
3. replacing the local source with the Git source;
4. retaining the same namespace, parameters, bundle identity, and semantic
   version assertion;
5. generating and reviewing the external lock record; and
6. proving graph equivalence before removing the vendor copy.

The external source syntax requires an additive manifest-format minor version.
Older Netsuke versions must reject it clearly.

## 18. Implementation plan

### Phase 0: prerequisite stability

- Accept and implement RFC 0002 local includes.
- Accept and implement RFC 0003 bundle descriptors, parameters, exports,
  canonical digests, and locks.

### Phase 1: Git object resolver

- Parse and validate exact Git source syntax.
- Implement canonical tag normalization and exact ref fetch.
- Support annotated-tag peeling and lightweight tags.
- Support exact full commit references.

### Phase 2: bounded acquisition

- Add transport policy, limits, redaction, and owner-protected temporary state.
- Disable hooks, filters, submodules, and ambient checkout behaviour.
- Read selected tree content without a normal checkout where practical.

### Phase 3: verification and locking

- Feed selected content through the RFC 0003 verifier.
- Add complete external provenance to lock records.
- Detect retagging and object-identity drift.

### Phase 4: content-addressed cache and offline mode

- Add atomic verified caches, leases, and bounded stale cleanup.
- Implement offline, locked, update, and refresh modes.

### Phase 5: policy and canaries

- Add optional signed-tag policy behind explicit configuration.
- Consume one digest-pinned tagged bundle from a downstream canary and reproduce
  its graph in offline mode.

## 19. Test strategy

The implementation must include:

- exact lightweight and annotated tag resolution;
- nested annotated tag peeling and bounded-depth rejection;
- invalid tag names and revision-expression rejection;
- tag-to-non-commit rejection;
- exact commit success and abbreviated-ID rejection;
- retagging detection with matching and differing content digests;
- semantic version and descriptor mismatch tests;
- subdirectory traversal and symlink/Gitlink boundary tests;
- digest verification over selected content only;
- credentials and redirect policy tests;
- transfer, object, pack, and repository-size limits;
- disabled hooks, filters, submodules, and LFS behaviour;
- lock update snapshots showing old/new provenance;
- cache corruption, atomic publication, concurrency, and stale cleanup;
- offline cache-hit and cache-miss behaviour;
- metadata queries that never initiate implicit network access;
- SHA-1 and SHA-256 Git object-format fixtures where supported; and
- end-to-end tagged Git resolution against a local protocol test server rather
  than the public internet.

Property tests should cover tag normalization, ref rejection, lock comparison,
and bounded provenance serialization.

## 20. Alternatives considered

### 20.1 Branch references

Branches are mutable and make ordinary builds perform change detection. Rejected
for the initial reproducible source model.

### 20.2 Tag without digest

A tag can be retargeted and says nothing about Netsuke's selected-subtree
canonicalization. Rejected.

### 20.3 Commit without digest

A commit is immutable in ordinary Git semantics, but the digest also binds the
bundle subdirectory and canonical reachable-resource model independently of Git
transport and object format. Exact commits remain supported, but the bundle
digest is still required.

### 20.4 Download release archives

Archives can work but have host-specific generation, redirect, content-type,
and path-normalization concerns. Git tagged resolution provides stronger object
provenance for the initial external source. A digest-pinned archive source may
be proposed separately.

### 20.5 Trust signed tags alone

Signatures identify a signer under a policy; they do not prove the consumer
reviewed the selected bytes. Rejected as a replacement for digest pinning.

### 20.6 Use a normal checkout

A checkout can invoke filters, depend on user configuration, materialize
submodules, and introduce filesystem metadata. Rejected as the normative
verification representation.

## 21. Open questions

- Which embedded Git implementation or controlled subprocess boundary best
  satisfies SHA-256 repository support and bounded exact-ref fetches?
- Should annotated tags be required by policy for organization-owned bundles?
- Which stable command vocabulary should own lock updates and cache refreshes?
- Should external bundle caches share infrastructure with `fetch()` resources
  or remain isolated by trust class?
- Should a later registry map semantic versions to digest-pinned Git tags, or is
  direct repository metadata sufficient?
- How should enterprise-host identity and credential-provider configuration be
  represented without entering manifest semantics?

## 22. Recommendation

Adopt external Git bundles only as digest-pinned, lock-recorded acquisitions over
RFC 0003 bundle semantics.

Support exact human-friendly tags, including annotated and lightweight tags,
but resolve them to immutable object identities and treat retagging as a
reviewable provenance change. Reject branches and implicit network access.

This gives Netsuke tagged Git resolution without asking users to pretend that a
tag is immutable, signed, or sufficient on its own.