# Architectural decision record (ADR) 0061: Manifest schema versions and admission

## Status

Proposed. This decision remains pending review and explicit acceptance.

## Date

2026-10-24

## Context and problem statement

`netsuke_version` identifies a manifest's schema, but Netsuke currently only
deserializes it as a SemVer value. It does not compare that value with a
supported range, and it reads the version after Jinja evaluation. A manifest
declaring a schema the binary cannot understand can therefore reach template
helpers that read the environment, inspect the filesystem, run commands, or
fetch URLs before deserialization rejects anything.

The progressive-enhancement RFCs introduce optional keys, fields, command
operations, and closed vocabularies. Several roadmaps may deliver them in any
order. Without one allocation rule, parallel work can claim the same minor,
skip a reserved minor, or publish syntax before older readers can reject it.
The accepted
[ADR-0043](adr-0043-consolidated-structured-command-execution-contract.md)
already reserves `1.1.0` for the consolidated structured-command surface and
identifies allocation coordination as an open limitation.

The decision must define what the version means, when a reader checks it, how
features reserve and acquire a minor, and how an older reader reports syntax it
cannot accept. It must preserve existing `1.0.0` manifests and avoid treating
the crate's release version as a schema version.

## Decision Drivers

- Reject unsupported manifests before any template evaluation.
- Give every additive syntax change one predictable allocation point.
- Honour ADR-0043's `1.1.0` reservation and keep supported schema minors
  contiguous.
- Preserve one shallow schema for existing manifests and optional features.
- Give authors a useful upgrade remedy without silently ignoring unknown
  syntax.
- Keep the admission policy independent of filesystem, environment, and
  configuration access.

## Requirements

### Functional requirements

- Treat `netsuke_version` as the manifest schema version, not the Netsuke
  release version.
- Admit a document only when its declared schema version is supported and its
  syntax is admitted by that reader.
- Reserve syntax names before implementation and identify their owning feature
  and roadmap task.
- Keep each included document and bundle entry subject to its own version
  check; defer their detailed composition rules to their owners.

### Technical requirements

- Use canonical SemVer spelling `MAJOR.MINOR.0`, without prerelease or build
  metadata. The reader's supported major and maximum minor are explicit data.
- Check the literal YAML value immediately after parsing a mapping root and
  before variable or macro registration, manifest expansion, or template
  evaluation.
- Allocate new minors only on `main`, at the change that admits the feature.
  Freeze the feature set when a tag publishes that minor.
- Do not add a manifest opt-in list, configuration override, or feature-specific
  switch alongside `netsuke_version`.

## Options considered

### Option A: Use an additive schema version as the admission contract

Keep the SemVer-shaped field, define its canonical form, and compare it with a
reader-owned supported range before evaluation. Reserve syntax in a design
registry, then allocate a minor when the feature becomes admissible. This keeps
the version as the single author-facing opt-in and makes the reader guarantee
testable.

### Option B: Add feature flags beside the schema version

An explicit `cargo-features`-style list could name individual syntax, but it
would duplicate the version signal and require every feature to maintain a
second compatibility mechanism.

### Option C: Treat the version as advisory or tie it to the Netsuke release

An informative-only value would let an older reader continue into syntax it
cannot understand. A release-version field would couple manifests to the
crate's release cadence and make existing `1.0.0` schema declarations
incompatible with the current `0.1.0-beta4` release line.

| Topic                     | Option A                   | Option B                            | Option C                         |
| ------------------------- | -------------------------- | ----------------------------------- | -------------------------------- |
| Older-reader behaviour    | Reject before evaluation   | Depends on maintaining both signals | Advisory versions may be ignored |
| Feature opt-in            | One schema version         | Version plus feature list           | Release number or warning only   |
| Parallel feature delivery | Shared allocation registry | Separate flags and ownership        | No enforceable allocation        |

_Table 1: Comparison of schema-versioning options._

This follows the useful distinction in Cargo between `rust-version`, which is
an enforced minimum compiler version, and editions, which mark breaking
language changes. It does not copy `cargo-features`, which opts into unstable
syntax. Docker Compose's informative top-level `version` is unsuitable here
because unsupported schema must fail closed. Kubernetes' per-object
`apiVersion` illustrates the value of versioning each independently loaded
document. See the [Cargo `rust-version` reference][cargo],
[Docker Compose version reference][compose], and
[Kubernetes deprecation policy][k8s].

[cargo]: https://doc.rust-lang.org/cargo/reference/rust-version.html
[compose]: https://docs.docker.com/reference/compose-file/version-and-name/
[k8s]: https://kubernetes.io/docs/reference/using-api/deprecation-policy/

## Decision outcome / proposed direction

### Version meaning and canonical form

`netsuke_version` is a manifest schema version. Its canonical spelling is
exactly `MAJOR.MINOR.0`; prerelease and build suffixes are not valid schema
versions. A reader admits a version when its major is supported, the minor is
no greater than that major's configured maximum, the patch is zero, and no
prerelease or build metadata is present. The current reader supports major `1`
through minor `0`. Supported majors and maxima remain data so a future reader
can continue to admit an older major while adding a new one. A major increment
is reserved for a breaking schema change.

### Admission point

When the parsed YAML root is a mapping, inspect its literal `netsuke_version`
value immediately after YAML parsing and before registering variables or
macros, expanding `foreach` or `when`, or evaluating any template. Reject a
missing value, a non-string value, invalid SemVer, or an unsupported version at
this point. Do not template the version. A non-mapping root is structurally not
a manifest and retains the existing structural diagnostic.

