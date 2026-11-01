# Architectural decision record (ADR) 0068: Typed records at release policy boundaries

## Status

Accepted on 2026-10-30. Release policy decides from frozen, slotted records at
every boundary where an external record shape reaches it; mappings remain only
where the policy decides over a domain-shaped name-to-value relation that no
external format defines.

## Date

2026-10-30

## Context and Problem Statement

Pull request [#903](https://github.com/leynos/netsuke/pull/903) moved release
publication onto a set of Python policy modules and adapters. Its policy and
adapter split is enforced by `tests/workflow_contracts/`, which rejects
network, filesystem, serialization, HTTP, and provider-format effects in the
modules listed as policy. CodeRabbit's Domain Architecture review of that pull
request found one remaining leak: `scripts/release_declaration.py` interpreted
the TOML-shaped `{"targets": ...}` mapping, read the `platform` member, and
raised configuration errors from inside a policy module. The review asked for
domain-shaped types such as `StagingTargets`, `ReleaseIdentityRecord`, and
`DigestRecord`, with TOML and JSON adapters owning field names, shape checks,
decoding, encoding, and malformed-record errors.

The maintainer accepted the finding as valid but deferred it out of #903 so
that the trusted-publishing work could land. The deferral is recorded in the
decision log of the
[trusted packaging ExecPlan](../execplans/use-trusted-packaging.md), and this
issue is its follow-up.

The code already contains the pattern the review asked for.
`release_candidate.ReleaseIdentity`, `release_inventory.PublishedAsset`, the
`release_reconcile_crates` registry observations, and every record under
`.github/scripts/_release_admission/` are frozen slotted dataclasses. The
question is therefore not whether the repository has a record convention, but
where the convention should stop: which remaining mappings are external record
shapes that an adapter should decode, and which are domain values that policy
may legitimately decide over.

## Decision Drivers

- Keep release identity exact. Commit identity stays an exact string
  comparison, and tag/version agreement is not relaxed.
- Keep inventory checks exhaustive. Every declared, trusted, and downloaded
  asset remains compared, including the empty-set rejections.
- Give each external record shape exactly one decoding location, owned by an
  adapter or CLI that already handles its serialization.
- Preserve every existing rejection category and its type, so operators keep
  the same diagnosis and the same exit status.
- Remove the duplicated staging-config decode without weakening the check that
  refuses a malformed `targets` table before discovery iterates it.
- Keep the migration bounded, so the change is reviewable and its test
  migrations are mechanical.
- Do not weaken the `POLICY_MODULES` boundary contract. Policy modules stay
  network-free and free of filesystem and serialization effects.

## Options considered

### Option A: retain the mapping boundary

Policy keeps reading mappings as it does today. The cost is zero now, and the
boundary remains as #903 left it. The cost is permanent: `assets`, `targets`,
and `platform` stay policy vocabulary, the staging configuration is decoded
twice, and a "validated mapping" is a call-order convention rather than a type
obligation. A future caller can pass an unvalidated mapping to a digest
comparison and skip the digest-format check entirely.

### Option B: decode at the edge into frozen slotted records

Adapters and CLIs decode each external shape into a frozen slotted record, and
policy functions accept the record. Field names, shape checks, and malformed
record errors move to the adapter that already owns the file or stream. Policy
keeps deciding what the record means: identity agreement, inventory coverage,
and digest comparison.

This matches the repository's established value-object convention and satisfies
the boundary contract without changing what any refusal protects.

### Option C: `TypedDict` only

A `TypedDict` is a static annotation. It has no runtime validation, no single
decoding location, and no way to stop an unvalidated mapping from reaching a
comparison. It would satisfy a type checker while leaving every runtime gap in
Option A in place, and it would add a second, weaker convention beside the
frozen dataclasses already in use.

| Criterion                                | Option A           | Option B             | Option C |
| ---------------------------------------- | ------------------ | -------------------- | -------- |
| One decoding location per external shape | No                 | Yes                  | No       |
| Runtime rejection preserved              | Yes, by convention | Yes, by construction | No       |
| Removes the staging double decode        | No                 | Yes                  | No       |
| Type-enforced validated record           | No                 | Yes                  | No       |
| Fixture migration                        | None               | Bounded              | None     |
| Consistent with existing records         | Partly             | Yes                  | No       |

Table 1: Comparison of the three boundary options against the decision criteria.

## Decision Outcome

Adopt Option B, narrowed to the external-shape seams. `TypedDict` is rejected
as a static-only mechanism that leaves the runtime gap in place.

The adopted scope is:

- `release_declaration` accepts a `StagingTargets` record built with
  `StagingTarget` entries. The TOML walk, including the `targets` and
  `platform` member names, moves to `release_declaration_cli`, which decodes
  the configuration once and passes the record to both existing checks.
- `release_digest_record` accepts a `DigestRecord` for the trusted side of its
  comparisons. The JSON `assets` walk moves to `release_digest_record_files`,
  which already owns the file and its serialization.
- `release_identity_record` is reclassified from policy to adapter in the
  boundary contract. Its field names are an external record shape, and no
  production caller currently depends on it being policy.

The following remain mappings because they are domain values rather than
external record shapes:

- `release_reconcile_github.plan_asset_actions` takes name-to-digest mappings
  produced by the declaration and by GitHub's listing adapter. Neither is a
  provider payload by the time the policy sees it.
- The computed-downloads side of `require_downloads_match_record` is derived by
  the verifier from bytes it hashed. Only the trusted side is a decoded
  external record, so the asymmetry records provenance.
- `PublishedAsset`, `ReleaseIdentity`, and the registry observations are
  already frozen slotted records and keep their current shape.

Every existing error type is retained: `MissingTargetsError`,
`MalformedTargetError` with `MalformedTargetKind`, `DigestRecordError`, and
`RecordedDigestMismatchError`. The migration moves where they are raised, not
what they mean.

## Consequences

- Each external record shape has one decoding location, and a policy function
  can no longer receive an undecoded staging table or digest payload through
  its normal path.
- The staging configuration is decoded once per invocation. The pre-check that
  refuses a malformed table before discovery runs becomes a decoded value
  rather than a second pass over the same object.
- `release_identity_record` appears in `ADAPTER_MODULES` and the boundary
  documentation describes it as a decoder. Its behaviour and error types do not
  change.
- Payload-shape tests move from the policy tests to the decoder tests.
  Policy tests construct records directly, so they test the decision rather
  than the decoding.
- Adding a new external record shape now requires a decoder at the adapter and
  a record beside the policy that consumes it. The record's constructor owns
  the invariant, and the adapter owns the wire names.
- The containment is deliberate: domain-shaped mappings remain, so the
  decision does not turn every name-to-value relation into a class. A future
  change that makes one of those mappings an external payload must add a
  decoder at that boundary.

## References

- [Issue #964: Evaluate typed records for release policy boundaries](https://github.com/leynos/netsuke/issues/964)
- [Pull request #903](https://github.com/leynos/netsuke/pull/903)
- [CodeRabbit Domain Architecture finding](https://github.com/leynos/netsuke/pull/903#discussion_r6448190491)
- [Trusted packaging ExecPlan, decision log](../execplans/use-trusted-packaging.md)
- [RFC 0005: Harden release integrity and admission](../rfcs/rfc-0005-harden-release-integrity-and-admission.md)
- [ADR-0020: Bounded release admission observability](adr-0020-bounded-release-admission-observability.md)
- [Developers' guide: release policy and adapter boundaries](../developers-guide.md#release-publication)
