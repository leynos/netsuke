# RFC 0035: Extend Lading-backed publication to the component workspace

## Preamble

- **RFC number:** 0035
- **Status:** Proposed
- **Created:** 2026-10-03
- **Scope:** Multi-crate release mechanics, admission, and reusable CI adapters.
- **Prior art:** [rstest-bdd release configuration][prior-config] and
  [release procedure][prior-release].
- **Delivery:** [Crate decomposition roadmap][roadmap], phase 33.

## Summary

Extend Netsuke's existing Lading configuration into an explicitly ordered
component release train. Reuse rstest-bdd's staging, package ordering, and
conditional preflight pattern; integrate crates.io trusted publishing; and
extract reusable CI adapters to `leynos/shared-actions`.

Lading owns release mechanics. Shared actions own reusable CI integration. The
Netsuke repository owns release authority, admission policy, workflow identity,
and distribution contracts. This documentation PR publishes no packages and
changes no credentials, tags, or registry settings.

## Current state and prior art

Netsuke already has [lading.toml][current-config], configuring unit-test-only
preflight and documentation bump globs. This is an extension, not a new tool
adoption. Its [package manifest][manifest] preserves the `netsuke-build` package,
`netsuke` executable, packaged localisation/build-audit resources, and existing
binstall archive naming.

The inspected [rstest-bdd configuration][prior-config] declares an explicit
publish order, excludes a private server component, and strips patches
per crate. Its local preflight remains enabled; CI skips duplicate preflight
only after relevant tests have already run. Its [Lading guide][lading-guide]
describes sequential live package/publication and resumable handling of already
published versions, not a transactional all-or-nothing release.

Those are prior-art behaviours. Strong candidate receipts, content-verified
resumption, bounded authenticated batches, and generic admission adapters below
are requirements to validate or add, not capabilities assumed to exist today.

## Goals and non-goals

Release the complete production/build dependency closure in order, preserve
existing binary/archive/help contracts, avoid duplicate expensive checks, and
keep authorization tied to an admitted source candidate.

Do not create an independent release cadence for each component, duplicate
Lading's dependency logic in shell, change registry ownership automatically, or
turn this whole programme into another v0.1.0 stabilization gate.

## Proposed design

### Release plan and package closure

Keep Netsuke-owned publishable components versioned in lockstep under
[ADR-052][release-adr]. OrthoConfig and `ortho_l10n` remain upstream products with
their own release cadence. Verify each proposed package name before bootstrap.
Use local path plus registry-version declarations for publishable dependencies.

Extend the existing configuration with the actual publishable package set,
explicit exclusions for fixtures, and a dependency-respecting order. Validate
normal and build dependency closure, including target-specific edges and
release features. Do not copy rstest-bdd's package list literally or assume that
Cargo workspace membership equals publication membership.

Lading should validate the configured order against the derived graph and fail
on missing packages, cycles, unavailable required versions, or contradictory
exclusions. Keep bumping, manifest synchronization, staging, patch policy, and
publication ordering in Lading. A staging policy must distinguish legitimate
workspace paths from development-only patches and prove that the packaged
manifests retain the intended registry requirements.

Update include lists and package resource ownership with each extraction.
Validate packages from a clean staged source rather than from files outside a
member's package. Keep `cargo install` and binstall consumer checks distinct
from workspace compilation.

### Candidate identity and admission

Extend [RFC 0005][admission], not a parallel weaker gate. A candidate manifest
binds the exact source commit, lockfile and package-manifest digests, package
names/versions, publication order, relevant tool/action revisions, release
feature/target configuration, and required evidence. Record archive checksums
as packages become available through the ordered release.

Admission verifies evidence for that exact candidate and the repository-owned
required checks. A successful run on a different commit, a mutable branch name,
or an unrelated artefact is insufficient. Preserve the existing bounded
[release-admission observability][admission-observability].

Do not confuse authentication with admission. Possession of an upload token
does not establish that a candidate passed review and tests. The authenticated
job executes only admitted reviewed source and pinned tooling, never arbitrary
pull-request code or executable content taken from an untrusted evidence bundle.

### Dry runs and live sequencing

Perform ordinary quality gates and package-plan validation before authentication.
Avoid duplicate full tests only when recorded evidence covers the same source,
features, targets, and inputs. A configuration flag alone cannot waive preflight
for an unverified local invocation.

The rstest-bdd dry-run override `--allow-unpublished-workspace-deps` supports a
new release train whose sibling versions are not yet in the registry. Restrict
it to dry runs, retain its warnings in evidence, and do not label such a run
proof that the final registry graph compiled successfully. Required later
package verification and registry-consumer checks close that evidence gap.

Live publication is ordered and non-transactional: package/verify and publish a
prerequisite before its dependants, with bounded waiting for registry visibility.
Record each completed package and content checksum. A failure leaves a visible
partial release; do not pretend that a rollback removed uploaded versions.

Resumption must compare existing registry package contents with the admitted
candidate, using a documented checksum/canonical-content comparison when archive
metadata prevents byte-for-byte reproducibility. Do not skip a version solely
because its name and version exist. Reject mismatches and require an explicit
recovery decision. Automatic yanking is not a rollback protocol.

If current Lading cannot expose the plan, verified receipts, or bounded resume
units needed here, add and test those capabilities upstream before wiring the
CI adapter. Do not invent command-line flags in repository workflows.

### Trusted publishing and bootstrap

Use the official [crates.io authentication action][auth-action], pinned to a
reviewed immutable revision. It exchanges OpenID Connect (OIDC) identity for a
temporary token and revokes that token in its post-job step. Supply the token
only to publication; never write it into staged sources, logs, caches, or
uploaded evidence. Mask derived sensitive values as well.