The policy is pure: it reads no environment, filesystem, or configuration and
offers no override. The diagnostic reports the schema-version meaning and the
supported range without echoing arbitrary manifest text.

### Reservation and allocation lifecycle

The schema registry in `docs/netsuke-design.md` records feature identifiers,
owning RFCs and roadmap tasks, reserved syntax, status, schema minor, and the
first tag that ships each admitted feature. A reservation blocks reuse of its
names but does not make that syntax admissible.

1. Only `main` allocates a new schema minor. The admission change is the change
   that makes the feature's syntax admissible, and it is the last change in
   that feature's delivery.
2. The first admission change after the latest published tag, including a
   beta, raises the reader's maximum minor by one in the same change. Later
   admissions before the next tag reuse that minor.
3. A tag freezes the feature set of every minor it contains. Changing the
   meaning of syntax in a frozen minor requires a later minor.
4. Release and hotfix branches do not raise the maximum or backport admissions.
5. Reverting the only admission of an unpublished minor also reverts its
   maximum-minor increase.
6. The first schema minor must not be published in the same release that
   introduces the reader gate.
7. Before a tag, capability reporting by feature identifier is authoritative;
   an untagged schema number alone does not promise a feature.
8. The supported range remains contiguous. Do not raise the maximum past a
   reserved minor that has not been admitted.

ADR-0043's accepted `1.1.0` reservation covers the consolidated
`structured-commands` and `named-shells` surface across RFCs 0001, 0009, 0010,
and 0011. While it remains unadmitted, later features wait for roadmap item
12.1.2 to admit `1.1`; the next admission then follows the lifecycle above. RFC
0003's `requires.manifest: ">=1.1.0, <2.0.0"` is an illustrative bundle
descriptor, not a separate allocation. Manifest versions and range comparators
use patch zero.

### Rejecting unsupported syntax

The schema admission vocabulary includes new keys, new command discriminators,
new values in a closed vocabulary, and new template vocabulary introduced by a
registered feature. Reject unsupported syntax in this order; never ignore it:

1. **Unsupported manifest version:** reject before evaluation, lead with an
   upgrade remedy, name the supported range, and explain that the field is the
   schema version.
2. **Syntax admitted in a later minor:** identify its feature and minimum
   schema version as the author remedy.
3. **Reserved but not yet admitted syntax:** identify the reserved feature.
4. **Syntax unknown to every registered feature:** retain the existing
   unknown-field diagnostic.
5. **Known syntax used in an unsupported context:** return the feature-specific
   diagnostic, such as a reserved field on a rule.

Rungs two and three inspect the raw document before evaluation so ordinary
unknown-field deserialization cannot swallow reserved syntax. This ADR does not
make those checks available immediately: roadmap item 12.1.2 or the first
feature admitted by it owns them. This plan's reader-gate implementation
delivers rung one only.

### Nested and composed documents

Each document a reader loads is checked against its own declared version.
Roadmap items 16.1.1 and 17.1.1 own whether included fragments or bundle entry
manifests require a version and how it relates to the importing document. Their
`requires.manifest` comparators must use patch zero. A nested schema such as
RFC 0029's `host_facts.schema` is additional to, and never a substitute for,
the manifest minor; its allocation belongs to RFC 0029's roadmap owner.

## Goals and non-goals

- Goals:
  - Reject an unsupported schema before evaluating manifest templates.
  - Coordinate additive schema changes across independent roadmaps.
  - Preserve `1.0.0` manifests and a single version-based opt-in.
- Non-goals:
  - Implement the syntax in RFCs 0021 to 0025 or other reserved RFCs.
  - Add feature-reporting interfaces, CLI flags, or a configuration override.
  - Define action-plan or state-record formats; those remain independently
    versioned under their owners.
  - Make pre-gate binaries safe readers of future syntax.

## Migration plan

1. Introduce the early reader check for schema `1.0.0` and preserve the current
   structural error for non-mapping roots.
2. Keep the first schema minor out of the same release that introduces that
   gate, allowing users to move to a reader that can reject unsupported
   versions first.
3. Admit later syntax on `main` through the registry and the admission change
   for its owning feature. Freeze each minor at its first tag.
4. Have include, bundle, and host-facts owners settle their nested version
   rules before implementing composition or nested schema versions.

## Known risks and limitations

- Binaries published before the gate continue to accept and evaluate versions
  they do not understand. The reader guarantee starts with the first release
  containing the gate, so the first schema minor must ship later.
- Direct use of the public `Deserialize` implementation on `NetsukeManifest`
  bypasses the loader's admission gate. The loader remains the supported
  manifest-reading boundary.
- New general-purpose Jinja globals and filters are not manifest keys. The
  owner of a registered feature must include its template vocabulary in that
  feature's admission. General standard-library growth remains governed by RFC
  0006.
- Future major versions and their migration policy are not specified beyond
  retaining data for multiple supported major ranges.

## Outstanding decisions

- RFCs 0002 and 0003 must settle the exact version rules for included and
  bundled documents under roadmap items 16.1.1 and 17.1.1.
- RFC 0029's owner must allocate its nested `host_facts.schema` contract
  without replacing the manifest minor.
- Roadmap item 20.2.1 owns the shape of feature capability reporting.
- Roadmap item 12.1.2 or a later admission change must implement raw-document
  checks for feature minima and reserved syntax.
