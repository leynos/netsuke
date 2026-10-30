# Issue 964: Evaluate typed records for release policy boundaries

Status: IN PROGRESS

Implementation is complete and verified: the decision is recorded as ADR-0068,
the migration has landed, and the full deterministic gate set is green on the
migration commit `716f9b914`. Review and the draft pull request are
outstanding, so the plan is not yet complete.

## Purpose

Issue [#964](https://github.com/leynos/netsuke/issues/964) asks whether the
release-publication policy should decide from frozen, slotted records instead
of the mappings it currently reads. The question follows a CodeRabbit Domain
Architecture finding on pull request
[#903](https://github.com/leynos/netsuke/pull/903), which observed that
`scripts/release_declaration.py` interprets the TOML-shaped `{"targets": ...}`
mapping and raises configuration errors from inside a policy module. The
maintainer deferred the refactor out of #903 and recorded the deferral in the
trusted-packaging decision log.

This plan answers four questions with evidence from `main` at commit
`0a3104d44` (the merge commit for #903):

1. Where does external record shape currently reach release policy?
2. Which of those checks protect release identity or artefact inventory, and so
   must survive unchanged?
3. What would migrating the affected seams to typed records cost in call sites,
   adapters, and tests?
4. Does the repository's existing evidence meet the decision criteria for
   adoption, and if so, what is the narrowest useful scope?

The evaluation is deliberately bounded. It excludes
`.github/scripts/_release_admission/`, which already uses frozen slotted
records and is the stated reference convention rather than a candidate for
change. It also excludes the `fresh` freshness contradiction and the
trusted-publishing Stage C and Stage D work, both of which remain with their
own issues.

## Constraints

- No production behaviour changes in the evaluation phase. The inventory and
  the decision record are documentation until the migration stage begins.
- Release identity checks stay exact. Commit identity remains an exact string
  comparison, and tag/version agreement is not relaxed.
- Inventory checks stay exhaustive. The declared set, the trusted record, and
  the downloaded set keep their current rejections, including the empty-set
  cases.
- `.github/scripts/_release_admission/` is not modified.
- `tests/workflow_contracts/release_publish_path_*.py` is not modified.
- Trusted-publishing Stage C stays partial and Stage D stays open. This issue
  neither claims nor advances them.
- Policy modules stay network-free and free of filesystem, serialization, and
  provider-format effects, as the existing `POLICY_MODULES` contract requires.
- Python remains the 3.14 baseline, and new code follows the repository's
  frozen slotted dataclass convention for value objects.

## Evidence base and method

The inventory was produced by reading every release source named in
`tests/workflow_contracts/release_publication_modules.py` and by tracing each
policy function to its production callers with `grep -n` over `scripts/`. Test
fixtures were classified by reading each test and asking whether the mapping
under test is (a) an external payload the adapter decodes, or (b) a
domain-shaped value the policy legitimately decides over.

Counts below name the command or the file set they came from. They are census
results for this commit, not invariants. A later commit that adds a call site
invalidates the number, not the conclusion, and the plan records the command so
the census can be repeated:

```sh
grep -rn "platform_target_counts\|declared_asset_names\|validate_digest_record\
\|require_record_matches_declaration\|require_downloads_match_record\
\|identity_from_mapping\|plan_asset_actions\|validate_inventory" \
  --include=*.py scripts
```

## Inventory of policy seams

| Seam              | Interface                                                                                                         | External shape                                                                | Producing adapter                                | Production callers                  | Validation site                           | Kind               | Notes                                                                                 |
| ----------------- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------ | ----------------------------------- | ----------------------------------------- | ------------------ | ------------------------------------------------------------------------------------- |
| Staging targets   | `release_declaration.platform_target_counts(target_config: object) -> dict[str, int]`                             | TOML `{"targets": {name: {"platform": str}}}`                                 | `release_declaration_cli` (`tomllib.loads`)      | `release_declaration_cli:300`       | Policy, via `_mapping` and `_platform_of` | Artefact inventory | Decoded twice: once as a pre-check and again inside `declared_asset_names`            |
| Staging targets   | `release_declaration.declared_asset_names(*, archive_names, target_config: object, staged_package_names)`         | Same TOML mapping                                                             | `release_declaration_cli`                        | `release_declaration_cli:308`       | Policy                                    | Artefact inventory | Re-runs `platform_target_counts`, so a malformed config is rejected twice             |
| Release identity  | `release_identity_record.identity_from_mapping(record: Mapping[str, object]) -> ReleaseIdentity`                  | Identity JSON `{tag, commit, package_name, version, bin_name, is_prerelease}` | None: the module is the decoder                  | None; tests only                    | Policy                                    | Release identity   | `ReleaseIdentity` and `CandidateFacts` already exist as frozen slotted records        |
| Digest record     | `release_digest_record.validate_digest_record(record: object) -> dict[str, str]`                                  | JSON `{"assets": {name: digest}}`                                             | `release_digest_record_files.read_digest_record` | `release_digest_record_files:60,81` | Policy                                    | Artefact inventory | Owns the `assets` member name inside policy                                           |
| Digest comparison | `release_digest_record.require_record_matches_declaration(digests: Mapping, *, expected)`                         | Domain mapping                                                                | `release_digest_record_files`                    | `verify_release_inventory_cli:113`  | Convention only                           | Artefact inventory | Accepts any `Mapping`, so "validated" is not type-enforced                            |
| Digest comparison | `release_digest_record.require_downloads_match_record(computed: Mapping, *, trusted: Mapping)`                    | Domain mapping                                                                | `verify_release_inventory_cli`                   | `verify_release_inventory_cli:115`  | Convention only                           | Artefact inventory | Same; the trusted side is the decoded record and the computed side is adapter-derived |
| GitHub assets     | `release_reconcile_github.plan_asset_actions(*, expected: Mapping[str, str], present: Mapping[str, str \| None])` | Domain mapping of name to digest                                              | `finalize_github_release_cli`                    | `finalize_github_release_cli:263`   | Not applicable                            | Artefact inventory | Domain-shaped, not an external payload. Retain as a mapping.                          |
| Inventory         | `release_inventory.validate_inventory(present: Iterable[PublishedAsset], *, expected)`                            | Already typed                                                                 | `verify_release_inventory_cli`                   | `verify_release_inventory_cli:112`  | `PublishedAsset`                          | Artefact inventory | Already a frozen slotted record. Retain.                                              |
| Registry state    | `release_reconcile_crates.classify_registry_state`, `plan_crates_action`                                          | Already typed                                                                 | `release_crates_io_adapter`                      | `publish_crates_io_cli`             | Typed observations                        | Artefact inventory | Already frozen slotted records. Retain.                                               |

### Repeated validation and unvalidated reads

- `release_declaration_cli` calls `platform_target_counts` as a pre-check and
  then `declared_asset_names` decodes the same `config` object again. Both
  calls can raise the same `MalformedTargetError`. The duplication exists so
  the discovery adapter does not iterate an unvalidated `targets` table; the
  pre-check is a workaround for the fact that the decoder is not a separate
  step.
- `release_identity_record` coerces fields with `str(...)` before calling
  `resolve_identity`. A non-string `tag` or `commit` is therefore normalized by
  the policy decoder rather than rejected at the adapter. The `is_prerelease`
  flag is the one field that is checked for type, because a string `"false"`
  would otherwise be truthy.
- `require_record_matches_declaration` and `require_downloads_match_record`
  declare `Mapping` parameters but perform no record-shape validation. A caller
  can pass a mapping that never passed `validate_digest_record`; the digest
  format check is then skipped. Every current caller does decode first, so this
  is a latent gap rather than an observed defect.
- No policy function performs an unvalidated key read on an external payload
  after this inventory: `release_declaration` validates before `.get`, and
  `release_digest_record` validates before comparison. The gap is that the
  validation is call-order convention, not a type obligation.

## Test classification

| Test file                                                  | Tests touching a mapping fixture                      | Classification                                                                                                                                                      |
| ---------------------------------------------------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `scripts/tests/test_release_declaration.py`                | 10                                                    | Mixed: 6 policy behaviour over a shared `TARGET_CONFIG`, 4 payload-shape cases (non-mapping config, missing `targets`, target without `platform`, unknown platform) |
| `scripts/tests/test_release_declaration_invalid_config.py` | 1 parametrized test, 2 payloads                       | Payload shape: raw TOML strings through the CLI                                                                                                                     |
| `scripts/tests/test_release_declaration_cli.py`            | Behavioural fixtures via `release_staging_support`    | Adapter, writes real TOML                                                                                                                                           |
| `scripts/tests/test_release_identity_record.py`            | 4 Hypothesis properties                               | Payload shape and policy behaviour: round trip, missing field, non-boolean flag, contradictory flag                                                                 |
| `scripts/tests/test_release_candidate.py`                  | 1 round-trip assertion                                | Policy identity: proves `as_record` and `identity_from_mapping` agree                                                                                               |
| `scripts/tests/test_release_digest_record.py`              | 14                                                    | Mixed: 5 adapter tests (round trip, sorted, lowercased, malformed payloads, unreadable path) and 9 policy comparison tests                                          |
| `scripts/tests/test_release_digest_record_files.py`        | 3                                                     | Adapter: round trip, unreadable path, JSON shape error                                                                                                              |
| `scripts/tests/test_verify_release_inventory_command.py`   | 2 fixtures plus one malformed trusted record          | Adapter end-to-end; the malformed record is payload shape                                                                                                           |
| `scripts/tests/test_verify_release_inventory_cli.py`       | Trusted-record fixtures                               | Adapter                                                                                                                                                             |
| `scripts/tests/test_finalize_github_release_cli.py`        | 6 fixture sites, 1 parametrized malformed-record test | Adapter; the malformed cases are payload shape                                                                                                                      |
| `scripts/tests/test_release_reconcile_github.py`           | 9                                                     | Policy behaviour over domain-shaped name-to-digest mappings. Retain as mappings.                                                                                    |

## Migration cost for adopted option

Scope is the two external-shape seams only: staging targets and the trusted
digest record. Identity reclassification moves a module between the contract
lists without moving code, because `ReleaseIdentity` already exists.

| Item                                          | Count    | Detail                                                                                                                                                                                                                                                                    |
| --------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Policy functions whose signature changes      | 4        | `platform_target_counts`, `declared_asset_names`, `require_record_matches_declaration`, `require_downloads_match_record`                                                                                                                                                  |
| Decoders moved to adapters                    | 2        | `platform_target_counts`'s mapping walk and `validate_digest_record`'s `assets` walk                                                                                                                                                                                      |
| Production adapters or CLIs edited            | 4        | `release_declaration_cli`, `release_digest_record_files`, `verify_release_inventory_cli`, `finalize_github_release_cli`                                                                                                                                                   |
| Contract list edits                           | 1        | Reclassify `release_identity_record` from `POLICY_MODULES` to `ADAPTER_MODULES`                                                                                                                                                                                           |
| Test files edited                             | 7        | `test_release_declaration.py`, `test_release_declaration_invalid_config.py`, `test_release_identity_record.py`, `test_release_digest_record.py`, `test_release_digest_record_files.py`, `test_verify_release_inventory_command.py`, `test_finalize_github_release_cli.py` |
| Test cases whose fixture construction changes | about 24 | 10 declaration, 4 identity, 14 digest record (overlapping adapter and policy), plus the CLI payload-shape cases                                                                                                                                                           |
| New behaviour to implement                    | None     | The migration is structural; every rejection category already exists                                                                                                                                                                                                      |

## Field-loss and normalization risks

- `ReleaseIdentity.as_record()` sorts keys and emits exactly the six fields
  the decoder requires. A record type must keep all six; dropping
  `is_prerelease` would lose the only check that a recorded flag agrees with
  the version.
- `validate_digest_record` lowercases digests. A record type that stores the
  original case would change comparison behaviour for records written in
  uppercase, so normalization stays at construction.
- `platform_target_counts` collapses target names into a platform histogram.
  The target name itself is not used downstream, so a `StagingTarget` record
  needs only the platform, not the table key. Recording the key as well would
  add an unused field and invite comparisons that the policy does not make.
- The staging decoder currently accepts any `dict` with string keys, including
  a mapping that has extra members beside `targets`. The decoder may keep
  ignoring unknown members; rejecting them would be a behaviour change and
  needs its own justification.
- `write_digest_record` currently accepts a plain mapping and validates it
  internally. If it takes a `DigestRecord`, the producer in
  `release_staged_digests` must build one, which would push record knowledge
  into another adapter. Keeping the writer's public parameter a mapping and
  constructing the record inside the adapter avoids that.

## Decision criteria

Adoption is justified only when all four hold:

1. **One decoding location.** Each external record shape is decoded once, at a
   named adapter or CLI, and policy receives a domain value.
2. **Preserved rejection categories.** Every existing refusal keeps its type
   and its message semantics, and the tests that prove each refusal still pass.
3. **Fewer duplicated checks.** The staging-target double decode disappears,
   and the digest comparison can no longer be called with an unvalidated
   mapping through the normal path.
4. **Bounded fixture migration.** No more than roughly two dozen test cases
   change construction, and no test loses a rejection it currently proves.

## Options considered

- **Option A: retain the mapping boundary.** Cost is zero now. The cost is
  permanent: `assets`, `targets`, and `platform` stay policy vocabulary, the
  staging config is decoded twice, and "validated mapping" stays a convention
  rather than a type. This is the status quo recorded in the trusted-packaging
  decision log.
- **Option B: decode at the edge into frozen slotted records.** Adapters and
  CLIs own field names and shape checks; policy functions take `StagingTargets`
  and `DigestRecord`. Cost is the table above. Benefit is criteria 1 through 3.
  This matches `CandidateFacts`, `ReleaseIdentity`, `PublishedAsset`,
  `RegistryVersion`, and `_release_admission/records.py`.
- **Option C: `TypedDict` only.** Rejected. A `TypedDict` is a static
  annotation with no runtime validation, no single decoding location, and no
  way to prevent an unvalidated mapping from reaching a comparison. It would
  satisfy the type checker while leaving every runtime gap in place.

The evidence supports **Option B, narrowed to the external-shape seams**.
Domain-shaped mappings are retained where the policy legitimately decides over
a name-to-value relation that no external format defines: `plan_asset_actions`
and the computed-downloads side of `require_downloads_match_record`.
`PublishedAsset`, `ReleaseIdentity`, and the registry observations are already
records and are unchanged.

## Delivery plan

1. Record the decision as ADR-0068, indexed from `docs/contents.md`, linked
   from the release automation design section and roadmap task 9.1.1, and
   appended to the trusted-packaging decision log. Add the boundary convention
   to the developers' guide.
2. Add characterization tests for the staging and digest decoders at their
   current seams, so the migration has a before-and-after oracle.
3. Introduce `StagingTarget` and `StagingTargets` beside
   `release_package_formats`, and `DigestRecord` beside
   `release_digest_record`, as frozen slotted dataclasses with validating
   constructors.
4. Move the staging TOML walk and the digest `assets` walk into
   `release_declaration_cli` and `release_digest_record_files`. Preserve
   `MissingTargetsError`, `MalformedTargetError`, `MalformedTargetKind`,
   `DigestRecordError`, and `RecordedDigestMismatchError` unchanged.
5. Change the four policy signatures, keep every identity and inventory check,
   and remove the staging double decode.
6. Reclassify `release_identity_record` as an adapter in the contract lists and
   update the boundary documentation.
7. Update the ExecPlan Progress, Outcomes, Status, and the roadmap checkbox.

## Progress

- [x] Inventory policy seams and production callers on `main` at `0a3104d44`.
- [x] Classify mapping fixtures as policy behaviour or payload shape.
- [x] Estimate Option B call-site, fixture, and adapter cost.
- [x] Allocate the next unreserved ADR number across remote branches and open
      pull requests. ADR-0068 is free across all 77 remote heads and every
      open pull request.
- [x] Write ADR-0068 and update the documentation index, design section,
      roadmap item, developers' guide, and trusted-packaging decision log.
- [x] Add characterization tests. Commit `032c503ec` pins eight malformed
      staging configurations and nine decoded digest-record shapes at the
      decoders' current seams, all passing against the pre-migration code.
- [x] Migrate the staging target and digest record seams. Commit `716f9b914`.
- [x] Reclassify the identity decoder and update the boundary contracts.
- [x] Run the release-publication, workflow-contract, typecheck, lint, format,
      Markdown, and spelling gates. All seven pass on `716f9b914`.
- [x] Update this plan's Outcomes and Status, and the roadmap 9.1.1 checkbox.

## Outcomes and retrospective

The evaluation adopted Option B, narrowed to the external-shape seams, and
recorded it as
[ADR-0068](../adrs/adr-0068-typed-records-for-release-policy-boundaries.md).
Two records now cross the boundary: `StagingTargets` (with `StagingTarget`
entries) in `release_declaration`, and `DigestRecord` in
`release_digest_record`. `release_declaration_cli` owns the `targets` and
`platform` member names and decodes the staging configuration once, before
discovery runs; `release_digest_record_files` owns `assets` and decodes the
trusted record into a value. `release_identity_record` moved from
`POLICY_MODULES` to `ADAPTER_MODULES` without a code change, because it already
decoded the record's field names into a validated identity.

Nothing was weakened. Every rejection category survives with its original type
and diagnosis: `MissingTargetsError`, `MalformedTargetError` with
`MalformedTargetKind`, `DigestRecordError`, and `RecordedDigestMismatchError`.
The empty-record, exact-coverage, missing-download, and multi-asset mismatch
checks are unchanged, and commit identity is still an exact string comparison.
The staging decode still runs before discovery, so a malformed table fails on
the configuration rather than on a later adapter.

Three points were less mechanical than the estimate suggested:

- `Mapping` is invariant in its key type, so `Mapping[str, str]` cannot be
  passed where `Mapping[str, object]` is expected. `DigestRecord.from_mapping`
  therefore accepts `Mapping[str, object]`, and the adapter narrows the decoded
  `assets` mapping with a string-key check before the cast.
- Moving the decode moved a check with it. The adapter rejects a payload whose
  `assets` keys are not strings, because JSON object keys are always strings
  and a non-string key can only arrive as an already decoded value. The record
  keeps the same rule in `__post_init__` as well, so `from_mapping` fails with
  `DigestRecordError` rather than `TypeError` when a mixed-type key set would
  break `sorted`.
- The fixture module that both verifier suites share now produces a
  `DigestRecord` rather than a dict, so `write_trusted_record` serializes the
  record instead of revalidating a mapping.

Review of the migration tightened where those invariants live. Both records now
validate in `__post_init__` as well as in their factories, so a directly
constructed record cannot reach the comparison functions unvalidated; the
staging suite exercises both construction paths, and the digest suite rebuilds
each refusal without the factory. One `_validated_entry` helper holds the
digest record's type and hexadecimal checks, so the factory's case
normalization and the record's canonical-form rule cannot drift apart.

The regression evidence is pinned rather than incidental. The nine
previously-rejected digest shapes and eight staging shapes were committed
before the migration and still pass after it, now asserted through the new
decoders; the policy suites construct records directly, so they test the
decision rather than the decoding.

Gate evidence, bound to the migration commit `716f9b914`: `make check-fmt` (366
Python files, 209 Markdown files unchanged), `make typecheck`, `make lint`
(Clippy, Whitaker, Ruff, Pylint, df12 lints, ambrleaks, Interrogate at 100%),
`make doc-coverage` (98.21% against the 80% threshold), `make markdownlint`
(209 files, no issues), `make test` (4804 tests passed, 7 skipped, doctests
clean), and `make test-workflow-contracts` (1665 passed, 3 skipped). The
documentation commit that carries this update is re-gated on the branch head.
Roadmap item 9.1.1 stays open, because its success criterion covers thresholds,
freshness rules, waivers, and retention as well; its policy-boundary sub-item
is checked against ADR-0068.

## References

- [Issue #964](https://github.com/leynos/netsuke/issues/964)
- [Pull request #903](https://github.com/leynos/netsuke/pull/903)
- [CodeRabbit review comment 6448190491](https://github.com/leynos/netsuke/pull/903#discussion_r6448190491)
- [Trusted packaging ExecPlan](use-trusted-packaging.md)
- [RFC 0005: Harden release integrity and admission](../rfcs/rfc-0005-harden-release-integrity-and-admission.md)
- [ADR-0020: Bounded release admission observability](../adrs/adr-0020-bounded-release-admission-observability.md)
