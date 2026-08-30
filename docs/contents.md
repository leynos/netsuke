# Documentation contents

This index groups the primary Netsuke documentation by purpose so design,
operator, user, and contributor references are easier to find.

## Documentation index

- [contents.md](contents.md): This index for the Netsuke documentation set.
- [repository-layout.md](repository-layout.md): Path ownership and repository
  structure guide for contributors.

## Core design and planning

- [netsuke-design.md](netsuke-design.md): Primary architecture, manifest, and
  execution design document.
- [netsuke-cli-design-document.md](netsuke-cli-design-document.md): Command-line
  interface design and user-experience requirements.
- [git-change-detection-helpers-design.md](git-change-detection-helpers-design.md):
  Git change-detection and glob-matching contracts and verification guidance
  for maintainers, reviewers, and manifest authors.
- [roadmap.md](roadmap.md): Phased implementation plan and tracked delivery
  work.
- [archive/roadmap-completed-foundations.md](archive/roadmap-completed-foundations.md):
  Archived completed roadmap foundations with relevance assessments and
  traceability notes.
- [formal-verification-methods-in-netsuke.md](formal-verification-methods-in-netsuke.md):
  Recommended scope and delivery order for Kani, Proptest, and optional Verus
  checks.

## Requests for comments

- [rfcs/0001-structured-command-blocks.md](rfcs/0001-structured-command-blocks.md):
  Proposed structured command blocks, shell-free argv templates, typed Jinja
  interpolation, stream routing, and pipeline semantics.

## Decision records

- [adr-001-replace-serde-yml-with-serde-saphyr.md](adr-001-replace-serde-yml-with-serde-saphyr.md):
  YAML parser migration decision record.
- [adr-002-replace-cucumber-with-rstest-bdd.md](adr-002-replace-cucumber-with-rstest-bdd.md):
  Behavioural-testing framework migration decision record.
- [adr-003-agent-consistent-human-first-cli.md](adr-003-agent-consistent-human-first-cli.md):
  Human-first, agent-consistent CLI doctrine decision record.
- [adr-003-actions-foreach-when-scope.md](adr-003-actions-foreach-when-scope.md):
  Manifest control-key scoping decision record.
- [ADR-004: Kani IR harnesses](adr-004-bound-kani-ir-harnesses-to-small-n.md):
  Kani IR harness bound and Proptest hand-off decision record.
- [adr-004-graph-subcommand-in-process-rendering.md](adr-004-graph-subcommand-in-process-rendering.md):
  Graph rendering architecture decision record.
- [ADR-004](adr-004-explicit-config-selection-outside-orthoconfig.md):
  Explicit configuration selector ownership decision record.
- [adr-005-typed-which-resolve-error.md](adr-005-typed-which-resolve-error.md):
  Typed executable resolver error decision record for `which` and
  `command_available`.
- [adr-006-adopt-polonius-nightly-toolchain.md](adr-006-adopt-polonius-nightly-toolchain.md):
  Pinned-nightly Polonius borrow-checker adoption decision record.
- [adr-007-publish-as-netsuke-build.md](adr-007-publish-as-netsuke-build.md):
  crates.io package rename decision record, and the package-versus-target
  naming rule it establishes.
- [adr-008-environment-seam-taxonomy.md](adr-008-environment-seam-taxonomy.md):
  Environment seam taxonomy decision record: three sanctioned shapes for
  injecting environment-dependent input instead of reading the process directly.
- [adr-009-bounded-redacted-manifest-telemetry.md](adr-009-bounded-redacted-manifest-telemetry.md):
  Manifest telemetry decision record, separating observability from evaluation
  and bounding and redacting the emitted metrics and spans.
- [adr-010-scope-glob-capability-to-literal-prefix.md](adr-010-scope-glob-capability-to-literal-prefix.md):
  Glob capability-scoping decision record, opening the metadata capability at a
  pattern's literal directory prefix instead of an ambient root.
- [adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md](adr-011-use-ninja-dyndep-for-serial-dependency-ordering.md):
  Serial `deps` ordering decision record, covering staged Ninja dyndep bundles,
  their scoped execution guarantee, and generated-state ownership.
- [ADR-012](adr-012-bound-dyndep-sidecar-retention.md):
  Deterministic retention, lease, and failure-boundary policy for generated
  dyndep sidecars.
- [ADR-013](adr-013-application-owned-configuration-observability.md):
  Application-owned configuration-load metrics, verbose snapshots, and bounded
  label vocabulary.
- [ADR-014: backend text escaping](adr-014-backend-text-escaping-seam.md):
  Ninja backend escaping boundary decision record, preserving ordinary shell
  dollar syntax in manifests without coupling the IR to Ninja.
