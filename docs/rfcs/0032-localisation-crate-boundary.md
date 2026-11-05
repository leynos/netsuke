# RFC 0032: Separate localisation from configuration machinery

## Preamble

- **RFC number:** 0032
- **Status:** Proposed
- **Created:** 2026-10-03
- **Scope:** Upstream runtime extraction and Netsuke integration.
- **Upstream dependency:** [OrthoConfig issue #566][upstream-issue].
- **Delivery:** [Crate decomposition roadmap][roadmap], phase 31.

## Summary

Create `netsuke-l10n` for Netsuke's catalogues and rendering, backed directly by
a reusable upstream `ortho_l10n` crate. Keep configuration discovery and CLI
adapters above that runtime. Preserve public type identity and existing lookup,
fallback, and builder behaviour during migration.

The complete Netsuke executable still needs configuration. The benefit is that
localisation-only consumers and tests no longer need to compile it.

## Current state

The [OrthoConfig manifest][upstream-manifest] unconditionally depends on its
derive crate, Clap, Clap dispatch, Figment, and configuration-discovery
facilities alongside Fluent. Disabling default features does not produce a
localisation-only dependency closure.

Its [localizer module][upstream-localizer] already defines `Localizer`,
`NoOpLocalizer`, `LocalizationArgs`, Fluent construction, and formatting errors,
but also exports Clap adapters. `LocalizationArgs` contains Fluent values, and
the default Fluent builder supplies embedded resources. Both facts constrain a
compatible extraction.

Netsuke exposes localisation, catalogue inventory, locale resolution, and
`cli::localization` through its [root library][library]. Its build script also
consumes catalogue inventory and the localisation key registry. The extraction
must follow those actual consumers rather than move one directory by name.

## Goals and non-goals

Provide a genuinely smaller runtime dependency closure, one set of nominal
public types, one identifier convention, and complete catalogue/package tests.

Do not introduce a new internationalization engine, backend-neutral argument
redesign, configuration-precedence framework, generic diagnostics system, or an
immediate separate Clap adapter crate. Do not install a process-wide locale or
perform ambient configuration discovery in the new runtime.

## Proposed design

### Ownership and direction

| Owner | Contract |
| --- | --- |
| `ortho_l10n` | Lookup interfaces, existing Fluent arguments, Fluent implementation/builder, formatting reports/errors, reusable identifier rules, and catalogue composition mechanics |
| `ortho_config` | Configuration discovery/merging, derives, configuration-specific policy, and Clap command/error adapters; compatibility re-exports of upstream runtime types |
| `netsuke-l10n` | Netsuke catalogues and key inventory, product-specific fallback/rendering, and explicit localizer construction |
| Netsuke application | Locale selection from CLI/configuration/environment and conversion of semantic diagnostics to presentation |
| Netsuke semantic model | Typed diagnostic facts, without owning translated strings |

*Table 1: Localisation ownership. Dependencies point from configuration and
product adapters towards the runtime, never back from `ortho_l10n`.*

Retain catalogue selection policies that are product semantics in Netsuke, but
pass their inputs explicitly. Separate pure locale resolution from reading the
process locale. The existing application may still expose compatibility helper
paths while the new component owns the implementation.

### Compatibility-sensitive upstream extraction

Re-export the same `Localizer`, builder, argument, and error types through
`ortho_config`. A second equivalent trait plus conversions is not a compatible
substitute. Exercise old-path, new-path, and mixed-path consumers, including the
existing thread-safety contract and Fluent numeric arguments.

Inventory the default builder's embedded resources before moving it. Preserve
its behaviour or stage an independently reviewed API migration. Retaining small
embedded resource data in the lightweight runtime can preserve compatibility
without importing the configuration implementation. Do not make the runtime
call back into `ortho_config` for defaults, and do not promise different default
behaviour for two re-exports of the same inherent builder method.

Keep Clap adapters in `ortho_config` initially. Keep configuration-specific
metadata generation there or in its existing macro components. Shared
identifier normalization must have one implementation/contract and agreement
tests; moving the whole documentation IR into `ortho_l10n` is not justified by
shared message identifiers.

### Relationship to documentation IR 2.0

[Netsuke issue #779][ir-issue] tracks adoption of OrthoConfig **documentation**
IR 2.0. It is distinct from Netsuke's build-graph IR. [OrthoConfig PR #536][ids-pr]
supersedes #420 and proposes derive/runtime/documentation identifier agreement.
At this proposal's review date, #536 is open; no compatible release version is
assumed here.

Coordinate the ownership of its localization trait and identifier records with
#566. Preserve generated-code paths and the agreement between derive constants,
`message_id_for`, and metadata. Do not fork the convention in Netsuke.

The downstream IR adoption updates both normal and build dependency declarations,
regenerates release-help metadata from the released upstream contract, and
checks affected catalogue identifiers. Read `ORTHO_DOCS_IR_VERSION` instead of
hard-coding a duplicate version. Runtime extraction and IR adoption are related
review streams, not an excuse to block every benchmark or fixture improvement.

### Netsuke migration stages

First characterize lookup, fallback order, regional/script locale distinctions,
formatting failures, diagnostic identifiers, and packaged resources. Move
catalogues, key inventory, and explicit construction together with their tests.
Preserve build-time auditing and generated help/completions.

An intermediate `netsuke-l10n` may still use `ortho_config` while the upstream
release is unavailable. Mark that dependency as temporary in the architecture
inventory, assign its removal to #566 adoption, and report the actual compiled
closure. It does not satisfy this RFC's lightweight acceptance gate.

After the compatible release, depend directly on `ortho_l10n` and remove the
configuration edge from focused runtime tests and their support. The application
retains `ortho_config` only where needed. Remove semantic rendering dependencies
in coordination with [RFC 0026][semantics], rather than mixing an unbounded
error rewrite into the first mechanical move.

## Verification and acceptance

A standalone fixture depending only on `ortho_l10n` must exercise resource
loading, lookup, interpolation, fallback, and formatting-error reporting without
compiling `ortho_config`, its macros, Figment, or Clap. Cover normal/build edges
and focused test builds; do not rely only on a workspace-wide invocation whose
feature unification may hide the result.

For Netsuke, independently build and test `netsuke-l10n` without the application
or configuration loader. Validate every supported catalogue, key-audit failures,
regional/script distinctions, unsupported locales, missing messages, numeric
plural arguments, malformed resources, and compatibility imports. Record which
Fluent version supplies the public argument types to avoid nominal mismatches.

Run release-help snapshot and identifier-agreement tests for #779. Compare
human/structured diagnostics and exit behaviour at the application boundary.
Package a clean consumer so catalogue paths cannot accidentally resolve through
the parent workspace. Use [RFC 0030][benchmark] for measured dependency/build
cost changes, including host build-script contexts.

## Alternatives considered

A facade that re-exports `ortho_config` leaves the dependency closure unchanged.
Default-feature disabling does not remove unconditional dependencies. Copying
the runtime into Netsuke duplicates upstream bug fixes and type identities.
A simultaneous backend-neutral redesign would enlarge the compatibility surface
without being necessary for the intended build boundary.

## Outstanding decisions

Upstream review determines exact resource ownership and identifier-type placement.
The consuming PR selects an actually released version and records any deliberate
API transition. The terminal application remains the owner of user locale
selection; no hidden global discovery may fill an unresolved design gap.

## Recommendation

Pursue #566 upstream, prepare the Netsuke component independently, and consider
the extraction complete only when isolated consumers prove the reduced closure.

[roadmap]: ../roadmap-crate-decomposition.md
[benchmark]: 0030-build-performance-benchmark-extension.md
[semantics]: 0026-hexagonal-domain-hardening.md
[library]: ../../src/lib.rs
[upstream-issue]: https://github.com/leynos/ortho-config/issues/566
[upstream-manifest]: https://github.com/leynos/ortho-config/blob/main/ortho_config/Cargo.toml
[upstream-localizer]: https://github.com/leynos/ortho-config/blob/main/ortho_config/src/localizer/mod.rs
[ir-issue]: https://github.com/leynos/netsuke/issues/779
[ids-pr]: https://github.com/leynos/ortho-config/pull/536
