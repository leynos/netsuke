# Documentation contents

This index groups the primary Netsuke documentation by purpose so design,
operational, and contributor references are easier to find.

## Core design and planning

- [netsuke-design.md](netsuke-design.md): Primary architecture, manifest, and
  execution design document.
- [netsuke-cli-design-document.md](netsuke-cli-design-document.md): Command-line
  interface design and user-experience requirements.
- [roadmap.md](roadmap.md): Phased implementation plan and tracked delivery
  work.
- [formal-verification-methods-in-netsuke.md](formal-verification-methods-in-netsuke.md):
  Recommended scope and delivery order for Kani, Proptest, and optional Verus
  checks.
- [adr-001-replace-serde-yml-with-serde-saphyr.md](adr-001-replace-serde-yml-with-serde-saphyr.md):
  YAML parser migration decision record.
- [adr-002-replace-cucumber-with-rstest-bdd.md](adr-002-replace-cucumber-with-rstest-bdd.md):
  Behavioural-testing framework migration decision record.

## User and operator guides

- [quickstart.md](quickstart.md): First-run walkthrough for building with
  Netsuke.
- [users-guide.md](users-guide.md): End-user reference for authoring and
  running Netsuke manifests.
- [ortho-config-users-guide.md](ortho-config-users-guide.md): Configuration
  system guide and precedence reference.
- [translators-guide.md](translators-guide.md): Localization workflow and
  translation guidance.

## Contributor guidance

- [developers-guide.md](developers-guide.md): Engineering workflow, quality
  gates, and testing strategy.
- [documentation-style-guide.md](documentation-style-guide.md): Documentation
  conventions, roadmap-writing rules, and Markdown requirements.
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
- [snapshot-testing-in-netsuke-using-insta.md](snapshot-testing-in-netsuke-using-insta.md):
  Snapshot-testing strategy and examples.
- [test-isolation-with-ninja-env.md](test-isolation-with-ninja-env.md): Test
  isolation strategy for Ninja process interactions.
- [security-network-command-audit.md](security-network-command-audit.md):
  Security review of network and command-execution surfaces.