Configure trusted-publisher identity per crate, bound to the intended repository,
workflow filename, and protected environment. Keep authentication and admission
in Netsuke's own workflow initially, using shared composite adapters where
appropriate. Do not assume a cross-repository reusable workflow has identical
OIDC claim semantics.

Plan an explicit first-publication bootstrap using the registry's supported
operator procedure. The [initial trusted-publishing rollout][bootstrap-doc]
requires a first manual publication before per-crate configuration. Recheck the
current [registry documentation][trusted-doc] during implementation rather than
assuming that this limitation or any token lifetime is permanent. Record each
crate's ownership, initial publication, and publisher configuration separately.
Bootstrap credentials must not become a silent fallback for ordinary releases.

Authenticate after expensive preparation, account for the actual token expiry,
and use bounded publication batches when necessary. Verify that Lading supports
the required batch/resume contract before relying on reauthentication; a single
early token does not authorize an arbitrarily long train.

Use an explicitly admitted tag/dispatch release path. The current registry
[token-exchange implementation][exchange] rejects `workflow_run` and
`pull_request_target`; do not wire publishing to those events. A permitted
workflow may retrieve earlier evidence after checking its exact candidate
binding. Revalidate registry restrictions and workflow identity at rollout.

### Reusable shared-actions components

Reuse existing [shared-actions][shared] Rust setup, metadata/release-mode,
binary-build, staging, and asset-upload facilities rather than duplicate them.
Proposed additional adapters are:

| Adapter responsibility | Reusable contract | Repository-owned input |
| --- | --- | --- |
| Lading setup | Install a pinned, verified tool; expose its identity | Approved tool revision and platform |
| Release-plan validation | Invoke Lading checks and retain structured results | Workspace configuration and exclusions |
| Evidence verification | Validate candidate/evidence binding with bounded parsing | Required check set and admission policy |
| Publication integration | Execute an approved bounded plan and emit receipts | Candidate identity and caller-supplied credential |
| Statistics reporting | Summarize bounded timing/cache/publication evidence | Retention and presentation preferences |

*Table 1: Proposed shared adapters, not a claim that these action names or
contracts already exist.*

Keep substantive logic in tested tools/modules, not large embedded shell blocks.
Pass workflow values as data through explicit inputs/environment; never interpolate
untrusted expressions into executable shell. Inherit the existing repository
workflow-contract requirements rather than weaken them during extraction.

Test these adapters in `shared-actions`, then pin and consume them in rstest-bdd
and Netsuke where their contracts genuinely match. Repository-specific policy
remains declarative configuration, not a hard-coded Netsuke rule in a supposedly
generic action. No shared component receives broader write authority merely
because it is reused.

## Verification and acceptance

Require plan validation against a multi-crate fixture, isolated package consumers,
private-member exclusion, version/path synchronization, archive/resource checks,
and preservation of binary/help/checksum contracts. Cover missing prerequisites,
index delay, partial publication, existing-version mismatch, token expiry,
authentication failure, and safe resumption with mocks or a controlled registry.
Do not test failure recovery by publishing disposable production versions.

Workflow contracts must reject wrong-commit evidence, missing admission,
unsupported triggers, broad permissions, mutable unreviewed action references,
secret-bearing reports, and shell-expression injection. Positive controls must
exercise the legitimate release path. Perform a documented operator-reviewed
bootstrap and trusted-publishing smoke test before retiring the old credential
path; actual publication requires separate release authorization.

Measure the publication path and retain stage timings. Skipping preflight is
acceptable only with equivalent evidence; a shorter job that omits verification
is not a performance improvement.

## Alternatives considered

Handwritten `cargo publish` loops duplicate graph/recovery logic. Blindly copying
the prior-art workflow misses Netsuke's admission and binary-distribution rules.
A privileged shared workflow can complicate identity and authorization. One
long-lived registry secret bypasses the intended temporary-credential boundary.

## Outstanding decisions

Inventory the selected Lading revision's actual capabilities, choose the receipt
comparison rule, and define authenticated batch boundaries. Recheck registry
bootstrap/expiry/event restrictions and current package availability before
implementation. These are explicit implementation gates, not hidden assumptions.

## Recommendation

Extend the existing Lading foundation, extract reusable adapters with tests, and
retain repository-owned admission and publisher identity for one component train.

[roadmap]: ../roadmap-crate-decomposition.md
[current-config]: ../../lading.toml
[manifest]: ../../Cargo.toml
[release-adr]: ../adr-052-lockstep-component-release-and-api-policy.md
[admission]: 0005-release-hardening.md
[admission-observability]: ../adr-020-release-admission-observability.md
[prior-config]: https://github.com/leynos/rstest-bdd/blob/main/lading.toml
[prior-release]: https://github.com/leynos/rstest-bdd/blob/main/docs/releasing-crates.md
[lading-guide]: https://github.com/leynos/rstest-bdd/blob/main/docs/lading-users-guide.md
[shared]: https://github.com/leynos/shared-actions
[auth-action]: https://github.com/rust-lang/crates-io-auth-action
[trusted-doc]: https://crates.io/docs/trusted-publishing
[bootstrap-doc]: https://blog.rust-lang.org/2025/07/11/crates-io-development-update-2025-07/
[exchange]: https://github.com/rust-lang/crates.io/blob/adbbd7a8c6049c98905ab3734f4c9cdf4762cc85/src/controllers/trustpub/tokens/exchange/mod.rs
