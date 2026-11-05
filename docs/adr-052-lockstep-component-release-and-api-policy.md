# Architectural decision record (ADR) 052: Release components as one product

## Status

Proposed.

## Date

2026-10-03.

## Context and problem statement

Crate decomposition creates new registry packages and externally reachable Rust
interfaces even when Netsuke remains one application. Independent versioning and
release schedules would multiply compatibility combinations before independent
products or consumers justify them. Conversely, labelling a published library
internal does not make its public items technically inaccessible.

[ADR-007][package-name] already distinguishes the `netsuke-build` package from
the `netsuke` library and executable targets. [RFC 0035][release] extends the
existing Lading configuration to publish the necessary component closure while
preserving that distribution contract.

## Decision drivers

- Keep one product release train and a tractable compatibility matrix.
- Preserve existing application imports and installation paths where practical.
- Make public API and registry dependency obligations explicit.
- Keep test-only fixtures out of production publication.
- Separate reusable release mechanics from repository-owned authority.

## Proposed direction

Version Netsuke-owned publishable components in lockstep initially. Use one
repository, coordinated changelog/release preparation, and one admitted candidate.
An unchanged component may participate in the same version bump to keep the
train coherent. Upstream products such as OrthoConfig and `ortho_l10n` retain
their own release cadence; this decision does not synchronize their versions
with Netsuke.

Publish every new package required by the application's normal/build dependency
closure before its dependants. Use version-plus-path declarations locally and
verify the registry requirements in staged packages. Keep private fixture
packages `publish = false` and outside that closure. Confirm package-name
availability and ownership before an operator-authorized first publication.

Retain the existing `netsuke-build` package and `netsuke` target names. Prefer
compatibility re-exports through the application library when they preserve
actual type identity. Expose the smallest useful component API, document its
intended consumers and stability contract, and review semantic-versioning
consequences of public signature, trait, feature, or serialization changes.
Lockstep versioning reduces combinations; it does not waive API obligations.

Internal components need not promise independently stable APIs merely because
they are separate crates, but any limitation must be explicit in their public
documentation. Do not hide breaking changes under an assertion that published
public items are private. Preserve facade compatibility or publish a deliberate,
documented migration under the product's version policy.

Optional manifest capabilities do not automatically become optional Cargo
features. Introduce a feature switch only for a concrete consumer/build need,
with default behaviour preserved and a bounded supported matrix. Test default
and relevant enabled/disabled combinations independently, including package
consumers; workspace-wide feature unification is not sufficient evidence.

Lading owns workspace release mechanics. Shared actions own reusable CI
adapters. Netsuke owns the admitted candidate, required checks, publishing
workflow identity, protected environment, and credential authority. Proposed
workflow integration does not authorize live publication or registry changes.

## Alternatives considered

Independent component release trains create avoidable compatibility and operator
burden. A path-only production workspace cannot preserve registry installation.
Publishing every fixture to simplify tests exposes inappropriate packages.
A public facade containing duplicate nominal types can break consumers despite
similar names and fields.

## Consequences

More package versions may publish per product release, requiring ordered and
resumable tooling. The release remains non-transactional, so receipts and
candidate-verified recovery matter. The public API surface needs active curation
rather than assuming that package count alone delivers modularity.

## Verification and revisit criteria

Validate the full normal/build package closure, synchronized versions, private
exclusions, package resource ownership, facade/direct mixed consumers, and
existing installation/archive contracts. Check target-specific dependencies and
relevant features, not only the default host graph.

Revisit independent versioning only when a component has a real independently
supported consumer population and a reviewed compatibility/release owner. Record
the additional testing and operator cost before changing the policy.

## Outstanding decisions

Implementation determines exact registry names, component public-surface scope,
and required migration notices. No package, tag, secret, or publisher setting
changes merely because this ADR enters review.

[package-name]: adr-007-publish-as-netsuke-build.md
[release]: rfcs/0035-lading-backed-workspace-publication.md