- [ADR-015](adr-015-use-bounded-git-cli-for-change-detection.md):
  Feature-private, bounded Git CLI queries for standard-library change
  detection.
- [ADR-014: base-directory seam](
  adr-014-base-directory-seam-and-dir-anchoring.md): Base-directory seam for
  manifest and glob resolution, explicit-selector independence from `-C`, and
  the in-process environment-mutation gate.
- [ADR-016: public CLI metadata source of truth](
  adr-016-public-cli-metadata-source-of-truth.md):
  Release-help metadata composition boundary for configuration fields,
  parser-only selectors, and subcommands.

## Proposals

- [rfcs/](rfcs/): Requests for Comments proposing changes that need technical
  review before they become binding.
  - [rfcs/0006-ansible-inspired-template-standard-library.md](rfcs/0006-ansible-inspired-template-standard-library.md):
    Survey of the ansible-core Jinja standard library, with an explicit
    disposition for every candidate helper and Netsuke-native contracts for
    the accepted set.

## User and operator guides

- [quickstart.md](quickstart.md): First-run walkthrough for building with
  Netsuke.
- [v0-1-0-migration-guide.md](v0-1-0-migration-guide.md): Migration notes for
  the v0.1.0 child-environment API, glob behaviour, and serial-dependency
  additions, plus the stability caveat that covers them.
- [v0-1-1-migration-guide.md](v0-1-1-migration-guide.md): Migration note for
  replacing no-op aggregate recipes with dependency-only actions or targets.
- [users-guide.md](users-guide.md): End-user reference for authoring and
  running Netsuke manifests, including executable discovery and
  `command_available` branch selection.
- [stdlib-yaml-and-jinja-guide.md](stdlib-yaml-and-jinja-guide.md): Complete
  template standard-library reference with executable YAML and Jinja examples.
- [ortho-config-users-guide.md](ortho-config-users-guide.md): Configuration
  system guide and precedence reference.
- [ortho-config-v0-9-0-migration-guide.md](ortho-config-v0-9-0-migration-guide.md):
  Migration guidance for the OrthoConfig v0.9.0 release.
- [translators-guide.md](translators-guide.md): Localization workflow,
  translation guidance, the locale registry that owns the supported-tag list,
  and the fallback policy that keeps regional and script variants distinct.
- [localization-glossary.md](localization-glossary.md): Terminology source of
  truth for Netsuke product names, manifest concepts, and identifiers, with
  per-locale term mappings for every shipped locale.
- [localization-styleguide.md](localization-styleguide.md): House style for
  user-facing text and how voice, tone, register, and mechanics carry across
  Netsuke locales.

## Contributor guidance

- [developers-guide.md](developers-guide.md): Engineering workflow, quality
  gates, Lading release configuration, local build acceleration, testing
  strategy, and stdlib resolver-boundary conventions.
- [polonius.md](polonius.md): Polonius migration audit, borrow-centric API
  evolution log, and principled refusals.
- [documentation-style-guide.md](documentation-style-guide.md): Documentation
  conventions, roadmap-writing rules, and Markdown requirements.
- [scripting-standards.md](scripting-standards.md): Python scripting standards
  for repository automation scripts, covering the Cyclopts CLI pattern,
  `cuprum` command execution, `pathlib` usage, and pytest coverage rules.
- [execplans/](execplans/): Execution plans and implementation handoff notes.

## Testing and quality references

- [behavioural-testing-in-rust-with-cucumber.md](behavioural-testing-in-rust-with-cucumber.md):
  Historical behavioural-testing background.
- [reliable-testing-in-rust-via-dependency-injection.md](reliable-testing-in-rust-via-dependency-injection.md):
  Dependency-injection testing patterns used by the project.
- [rstest-bdd-users-guide.md](rstest-bdd-users-guide.md): Current behavioural
  testing framework reference.
- [rstest-bdd-v0-5-0-migration-guide.md](rstest-bdd-v0-5-0-migration-guide.md):
  Migration notes for the current `rstest-bdd` release.
- [rust-doctest-dry-guide.md](rust-doctest-dry-guide.md): Doctest workflow and
  dry-run guidance.
- [rust-testing-with-rstest-fixtures.md](rust-testing-with-rstest-fixtures.md):
  `rstest` fixture patterns used in the repository.
- [whitaker-users-guide.md](whitaker-users-guide.md): Installing and
  configuring the Whitaker Dylint lint suite enforced by `make lint`.
- [snapshot-testing-in-netsuke-using-insta.md](snapshot-testing-in-netsuke-using-insta.md):
  Snapshot-testing strategy and examples.
- [test-isolation-with-ninja-env.md](test-isolation-with-ninja-env.md): Test
  isolation strategy for Ninja process interactions.
- [security-network-command-audit.md](security-network-command-audit.md):
  Security review of network and command-execution surfaces.
