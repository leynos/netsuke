# Adopt OrthoConfig v0.9.0 without weakening Netsuke's boundaries

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Netsuke currently uses `ortho_config` v0.8.0 at runtime and build time, and its
release workflow installs `cargo-orthohelp` v0.8.0. This migration moves those
three consumers to the compatible v0.9.0 release family while retaining
Netsuke's established command-line interface (CLI), configuration precedence,
localized errors, two-pass project discovery, and release-help paths.

After the change, a user can keep using the same configuration files, flags, and
`NETSUKE_*` variables. An absent optional configuration still falls back to
defaults, whereas a candidate that exists but cannot be loaded is reported as
an error. Release builds generate their man page and PowerShell help with
`cargo-orthohelp` v0.9.0. Tests that inject environment values control
automatic discovery as well as configuration-value merging, without reading or
mutating the test process's environment.

Observable success means all of the following are true:

1. `Cargo.toml` and `Cargo.lock` resolve both direct `ortho_config`
   requirements to v0.9.0, and the release workflow installs and validates
   `cargo-orthohelp` v0.9.0.
2. Existing configuration precedence remains defaults, discovered files,
   `NETSUKE_*` environment values, then explicit CLI values; `--config` still
   outranks `NETSUKE_CONFIG` and bypasses automatic discovery.
3. Automatic discovery distinguishes no candidate from candidates that all
   fail, and injected discovery tests cannot fall through to the host's home or
   platform configuration directories.
4. Netsuke's customized localized value parsers still produce localized help
   and localized parse failures through OrthoConfig v0.9.0's combined parsing
   entry point.
5. The existing man-page and PowerShell generation workflow succeeds with the
   v0.9.0 documentation metadata.
6. `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass after
   every major implementation milestone. Documentation gates pass before the
   migration is complete.

This draft is a plan only. Per the `execplans` skill's approval gate, do not
implement it until the user explicitly approves the draft.

## Constraints

- Preserve the current public CLI and configuration contract unless a verified
  v0.9.0 incompatibility makes that impossible. In particular, preserve
  `--config`, `NETSUKE_CONFIG`, `-C/--directory`, `NETSUKE_*` value merging,
  default-command resolution, and the documented precedence order.
- Preserve the accepted decision in
  `docs/adr-004-explicit-config-selection-outside-orthoconfig.md`: Netsuke's
  CLI adapter, not OrthoConfig attributes, owns explicit selector precedence,
  early diagnostic selection, and fail-closed selected-file handling.
- Keep `CliConfig` in `src/cli/config.rs` as the configuration policy model.
  It may derive OrthoConfig traits, but it must not acquire filesystem,
  process-environment, tracing-subscriber, or command-execution concerns.
- Keep process-backed environment access at composition roots. Domain and
  policy queries receive data through the existing `ConfigEnvProvider` port;
  tests must not call `std::env::set_var` or `std::env::remove_var`.
- Treat OrthoConfig discovery and file loading as driven adapters. Netsuke's
  CLI orchestration may compose them, but domain and manifest modules must not
  depend on OrthoConfig discovery types.
- Do not transplant a canonical hexagonal directory tree into Netsuke. Apply
  the `$hexagonal-architecture` dependency rule to the existing feature-based
  modules: policy points inward; `ProcessEnv`, `MapEnv`, Figment, files, clap,
  and `cargo-orthohelp` remain adapters.
- Preserve the two-pass project discovery required by `--directory` and the
  append merge strategy. Do not replace it with the v0.9.0 `discovery(...)`
  derive attribute unless tests prove identical layer order, inherited-layer
  handling, selector precedence, and diagnostics.
- Preserve Netsuke's custom clap value-parser configuration. Do not adopt
  `LocalizedParse` directly because it cannot insert
  `configure_validation_parsers`; use `parse_localized_command` only after
  supplying the already localized and configured command.
- Keep `ortho_config`'s `yaml` feature disabled. Netsuke configuration is TOML;
  Netsuke manifest YAML is a separate `serde-saphyr` boundary and is not part
  of this dependency's YAML 1.2 migration.
- Retain direct dependencies that Netsuke source genuinely imports. In
  particular, do not remove `serde-saphyr`, `serde_json`, or `fluent-bundle`
  merely because OrthoConfig re-exports implementation dependencies.
- Use caret requirements for every new or changed dependency. The required
  versions are `ortho_config = "0.9.0"`, `googletest = "0.14.3"`, and
  `pretty_assertions = "1.4.1"`; Cargo's plain version syntax supplies caret
  semantics.
- Follow Red-Green-Refactor. A focused test must fail for the expected reason
  before each behavioural production change, then pass after the smallest
  implementation, and remain passing after cleanup.
- Use `rstest` for new parameterized unit and integration coverage,
  `rstest-bdd` for user-visible acceptance behaviour, `googletest` matchers for
  structural and error assertions, and `pretty_assertions` for collection or
  metadata equality where a diff aids diagnosis.
- Preserve current `insta` snapshots unless output intentionally changes.
  Review new snapshots before accepting them; never use automatic snapshot
  acceptance as the oracle.
- Keep all Rust source files below 400 lines. Extract only cohesive helpers,
  first sweeping the repository for an equivalent. Document any new helper's
  ownership, permitted call sites, and composition rules in
  `docs/developers-guide.md`.
- Use the dated nightly in `rust-toolchain.toml` and Polonius. Do not add clones
  or NLL-era lookup workarounds to make the migration compile.
- Use en-GB-oxendict prose, wrap Markdown prose at 80 columns, and follow
  `docs/documentation-style-guide.md`.
- Update `docs/netsuke-design.md` for the migration decisions,
  `docs/developers-guide.md` for internal seams and release tooling, and
  `docs/users-guide.md` for any observable error or discovery clarification. If
  implementation requires a new public architectural commitment, stop and add
  an architectural decision record (ADR) in the style required by
  `docs/documentation-style-guide.md`, then reference it from the design.
- Do not commit a milestone unless all gates assigned to it pass. Keep
  functional changes and any later discretionary refactor in separate atomic
  commits.

## Tolerances (exception triggers)

- Scope: if the migration needs more than 18 changed files excluding
  `Cargo.lock`, snapshots, and this ExecPlan, or more than 900 net lines, stop
  and split or re-scope the work with the user.
- Public interface: if an exported Netsuke type or function must change, stop
  and present the compatibility options. Adding a private adapter helper does
  not trigger this threshold.
- Dependencies: if production needs any new dependency other than the requested
  `ortho_config` update, stop. The requested test-only assertion dependencies
  do not trigger this threshold.
- Discovery: if v0.9.0 cannot preserve current project/user layer order or
  `extends` append semantics through `compose_layers`, stop rather than
  approximating the old behaviour.
- Localization: if `parse_localized_command` changes any established help or
  error snapshot beyond an expected v0.9.0 correction, stop and compare keeping
  the current manual path against adopting the combined path.
- Release help: if a real v0.9.0 generation smoke changes a staged path, module
  layout, or public help content, stop before updating packaging and present
  the user-visible difference.
- Test iterations: if a focused red test does not turn green after three
  implementation attempts, record the evidence in `Decision Log` and stop.
- Full gates: if the same full gate fails after three focused correction
  cycles, stop and report the exact command and remaining failures.
- Milestone duration: if one milestone takes more than four hours without a
  passing focused checkpoint, record the partial state and stop.
- Ambiguity: if two valid interpretations materially change configuration or
  release behaviour, stop and present both options rather than choosing
  silently.

## Risks

- Risk: v0.9.0's accumulated discovery errors expose a malformed optional file
  that v0.8.0 effectively ignored. Severity: high. Likelihood: medium.
  Mitigation: specify absent, malformed, unreadable, and successful candidate
  cases before the upgrade; accept the new error only where it matches the
  migration guide, and document it for users.

- Risk: passing a test `MapEnv` to automatic discovery while leaving value
  merging on another environment source produces split-brain tests. Severity:
  high. Likelihood: medium. Mitigation: make the private merge orchestration
  accept both capabilities and have the injected public entry point derive the
  discovery `MapEnv` from the same `ConfigEnvProvider` fixture. The ambient
  entry point uses `ProcessEnv`, preserving platform home fallback in
  production.

- Risk: adapting all environment entries into OrthoConfig's `EnvSource` would
  grant discovery an unnecessary enumeration capability or lose non-Unicode
  values. Severity: medium. Likelihood: medium. Mitigation: snapshot only
  OrthoConfig's documented discovery keys (`NETSUKE_CONFIG`, `HOME`,
  `USERPROFILE`, `XDG_CONFIG_HOME`, `XDG_CONFIG_DIRS`, `APPDATA`, and
  `LOCALAPPDATA`) into `MapEnv`; retain the existing entry enumeration solely
  for Netsuke's `NETSUKE_*` merge adapter.

- Risk: `parse_localized_command` uses `FromArgMatches` rather than Netsuke's
  current `from_arg_matches_mut` call and could alter custom-parser behaviour.
  Severity: medium. Likelihood: low. Mitigation: add focused happy and unhappy
  parser tests before replacing the glue, retain the configured command, and
  compare localized snapshots.

- Risk: v0.9.0 documentation metadata or `cargo-orthohelp` changes generated
  man-page or PowerShell output. Severity: high. Likelihood: medium.
  Mitigation: pin a compact metadata snapshot, update workflow contract tests
  first, then run the real generator for Unix and Windows formats before
  changing any packaging expectation.

- Risk: adding two assertion libraries creates inconsistent test idioms.
  Severity: low. Likelihood: medium. Mitigation: use `googletest` for
  matcher-oriented structure and error inspection, `pretty_assertions` only for
  equality with useful diffs, and document these narrow roles rather than
  mechanically rewriting existing tests.

- Risk: the generic migration guide's YAML warning is mistaken for a Netsuke
  manifest migration. Severity: medium. Likelihood: low. Mitigation: keep the
  OrthoConfig `yaml` feature off and state explicitly in design and developer
  documentation that `serde-saphyr` continues to own Netsukefile parsing.

## Progress

- [x] (2026-08-12 16:03Z) Read the OrthoConfig v0.9.0 migration and user
  guides, Netsuke design and contributor guidance, current configuration code,
  tests, release workflow contracts, and repository layout.
- [x] (2026-08-12 16:03Z) Confirm `ortho_config` and `cargo-orthohelp` v0.9.0
  exist and require Rust 1.89.0; the repository's dated 2026 nightly is new
  enough to satisfy that declared minimum.
- [x] (2026-08-12 16:03Z) Draft this ExecPlan with architecture boundaries,
  Red-Green-Refactor milestones, coverage choices, and exception thresholds.
- [x] (2026-08-12 16:35Z) Obtain explicit approval for the draft before
  implementation.
- [x] (2026-08-12 16:35Z) Start Milestone 1 with a clean working tree and
  record the v0.8.0 baseline: `make check-fmt`, `make typecheck`, `make lint`,
  and `make test` all passed. The non-doctest suite reported 1,917 passing
  tests and one skip.
- [x] (2026-08-12 17:08Z) Milestone 1: captured the baseline and added the
  red workflow-pin and injected-XDG-discovery tests. The first failed because
  the workflow still named v0.8.0; the second returned no injected XDG layer
  before `MapEnv` was wired into discovery.
- [x] (2026-08-12 17:08Z) Milestone 2: upgraded both `ortho_config`
  requirements and the release-tool workflow pin to v0.9.0, added the required
  assertion dependencies, and updated the resolved lockfile.
- [x] (2026-08-12 17:08Z) Milestone 3: made injected automatic discovery
  hermetic, retained ambient `ProcessEnv` in production, and added unit,
  behavioural, and closed-environment end-to-end coverage for absent, valid,
  malformed, and missing-parent candidate outcomes.
- [x] (2026-08-12 17:08Z) Milestone 4: adopted `parse_localized_command`
  after the existing parser happy- and unhappy-path suites preserved Netsuke's
  configured localized value-parser contract.
- [x] (2026-08-12 17:08Z) CodeRabbit reviewed Milestones 1–4 at `eba40a18`
  after browser OAuth authentication completed. `coderabbit review --agent`
  reported zero high, medium, or low findings.
- [x] (2026-08-12 17:08Z) Milestone 5: installed `cargo-orthohelp` v0.9.0,
  changed the release helper to invoke its required `orthohelp` subcommand, and
  generated the expected Unix and Windows artefact layouts. The compact
  metadata snapshot and workflow/release-helper contracts are green.
- [x] (2026-08-12 18:31Z) Milestone 6: updated the design, explicit-selector
  ADR, developer conventions, user guide, and decision-record index. The
  documentation records the v0.9.0 discovery adapter, failure distinction,
  release-helper invocation, and deliberate metadata follow-up boundary.
- [x] (2026-08-12 18:42Z) Milestone 7: reran the complete deterministic gate
  suite and reviewed the documentation diff with CodeRabbit. All checks and
  reviews are green; the outcomes and deferred metadata follow-up are recorded
  below.
- [x] (2026-08-14) Verified the follow-up review findings against the rebased
  tree. One stale documentation signature was corrected in
  `docs/developers-guide.md`, changing
  `project_scope_file_str(...) -> Option<String>` to
  `project_scope_file(...) -> Option<PathBuf>`; all other findings were already
  fixed.
- [x] (2026-08-14) Replayed the initial rebase's `Cargo.lock` conflict by
  taking the `origin/main` baseline, then applying the narrow
  `cargo update -p ortho_config --precise 0.9.0` update. This avoided the
  unrelated churn from `cargo generate-lockfile`; the resulting change was
  committed as `642bac70`.
- [x] (2026-08-14) Rebased cleanly again onto `origin/main` at `69286cdf`.
  Mainline's fixture no-clobber changes had no pertinent overlap with this
  branch, so no additional plan or implementation change was required. The
  final branch head is `c12ce72a`.
- [x] (2026-08-14) Ran the exact post-rebase gates: `make check-fmt`,
  `make test` (1,992 non-doctests and all doctests), `make typecheck`, and
  `make lint`; all passed. Published PR #558 at the final branch head.
- [x] (2026-08-15) Addressed the post-turn Whitaker
  `no_std_fs_operations` finding: `build.rs` and
  `build_l10n_audit::read_source` now use capability-scoped directory reads,
  while discovery retains only the dedicated path-normalization module
  exclusions at `netsuke::cli::discovery::paths` and
  `build_script_build::cli::discovery::paths`, needed for
  OrthoConfig-compatible absolute comparison keys and cross-directory symlinks.
  The focused `normalized_path_key_follows_cross_directory_symlinks` test
  proves that compatibility. The broad discovery-module and build-script crate
  exclusions were removed; the final gate evidence is recorded below.
- [x] (2026-08-15) Completed the post-remediation gates. The initial
  `make lint` run found only three Clippy `doc_markdown` backtick omissions in
  `discovery_paths` Rustdoc; after correction, `make check-fmt`, `make test`
  (1,993 nextest tests, one skipped; 100 doctests passed, 28 ignored),
  `make typecheck`, `make lint` (docs, Clippy, and Whitaker),
  `make markdownlint` (81 files, 0 errors), `make nixie`, and
  `git diff --check` all passed.
- [x] (2026-08-16) Addressed the follow-up review findings. Man-page
  replacement now delegates directly to `Dir::rename`, avoiding a
  pre-delete gap so a failed replacement preserves the existing page. The
  `dylint.toml` comment now accurately describes the module-scoped
  `build_script_build::cli::discovery::paths` exception rather than claiming
  that the whole `build_script_build` crate is excluded.

## Surprises & discoveries

- Observation: Netsuke does not call `ConfigDiscovery::load_first`; it calls
  `compose_layers` so each inherited file remains a separate merge layer.
  Evidence: `src/cli/discovery_layers.rs` builds a discovery scanner and reads
  `DiscoveryLayersOutcome::{value,required_errors,optional_errors}`. Impact:
  the v0.9.0 `load_first` change is not a source-level break, but the same
  absent-versus-failed outcome must be pinned around `compose_layers`.

- Observation: injected `ConfigEnvProvider` values currently select explicit
  files and feed the `NETSUKE_*` merge layer, but automatic `ConfigDiscovery`
  still uses its process-backed default. Evidence:
  `collect_file_layers_with_env` calls `collect_file_layers`, whose
  `config_discovery` builder does not call `env_source`. Impact: v0.9.0's
  `MapEnv` can close a real hermeticity gap without replacing Netsuke's port.

- Observation: Netsuke needs a configured clap command before parsing because
  localized enum validators are installed by `configure_validation_parsers`.
  Evidence: `parse_with_localizer_from` localizes `Cli::command`, configures
  the parsers, parses matches, and localizes both failure paths. Impact: use
  `parse_localized_command`, not `LocalizedParse`, and preserve the
  preprocessing sequence.

- Observation: the OrthoConfig `yaml` feature is not enabled. Netsuke's direct
  `serde-saphyr` dependency parses build manifests, not Netsuke configuration.
  Evidence: both `ortho_config` requirements enable only `serde_json`, while
  `serde-saphyr` is a separate normal dependency. Impact: YAML 1.2 Boolean and
  duplicate-key migration work is out of scope for configuration, and manifest
  parsing must not be changed accidentally.

- Observation: the release workflow and its Rust contract tests hard-code
  `cargo-orthohelp` v0.8.0 independently from Cargo's runtime and build
  dependencies. Evidence: `.github/workflows/build-and-package.yml` and
  `tests/workflow_build_and_package.rs` both assert v0.8.0. Impact: the tool
  pin and its tests are a required part of the atomic version migration.

- Observation: the accepted explicit-selection ADR still describes
  `env_config_path` as reading `std::env::var_os`, although the current code
  has already replaced that ambient read with `ConfigEnvProvider` injection.
  Evidence: `docs/adr-004-explicit-config-selection-outside-orthoconfig.md`
  names the old implementation, while `src/cli/discovery.rs` takes
  `&impl EnvProvider`. Impact: update the existing ADR's implementation
  consequences during the documentation milestone; do not create a competing
  ADR for the v0.9.0 adapter refinement.

- Observation: neither `googletest` nor `pretty_assertions` is currently a
  dependency. Evidence: no match exists in `Cargo.toml` or `Cargo.lock`;
  crates.io reports current compatible releases 0.14.3 and 1.4.1 respectively.
  Impact: add them as test-only caret requirements and document their distinct
  use rather than rewriting unrelated tests.

- Observation: `CliConfig::get_doc_metadata()` exposes field sources and
  precedence but not merge strategy. Evidence: OrthoConfig v0.9.0's
  `FieldMetadata` has CLI, environment, and file metadata but no merge-policy
  member. Impact: the new compact snapshot derives Netsuke's four append fields
  from the configuration policy and marks the remaining fields as replace,
  avoiding a full upstream-structure snapshot.

- Observation: the initial adapter hand-off created six arguments in
  `push_file_layers_with_sources`, which Clippy rejects. Evidence: the first
  full lint reported `clippy::too_many_arguments`. Impact: `DiscoverySources`
  now groups the Netsuke `EnvProvider` port with the narrow OrthoConfig
  discovery adapter, documenting a real composition boundary instead of
  suppressing the lint.

- Observation: Whitaker rejects direct `std::fs` operations in the new E2E
  target. Evidence: `make lint` reported `no_std_fs_operations` for three
  fixture writes. Impact: the test uses the established `test_support::fs`
  fixture boundary and leaves production capability policy intact.

- Observation: the initial CodeRabbit Milestones 1–4 review required browser
  OAuth authentication. Evidence: `coderabbit review --agent` initially emitted
  `awaiting_browser_auth`, and the rerun completed after authentication with
  zero findings. Impact: this was an external authentication prerequisite, not
  a code concern or rate limit; every completed milestone has a clean
  CodeRabbit review.

- Observation: real v0.9.0 release-help generation requires
  `cargo-orthohelp orthohelp`, whereas the previously pinned command accepted
  generator options directly. Evidence: direct invocation rejected `--format`
  and instructed callers to use the subcommand; both Unix and Windows runs
  succeeded after the helper added it. Impact: the script and a red-green
  helper-contract assertion now pin the new invocation shape.

- Observation: generated help contains parser-only schema omissions rather
  than a v0.9.0 output-path failure. Evidence: the generated man page has no
  `--config` flag or subcommands and renders missing Fluent IDs because
  `CliConfig` has no parser-only selector field or `OrthoConfigSubcommandDocs`
  metadata. Impact: retain the generated artefacts and record configuration and
  parser metadata convergence as follow-up work; do not expand this version
  migration beyond its approved boundary.

- Observation: the follow-up review had one still-valid documentation finding;
  the implementation findings were already fixed. Evidence:
  `docs/developers-guide.md` described `project_scope_file_str(...)` returning
  `Option<String>`, while the current API is
  `project_scope_file(...) -> Option<PathBuf>`. Impact: correct that stale
  signature only; no runtime change was required.

- Observation: the initial rebase exposed a `Cargo.lock` conflict, and a
  whole-lockfile regeneration introduced unrelated dependency churn. Evidence:
  the conflict was resolved with the mainline lockfile as the baseline, then a
  precise `cargo update -p ortho_config --precise 0.9.0` restored the intended
  package update. Impact: preserve the mainline lockfile and avoid
  `cargo generate-lockfile` for this narrow migration.

- Observation: a later mainline change did not overlap this migration's
  relevant files. Evidence: the clean rebase onto `origin/main` at `69286cdf`
  showed no pertinent overlap with the fixture no-clobber changes. Impact: no
  additional conflict resolution or plan amendment was needed.

- Observation: the post-turn Whitaker lint found valid ambient filesystem I/O
  in the build script, the localization audit's `read_source`, and the
  discovery path normalizer. Evidence: the `no_std_fs_operations` report
  identified those operations after the earlier migration gates. Impact:
  build-script and audit reads now open directory capabilities with `cap_std`,
  and the stale build-script crate and broad discovery-module exclusions are
  removed; only the dedicated path-normalization modules remain excluded.

- Observation: capability-scoped canonicalization cannot replace the discovery
  normalizer completely. Evidence: the focused Unix test
  `normalized_path_key_follows_cross_directory_symlinks` requires an alias in
  one directory to resolve to its target in another, which `cap_std` rejects
  when the symlink leaves the capability root. Impact: retain absolute
  canonical comparison keys and this cross-directory symlink behaviour through
  `std::fs::canonicalize`, with exclusions limited to the dedicated
  path-normalization modules in the library and build-script paths rather than
  exempting the broader discovery module or build-script crate.

- Observation: the first post-remediation gate set found only documentation
  lint omissions. Evidence: `make check-fmt`, `make test` (1,993 nextest tests
  and doctests), `make typecheck`, `make markdownlint`, `make nixie`, and
  `git diff --check` passed, while `make lint` reported three Clippy
  `doc_markdown` backtick omissions in `discovery_paths` Rustdoc. Impact: the
  three omissions were corrected, and the final lint rerun passed with the
  documentation, Clippy, and Whitaker checks green.

- Observation: Whitaker `excluded_paths` matches module boundaries rather than
  arbitrary nested implementation scopes. Evidence: the attempted inner
  `ambient` submodule exclusion did not match the reported path, while the
  dedicated `discovery::paths` module did. Impact: replace the unsupported
  inner scope with the minimal library and build-script `discovery::paths`
  exclusions; the broader discovery module and build-script crate remain
  covered by the capability policy.

- Observation: the man-page replacement path must not remove the existing page
  before installing the temporary output. Evidence: the review identified the
  `metadata`/`remove_file` pair immediately before `Dir::rename` in
  `build.rs`. Impact: direct `Dir::rename` leaves the current page in place if
  replacement fails, preserving the atomic replacement boundary.

- Observation: the `dylint.toml` explanatory comment had drifted from the
  configured scope. Evidence: `build_script_build` is not present in
  `excluded_crates`; only `build_script_build::cli::discovery::paths` is
  excluded by module path. Impact: revise the comment to describe the actual
  narrow exception and avoid implying a broader capability-policy bypass.

## Decision Log

- Decision: preserve the current feature-based module layout and apply
  hexagonal dependency direction within it. Rationale: the migration changes
  infrastructure integrations, not Netsuke's bounded context. A
  directory-pattern transplant would create churn without protecting an
  additional boundary. Date/Author: 2026-08-12 / Codex.

- Decision: retain `ConfigEnvProvider` as Netsuke's environment port and adapt
  injected values to OrthoConfig's `MapEnv` at the discovery boundary.
  Rationale: domain-facing code should not depend on an upstream discovery
  trait, while the adapter can use v0.9.0's hermetic source without process
  mutation. Date/Author: 2026-08-12 / Codex.

- Decision: retain manual two-pass discovery rather than adopting the new
  `#[ortho_config(discovery(...))]` attribute. Rationale: Netsuke adds a
  `--directory`-anchored project pass, de-duplicates canonicalized paths,
  preserves inherited append layers, and deliberately bypasses discovery for
  explicit selectors. The derive attribute does not express that entire
  application policy. Date/Author: 2026-08-12 / Codex.

- Decision: treat ADR 004 as controlling the explicit-selection boundary and
  update its stale environment-access detail during migration. Rationale: the
  accepted ownership decision remains correct, but leaving its
  direct-process-read description unchanged would contradict the current
  injected port and the v0.9.0 discovery adapter. Date/Author: 2026-08-12 /
  Codex.

- Decision: plan adoption of `parse_localized_command`, but make it a go/no-go
  milestone rather than an unconditional rewrite. Rationale: it removes v0.8.0
  glue and guarantees the whole parse path is localized, while focused tests
  must first prove compatibility with Netsuke's customized value parsers.
  Date/Author: 2026-08-12 / Codex.

- Decision: do not derive `OrthoConfigSubcommandDocs` during this migration.
  Rationale: Netsuke's parser-facing `Commands` enum and configuration-facing
  `CliConfig` are intentionally separate. Making every subcommand an
  OrthoConfig schema is a larger public metadata design decision, not a version
  compatibility fix. Record it as follow-up work if the v0.9.0 metadata smoke
  demonstrates an actual release-help gap. Date/Author: 2026-08-12 / Codex.

- Decision: do not enable OrthoConfig metrics or agent context.
  Rationale: both are optional v0.9.0 capabilities and this migration has no
  new recorder, `context --json` contract, or consumer. Generic capability
  adoption belongs in separately approved product work. Date/Author: 2026-08-12
  / Codex.

- Decision: use existing Proptest selector coverage and do not add Kani or
  Verus work. Rationale: the migration introduces no new mathematical business
  rule, state machine, unsafe boundary, or lemma. The relevant range invariant
  is already expressed by `resolve_config_path_obeys_precedence_invariant`;
  example, behavioural, and end-to-end tests are the proportionate tools for
  adapter compatibility. Date/Author: 2026-08-12 / Codex.

- Decision: record migration choices in `docs/netsuke-design.md`, not a new
  ADR, unless implementation crosses an exception threshold. Rationale: version
  alignment, a private environment adapter, and use of a new upstream helper
  refine the existing configuration architecture without establishing a
  hard-to-reverse system-wide direction. Date/Author: 2026-08-12 / Codex.

- Decision: make `DiscoverySources` a crate-private composition input owned by
  `src/cli/discovery.rs`. Rationale: only CLI merge and early diagnostic
  resolution may pair a Netsuke environment port with either `ProcessEnv` or a
  fixed-key `MapEnv`; no other module may use it as a general environment
  copying utility. Date/Author: 2026-08-12 / Codex.

- Decision: snapshot an application-owned metadata projection rather than
  OrthoConfig's complete IR. Rationale: Netsuke must pin field order, sources,
  its append/replace policy, precedence, discovery declaration, and subcommand
  count, but upstream headings and prose fields are not an application
  contract. Date/Author: 2026-08-12 / Codex.

- Decision: adopt `cargo-orthohelp orthohelp` in the release helper.
  Rationale: it is the v0.9.0 executable's documented direct invocation form,
  preserves Cargo's `cargo orthohelp` user form, and is verified by both a
  helper contract and real Unix/Windows generation. Date/Author: 2026-08-12 /
  Codex.

- Decision: defer parser/config documentation metadata convergence. Rationale:
  exposing `--config`, subcommands, and localized prose in generated help
  requires merging the intentionally separate parser and configuration schema,
  which is a larger design decision than a compatible library upgrade.
  Date/Author: 2026-08-12 / Codex.

- Decision: fix only the stale `project_scope_file` documentation signature and
  retain the other reviewed changes as-is. Rationale: the signature mismatch
  was still present in the current tree, while the remaining review findings
  were already resolved; a broader edit would add unsupported churn.
  Date/Author: 2026-08-14 / Codex.

- Decision: resolve the initial lockfile rebase conflict from the mainline
  baseline, then apply a precise OrthoConfig update. Rationale: this preserves
  dependencies introduced on `origin/main` and limits the migration to the
  required package versions; `cargo generate-lockfile` needlessly refreshed
  unrelated packages. Date/Author: 2026-08-14 / Codex.

- Decision: accept the clean rebase onto `origin/main` at `69286cdf` without
  changes for the fixture no-clobber work. Rationale: inspection found no
  pertinent overlap with the migration's discovery or release-help changes.
  Date/Author: 2026-08-14 / Codex.

- Decision: publish PR #558 at `c12ce72a` after the exact post-rebase gates
  passed. Rationale: `make check-fmt`, `make test`, `make typecheck`, and
  `make lint` all passed, including 1,992 non-doctest tests and the doctests.
  Date/Author: 2026-08-14 / Codex.

- Decision: replace the build script's and localization audit's ambient file
  reads with `cap_std` directory capabilities. Rationale: those reads have
  stable parent-directory boundaries and do not require the ambient authority
  that the post-turn Whitaker finding exposed. Date/Author: 2026-08-15 / Codex.

- Decision: retain narrowly scoped Whitaker exclusions for the dedicated
  path-normalization modules at `netsuke::cli::discovery::paths` and
  `build_script_build::cli::discovery::paths`. Rationale:
  `std::fs::canonicalize` preserves the absolute comparison keys and
  cross-directory symlink behaviour required to match OrthoConfig, while
  `cap_std` rejects that symlink case. The focused existing path test is the
  compatibility evidence; neither the discovery module nor build-script crate
  is broadly exempt, and no Rust `#[expect]` is used for this Dylint lint.
  Date/Author: 2026-08-15 / Codex.

- Decision: record the final post-remediation gate result in this plan.
  Rationale: the focused compatibility evidence and the final gate set both
  passed after correcting three `doc_markdown` findings. The earlier
  post-rebase gate results remain historical evidence, while this later run is
  the final all-green result for the remediation. Date/Author: 2026-08-15 /
  Codex.

- Decision: replace an existing man page with a direct `Dir::rename` from the
  temporary output, without a metadata check or pre-emptive removal.
  Rationale: the capability API can perform the replacement while preserving
  the current page when the rename fails, avoiding a window with no page.
  Date/Author: 2026-08-16 / Codex.

- Decision: describe the build-script filesystem exception in `dylint.toml` as
  module-scoped. Rationale: only
  `build_script_build::cli::discovery::paths` is excluded; the
  `build_script_build` crate remains subject to the capability policy.
  Date/Author: 2026-08-16 / Codex.

## Outcomes & retrospective

The migration is complete. Runtime and build-time dependencies now resolve
`ortho_config` v0.9.0, and release automation installs `cargo-orthohelp`
v0.9.0. The release helper invokes its required `orthohelp` subcommand and a
real Unix and Windows generation smoke produced the expected man page and
PowerShell help layouts.

The existing `ConfigEnvProvider` remains Netsuke's port. Production composes
OrthoConfig `ProcessEnv`; injected paths project only the documented discovery
keys into `MapEnv`. This protects the discovery adapter boundary without
changing the user-facing configuration precedence. Unit, behavioural, and
closed-environment end-to-end tests now cover absent, valid, malformed, and
missing-parent discovery outcomes. The parser uses v0.9.0's combined localized
entry point after Netsuke applies its validation parser configuration.

The new compact metadata snapshot tests Netsuke's stable configuration
projection, while `googletest` and `pretty_assertions` make new diagnostic and
collection assertions clear. Existing Proptest coverage remains sufficient for
the selector precedence invariant; no new Kani or Verus obligation was
introduced. The user guide, design, ADR 004, developer guidance, and document
index now describe the resulting behaviour and ownership conventions.

`make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make markdownlint`, and `make nixie` passed at the 2026-08-14 final milestone,
before the post-turn capability remediation. CodeRabbit reported zero findings
for Milestones 1–4, 5, and 6. The only deferred item is configuration and
parser metadata convergence: generated release help still cannot represent
parser-only `--config` and subcommand metadata. That is a separate public
metadata design decision, not a v0.9.0 compatibility defect.

The follow-up review found one stale documentation signature, which was
corrected in `docs/developers-guide.md`; all other findings were already fixed.
The initial rebase's lockfile conflict was resolved by retaining the mainline
baseline and applying a narrow OrthoConfig update, rather than regenerating the
whole lockfile. A subsequent clean rebase onto `origin/main` at `69286cdf`
confirmed no pertinent overlap with the fixture no-clobber changes. The exact
post-rebase gates (`make check-fmt`, `make test`, `make typecheck`, and
`make lint`) passed, with 1,992 non-doctest tests and all doctests succeeding.
PR #558 was published at final head `c12ce72a`.

The post-turn Whitaker remediation is recorded above: build-script and
localization-audit reads use `cap_std` directory capabilities, and the
discovery normalizer retains only its focused `discovery::paths` module
exclusions. The unsupported inner `ambient` scope was replaced after Whitaker
path-match evidence; the broader discovery module and build-script crate remain
under the capability policy.

The first post-remediation lint run found only three Clippy `doc_markdown`
backtick omissions in `discovery_paths` Rustdoc. After those corrections, the
final gates passed: `make check-fmt`; `make test` with 1,993 nextest tests, one
skipped test, and 100 doctests passed with 28 ignored; `make typecheck`;
`make lint` including docs, Clippy, and Whitaker; `make markdownlint` over 81
files with 0 errors; `make nixie`; and `git diff --check`.

## Context and orientation

The repository is a Rust workspace whose package is `netsuke-build`, while its
library and binary targets are both named `netsuke`. Run every command in this
plan from the repository root:

```plaintext
/data/leynos/Projects/netsuke.worktrees/adopt-ortho-config-v0-9-0
```

The relevant production flow is:

```plaintext
src/main.rs
  -> src/cli/parser.rs parses a localized Cli and retains clap ArgMatches
  -> src/cli/discovery.rs selects explicit or automatic file discovery
  -> src/cli/discovery_layers.rs adapts ConfigDiscovery into MergeLayer values
  -> src/cli/environment.rs adapts NETSUKE_* entries into a Figment provider
  -> src/cli/merge.rs composes defaults, files, environment, and CLI values
  -> src/cli/config.rs validates the CliConfig policy model
  -> src/cli/merge.rs maps the policy model back to the runtime Cli
```

In hexagonal terms, `CliConfig`, the policy enums, selector precedence, and
post-merge validation are inward-facing policy. `ConfigEnvProvider` is a driven
port: it describes the environment capability Netsuke needs. `StdEnvProvider`,
OrthoConfig `ProcessEnv` and `MapEnv`, Figment providers, clap parsing,
configuration files, and `cargo-orthohelp` are adapters. `src/main.rs` and the
public merge/parse wrappers are composition roots that select production or
test adapters.

The dependency and release-tool pins live in:

- `Cargo.toml`: runtime and build-time `ortho_config = "0.8.0"` requirements;
- `Cargo.lock`: resolved runtime, macro, and transitive packages;
- `.github/workflows/build-and-package.yml`: installs and validates
  `cargo-orthohelp = 0.8.0`;
- `tests/workflow_build_and_package.rs`: contract tests for that workflow; and
- `docs/developers-guide.md` and `docs/netsuke-design.md`: documented tool pin.

The existing test surfaces to extend are:

- source-adjacent `rstest` modules under `src/cli/` for discovery and parsing;
- `tests/cli_tests/` for configuration selection and merge integration;
- `tests/features/configuration_discovery.feature` and
  `tests/bdd/steps/configuration_discovery.rs` for acceptance behaviour;
- a dedicated `assert_cmd` integration target for the real binary's
  configuration-failure contract;
- `src/snapshots/` or `tests/snapshots/` for reviewed `insta` output; and
- `tests/workflow_build_and_package.rs` plus
  `tests/release_help_script_tests.rs` for release tooling.

Before editing, read and use these repository sources:

- `docs/contents.md` and `docs/repository-layout.md` for ownership;
- `docs/ortho-config-v0-9-0-migration-guide.md` for required and recommended
  version work;
- `docs/ortho-config-users-guide.md` for v0.9.0 APIs and precedence;
- `docs/netsuke-design.md`, especially section 8.4 and configuration discovery;
- `docs/adr-004-explicit-config-selection-outside-orthoconfig.md` for the
  accepted ownership of explicit selector policy;
- `docs/developers-guide.md`, especially quality gates, test suite map,
  environment seams, configuration merge architecture, and release help;
- `docs/users-guide.md`, especially "Configure Netsuke";
- `docs/rust-testing-with-rstest-fixtures.md` for fixtures and parameterized
  cases;
- `docs/rstest-bdd-users-guide.md` for feature binding and step isolation;
- `docs/rust-doctest-dry-guide.md` for public examples and doctest selection;
- `docs/reliable-testing-in-rust-via-dependency-injection.md` for injected
  environment adapters; and
- `docs/documentation-style-guide.md` for design and ADR decisions.

The implementing agent must use and re-read these skills before acting:

- `$execplans` at `/home/leynos/.codex/skills/execplans/SKILL.md` governs the
  living sections, approval gate, Red-Green-Refactor evidence, and revision
  notes.
- `$hexagonal-architecture` at
  `/home/leynos/.codex/skills/hexagonal-architecture/SKILL.md` governs inward
  dependencies, port ownership, adapter isolation, and layer-specific tests.
- `$leta` at `/home/leynos/.codex/skills/leta/SKILL.md` governs semantic Rust
  navigation; use `leta show`, `leta refs`, and `leta calls` before text
  searches for known symbols.
- `$rust-router` at `/home/leynos/.codex/skills/rust-router/SKILL.md` must route
  implementation questions to the smallest applicable Rust skill.
- `$rust-unit-testing` at
  `/home/leynos/.codex/skills/rust-unit-testing/SKILL.md` applies when adding
  the `rstest` coverage.
- `$commit-message` at
  `/home/leynos/.codex/skills/commit-message/SKILL.md` applies only when the
  user authorizes commits.

## Plan of work

### Milestone 1: capture the baseline and specify v0.9.0 compatibility

First update `Progress` with the start timestamp, confirm the branch and clean
worktree, and run the four required baseline gates. Save concise transcripts in
`Artefacts and notes`; do not change dependencies until their v0.8.0 results
are known.

Add the smallest red tests for the behaviours that v0.9.0 can affect:

1. In a cohesive source-adjacent discovery test module, add `#[rstest]` cases
   for no candidate, a valid candidate, a malformed sole candidate, and a
   missing `extends` parent. Use an injected environment and temporary
   capability-scoped directories. The malformed and missing-parent cases must
   assert an error rather than an empty layer list. Use `googletest` matchers
   to inspect variants and diagnostic fragments.
2. Add a parameterized test proving that an injected `HOME`,
   `XDG_CONFIG_HOME`, or `APPDATA` determines the candidates without consulting
   the host. Compare candidate or layer vectors with `pretty_assertions`.
3. Add focused parser cases showing successful custom policy parsing and a
   localized invalid policy error. These tests pin the contract before adopting
   `parse_localized_command`.
4. Add `tests/ortho_config_metadata_snapshot_tests.rs` with an
   `insta::assert_yaml_snapshot!` of a compact, application-owned projection of
   `CliConfig::get_doc_metadata()`: schema/IR version if exposed, ordered field
   names, sources, merge strategies, discovery metadata, and subcommand count.
   Do not snapshot debug output or the entire upstream structure.
5. Add or extend workflow contract cases so v0.8.0 is demonstrably the old pin
   and changing the workflow without its test fails.

Run each focused test before production edits and record the expected red
failure. Tests that merely characterize already-correct v0.8.0 behaviour may
pass; label them characterization evidence rather than claiming a false red. At
least the dependency/tool pin assertion and the injected automatic-discovery
test must fail before their implementation.

End the milestone by running:

```bash
make check-fmt
make typecheck
make lint
make test
```

The full suite may be red only for the deliberately introduced migration tests.
Record each expected failure by exact test name and reason. Unexpected failures
block Milestone 2.

### Milestone 2: align dependencies and restore compilation

In `Cargo.toml`, update both runtime and build-time requirements to:

```toml
ortho_config = { version = "0.9.0", features = ["serde_json"] }
```

Add the requested assertion libraries under `[dev-dependencies]`:

```toml
googletest = "0.14.3"
pretty_assertions = "1.4.1"
```

Do not add `ortho_config_macros`; v0.9.0 continues to re-export the derive
macros. Keep the existing direct dependencies that source imports. Update the
lockfile narrowly with:

```bash
cargo update -p ortho_config --precise 0.9.0
cargo update -p ortho_config_macros --precise 0.9.0
```

If Cargo updates unrelated packages beyond what the new resolution requires,
inspect `git diff -- Cargo.lock` and regenerate with the narrowest supported
command; do not hand-edit the lockfile.

In `.github/workflows/build-and-package.yml`, change both the installation and
version validation to `cargo-orthohelp` v0.9.0. Update the corresponding
`rstest` assertions in `tests/workflow_build_and_package.rs` so a mixed 0.8/0.9
toolchain fails clearly. Do not yet change help paths or generated-content
expectations.

Compile first, then fix only verified v0.9.0 API changes. Run:

```bash
cargo check --workspace --all-targets --all-features
cargo test --test workflow_build_and_package
make check-fmt
make typecheck
make lint
make test
```

All commands must pass before Milestone 3. If a v0.9.0 compile error suggests a
public API redesign, trigger the interface tolerance instead of hiding it with
a clone, wrapper exposed outside `cli`, or lint suppression.

### Milestone 3: make discovery hermetic and pin failure behaviour

Before adding a helper, use semantic and text searches to confirm there is no
existing adapter from `ConfigEnvProvider` to OrthoConfig `EnvSource` or
`MapEnv`. Record the sweep in `Surprises & Discoveries`.

Refactor only the CLI discovery composition:

1. Let the internal discovery-layer functions accept an OrthoConfig
   `SharedEnvSource` and pass it to `ConfigDiscoveryBuilder::env_source`.
2. Keep the ambient public wrappers (`merge_with_config` and
   `resolve_merged_json`) bound to OrthoConfig `ProcessEnv`, preserving the
   production home-directory fallback.
3. Have the injected wrappers (`merge_with_config_and_env` and
   `resolve_merged_json_with_env`) build a closed `MapEnv` from only the seven
   documented discovery keys listed in `Risks`, using the same
   `ConfigEnvProvider` that supplies selector and `NETSUKE_*` values.
4. Keep complete environment enumeration confined to `EnvironmentLayer`; do
   not add enumeration to OrthoConfig's name-only `EnvSource` capability.
5. Keep explicit selector policy in `resolve_config_selector`. Automatic
   discovery must not become responsible for deciding whether `--config` or
   `NETSUKE_CONFIG` wins.

This helper is owned by `src/cli/discovery.rs` or a colocated discovery adapter
module, may be called only by CLI configuration composition, and may contain
only the fixed discovery-key projection. It is not a general environment-copy
utility. Document those ownership, call-site, and composition rules in
`docs/developers-guide.md` in Milestone 6.

Turn the discovery red tests green, then add the user-visible behavioural and
system coverage. Extend `tests/features/configuration_discovery.feature` with
this synchronized specification:

```gherkin
Feature: Configuration file discovery and precedence
  Netsuke reports broken discovered configuration and keeps absent
  configuration distinct from failed configuration.

  Scenario: No configuration file uses built-in defaults
    Given a temporary workspace
    And no Netsuke configuration file exists
    When the CLI is parsed with no additional arguments
    Then parsing succeeds
    And the default configuration is used

  Scenario: A malformed discovered configuration is an error
    Given a temporary workspace
    And a malformed project config file ".netsuke.toml"
    When the CLI is parsed with no additional arguments
    Then an error should be returned
    And the merge error identifies the configuration load failure
```

Use existing world fixtures and steps where possible. New `Given` steps may
write only inside the scenario workspace; the `When` step must exercise the
same public parse-and-merge path as existing discovery scenarios. Keep the
`Then` wording user-observable and avoid asserting internal adapter names.

Add a dedicated `assert_cmd` end-to-end test target if no current target drives
this exact binary path. It must run `netsuke` with `env_clear`, an explicit
child environment, a temporary workspace, and a malformed automatically
discovered `.netsuke.toml`; assert non-zero exit and the stable application
diagnostic category, not the whole upstream error sentence. Pair it with a
happy-path run that has no configuration and reaches normal manifest handling.

Run focused then full validation:

```bash
cargo nextest run --lib 'cli::discovery'
cargo nextest run --test cli_tests
cargo nextest run --test bdd_tests
cargo nextest run --test config_discovery_e2e_tests
make check-fmt
make typecheck
make lint
make test
```

Use the actual integration-test target name if the test is colocated in an
existing target. Update this plan with that name. All commands must pass before
Milestone 4.

### Milestone 4: consolidate the localized parse adapter

With the parser compatibility tests already green on v0.9.0, replace the manual
match parsing and error relocalization in `parse_with_localizer_from` with
`ortho_config::parse_localized_command`. Build and localize `Cli::command`, run
`configure_validation_parsers`, then pass that configured command, the argument
iterator, and the same localizer to the combined helper. Preserve the
`(Cli, ArgMatches)` return type and every public caller.

Run the focused happy and unhappy tests after the smallest edit. Then run the
existing localized help snapshots and BDD invalid-policy scenarios. If any
unexpected snapshot changes, stop under the localization tolerance; do not
accept them automatically.

```bash
cargo nextest run --lib 'cli::parser'
cargo nextest run --test cli_tests
cargo nextest run --test bdd_tests
cargo nextest run --test novice_flow_smoke_tests
make check-fmt
make typecheck
make lint
make test
```

If compatibility cannot be proved, retain the current manual parser code,
record the reason in `Decision Log`, and continue the version migration. The
v0.9.0 combined helper is recommended, not required.

### Milestone 5: verify metadata and release-help adapters

Turn the compact metadata snapshot green and review it. It should prove the
application-owned metadata contract without binding Netsuke to every private
field or formatting choice in OrthoConfig.

Install and validate the exact tool used by continuous integration (CI), then
run the generator for both supported output families:

```bash
cargo install cargo-orthohelp --version 0.9.0 --locked
cargo-orthohelp --version
scripts/generate-release-help.sh \
  x86_64-unknown-linux-gnu netsuke \
  target/orthohelp/x86_64-unknown-linux-gnu/release Netsuke
scripts/generate-release-help.sh \
  x86_64-pc-windows-msvc netsuke \
  target/orthohelp/x86_64-pc-windows-msvc/release Netsuke
```

Expect the reported version to contain `0.9.0`, the Unix run to produce
`man/man1/netsuke.1`, and the Windows run additionally to produce the existing
PowerShell module, manifest, locale help, and about-help paths. Inspect the
generated artefacts for the public flags and subcommands already guaranteed by
the v0.8.0 workflow. Do not commit `target/` output.

Run:

```bash
cargo nextest run --test ortho_config_metadata_snapshot_tests
cargo nextest run --test workflow_build_and_package
cargo nextest run --test release_help_script_tests
make check-fmt
make typecheck
make lint
make test
```

If v0.9.0 exposes a genuine missing-subcommand documentation problem, record it
as a follow-up design task. Do not fold parser/config-schema convergence into
this migration.

### Milestone 6: synchronize documentation

Update documentation only after behaviour and generated artefacts are known:

- In `docs/netsuke-design.md`, record v0.9.0 as the configuration runtime,
  explain the absent-versus-failed discovery contract, preserve the manual
  two-pass design, and state why discovery attributes and subcommand docs were
  not transplanted.
- In `docs/adr-004-explicit-config-selection-outside-orthoconfig.md`, preserve
  the accepted selector-ownership decision while replacing the stale direct
  `std::env::var_os` description with the current injected `ConfigEnvProvider`
  port and the v0.9.0 automatic-discovery adapter boundary.
- In `docs/developers-guide.md`, update the `cargo-orthohelp` install command,
  document the `ProcessEnv`/`MapEnv` composition roots, the fixed-key
  projection helper's ownership and permitted callers, and the narrow roles of
  `googletest` and `pretty_assertions`.
- In `docs/users-guide.md`, update only user-visible behaviour: malformed or
  unreadable discovered candidates are errors when no candidate can load, and
  absence still uses defaults. Retain selector precedence and privacy wording.
- In `docs/contents.md`, add the existing explicit-selection ADR to the
  decision-record index if it is still missing. No new guide or ADR is planned,
  and the existing `docs/execplans/` entry already indexes this plan's
  directory.
- Do not rewrite the imported `docs/ortho-config-users-guide.md`; it already
  documents v0.9.0 and serves as the upstream reference.

If no public behaviour changed beyond making the failure distinction explicit,
say that rather than inventing a migration burden. State that Netsukefile YAML
continues to use `serde-saphyr` and is unaffected by OrthoConfig's optional
YAML provider.

Run:

```bash
make fmt
make check-fmt
make typecheck
make lint
make test
make markdownlint
make nixie
```

Every command must pass before completion.

### Milestone 7: final review and atomic delivery

Review `git diff --check`, `git diff --stat`, dependency diffs, new snapshots,
all changed tests, and the surrounding discovery/parser code. Verify no source
file exceeds 400 lines and no in-process environment mutation was introduced.
Use the repository's refactoring heuristics. If a refactor is truly needed,
complete the functional migration first, pass all gates, and perform the
refactor as a separate atomic change with the same gates.

Update every living section of this plan, including exact test names, gate
transcripts, final file count, decisions, and retrospective. Change status to
`COMPLETE` only when no required work remains. Commit only if the user has
authorized it, using the `$commit-message` skill and intended-file staging.

## Concrete steps

From the repository root, establish the baseline:

```bash
git status --short --branch
make check-fmt 2>&1 | tee /tmp/check-fmt-netsuke-ortho-v0-9-0-baseline.out
make typecheck 2>&1 | tee /tmp/typecheck-netsuke-ortho-v0-9-0-baseline.out
make lint 2>&1 | tee /tmp/lint-netsuke-ortho-v0-9-0-baseline.out
make test 2>&1 | tee /tmp/test-netsuke-ortho-v0-9-0-baseline.out
```

Expected summary:

```plaintext
git status: current branch, no unexpected changes
check-fmt: exit 0
typecheck: exit 0
lint: exit 0
test: nextest and doctests exit 0
```

After adding the red tests, run them individually and paste concise failure
evidence into `Artefacts and notes`. The exact filters must be updated after
the test names are chosen. A representative sequence is:

```bash
cargo nextest run --lib 'cli::discovery::*injected*'
cargo nextest run --lib 'cli::discovery::*malformed*'
cargo nextest run --lib 'cli::parser::*localized*'
cargo nextest run --test workflow_build_and_package
```

After the version edit, update only the intended packages and inspect them:

```bash
cargo update -p ortho_config --precise 0.9.0
cargo update -p ortho_config_macros --precise 0.9.0
cargo tree -i ortho_config
cargo tree -i ortho_config_macros
git diff -- Cargo.toml Cargo.lock
```

Expected dependency evidence:

```plaintext
ortho_config v0.9.0
ortho_config_macros v0.9.0
no direct ortho_config_macros requirement in Cargo.toml
```

After each major milestone, use a milestone-specific suffix and run:

```bash
make check-fmt
make typecheck
make lint
make test
```

After documentation changes and at final completion, run:

```bash
make fmt
make check-fmt
make typecheck
make lint
make test
make markdownlint
make nixie
git diff --check
git status --short
```

Do not treat a missing local tool as a passing gate. Install the repository pin
where documented, or record the exact unavailable gate and stop before commit.

## Validation and acceptance

Acceptance is behaviour, not the presence of new types:

- With no selected or discovered Netsuke configuration, parsing and merging
  succeeds with built-in defaults.
- With a valid `.netsuke.toml`, its values load and remain below environment
  and explicit CLI values in precedence.
- With only a malformed or unreadable discovered candidate, Netsuke returns a
  configuration error; it does not silently use defaults.
- With a missing `extends` parent, the diagnostic identifies the resolved
  missing path and referencing file without tests parsing a complete unstable
  human sentence.
- `--config` remains above `NETSUKE_CONFIG`, and either explicit selector
  bypasses automatic discovery.
- An injected environment controls automatic discovery and value merging; a
  deliberately conflicting host home or XDG directory cannot change the test.
- `--color always` and the other customized policy parsers still succeed, while
  invalid values still produce localized failures.
- Release workflow tests require `cargo-orthohelp` v0.9.0, and real generation
  produces every existing staged Unix and Windows help artefact.
- The compact metadata snapshot is reviewed and stable across its relevant
  variants.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass after
  each major milestone; `make markdownlint` and `make nixie` pass at the end.

Record Red-Green-Refactor evidence as follows:

- Red: name the focused command, failing test, and why its failure proves the
  missing v0.9.0 or hermetic behaviour. Do not count dependency compile errors
  as behavioural red evidence when a smaller assertion is possible.
- Green: rerun the identical focused command after the smallest dependency or
  adapter change and record exit 0.
- Refactor: rerun the focused tests plus the four full gates after cleanup.
  No snapshot may remain unreviewed.

The test-tool choice is deliberate:

- `rstest` covers finite source/outcome matrices and reusable temporary
  fixtures.
- `rstest-bdd` covers the user-observable absent-versus-broken configuration
  distinction.
- `assert_cmd` covers the real process boundary, exit status, and streams.
- `googletest` makes typed errors, variants, and nested metadata readable.
- `pretty_assertions` gives useful diffs for ordered layers and projected
  metadata.
- `insta` pins compact multivariant metadata and any intentionally changed
  localized or release-help output.
- Existing Proptest covers selector precedence over generated optional paths.
  No new broad input invariant warrants a second property suite.
- Kani and Verus are not acceptance requirements because this migration adds
  neither a bounded state transition nor contractual business lemma.

## Idempotence and recovery

All test, format, lint, metadata, and help-generation commands are safe to
rerun. `cargo update --precise` is deterministic relative to the manifest and
registry index. Never hand-edit `Cargo.lock`; if an update is interrupted,
rerun the precise commands and inspect the resulting diff.

Test fixtures must live in `TempDir` values and clean up on drop. Child-process
tests use `env_clear` and explicit `Command::env`; they do not need process
environment restoration. The `target/orthohelp/` smoke output is disposable and
ignored. Do not delete broad target or workspace directories to recover from a
failed test.

If a red test fails for the wrong reason, revert only that uncommitted test
edit with a targeted patch, correct the fixture, and rerun it. Do not weaken
the assertion. If a milestone gate fails, leave the working tree intact, record
the exact failure in this plan, and resume from the focused failing command.
Existing user changes are never reset or overwritten.

## Artefacts and notes

Retain concise evidence here rather than pasting full logs into the plan:

```plaintext
Baseline:
- check-fmt: passed at the 2026-08-12 baseline
- typecheck: passed at the 2026-08-12 baseline
- lint: passed at the 2026-08-12 baseline
- test: passed at the 2026-08-12 baseline (1,917 non-doctests and one skip)

Red evidence:
- injected automatic discovery: red test recorded before the `MapEnv` wiring
- malformed discovered candidate: red test recorded before the discovery fix
- localized combined parse: compatibility characterization recorded before
  adopting `parse_localized_command`
- release tool pin: red workflow assertion recorded before the v0.9.0 pin

Green/refactor evidence:
- focused discovery tests: passed after the hermetic adapter change
- focused parser tests: passed after the combined localized parser change
- BDD and E2E tests: passed for absent, valid, malformed, and missing-parent
  discovery outcomes
- metadata and release-help tests: passed, including Unix and Windows smoke
  layouts
- prior final gates: passed after the final rebase; `make test` reported 1,992
  non-doctests and all doctests, before the post-turn remediation

Post-turn Whitaker remediation:
- `build.rs` and `build_l10n_audit::read_source` use `cap_std` directory
  capabilities for their stable parent-directory reads
- stale build-script crate and broad discovery-module exclusions were removed
- `netsuke::cli::discovery::paths` and
  `build_script_build::cli::discovery::paths` remain the only discovery
  exclusions, preserving `std::fs::canonicalize`'s absolute comparison keys
  and cross-directory symlink behaviour; the attempted inner `ambient` scope
  was replaced because Whitaker matches module boundaries
- `normalized_path_key_follows_cross_directory_symlinks` passed as focused
  compatibility evidence
- initial `make lint` failed only on three Clippy `doc_markdown` backtick
  omissions in `discovery_paths` Rustdoc; the omissions were corrected
- final post-remediation gates: `make check-fmt` passed; `make test` passed with
  1,993 nextest tests, one skipped test, and 100 doctests passed with 28
  ignored; `make typecheck` passed; `make lint` passed including docs, Clippy,
  and Whitaker; `make markdownlint` passed over 81 files with 0 errors; and
  `make nixie` plus `git diff --check` passed
- final all-green full-gate result confirmed after the documentation corrections

Review/rebase/publication evidence:
- review correction: `project_scope_file(...) -> Option<PathBuf>` documented in
  place of the stale `project_scope_file_str(...) -> Option<String>` signature;
  other findings were already fixed
- initial rebase: mainline `Cargo.lock` baseline plus
  `cargo update -p ortho_config --precise 0.9.0`; no
  `cargo generate-lockfile`; commit `642bac70`
- subsequent rebase: clean onto `origin/main` `69286cdf`, with no pertinent
  overlap with fixture no-clobber changes; final head `c12ce72a`
- publication: PR #558 published after `check-fmt`, `test`, `typecheck`, and
  `lint` passed
```

Store long command output under `/tmp` with the
`netsuke-ortho-v0-9-0-<milestone>` naming convention. These logs are diagnostic
scratch artefacts, not repository deliverables.

## Interfaces and dependencies

The final implementation should preserve these public interfaces:

```rust
pub trait ConfigEnvProvider {
    fn get(&self, key: &str) -> Option<std::ffi::OsString>;
    fn entries(&self) -> Vec<(std::ffi::OsString, std::ffi::OsString)>;
}

pub fn merge_with_config(
    cli: &Cli,
    matches: &clap::ArgMatches,
) -> ortho_config::OrthoResult<Cli>;

pub fn merge_with_config_and_env(
    cli: &Cli,
    matches: &clap::ArgMatches,
    env: &impl ConfigEnvProvider,
) -> ortho_config::OrthoResult<Cli>;

pub fn parse_with_localizer_from<I, T>(
    iter: I,
    localizer: &std::sync::Arc<dyn ortho_config::Localizer>,
) -> Result<(Cli, clap::ArgMatches), clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone;
```

Private orchestration may add a function whose responsibility is explicit in
its signature, for example:

```rust
fn merge_with_config_sources(
    cli: &Cli,
    matches: &clap::ArgMatches,
    value_env: &impl EnvProvider,
    discovery_env: ortho_config::SharedEnvSource,
) -> ortho_config::OrthoResult<Cli>;
```

The exact private name may follow existing module vocabulary, but it must not
be exported. The ambient wrapper supplies `Arc::new(ProcessEnv)`; the injected
wrapper supplies `Arc::new(MapEnv)` projected from the same fixture. Apply the
same composition rule to early JSON resolution so early diagnostics and the
full merge cannot select different files.

The final dependency intent is:

```toml
[dependencies]
ortho_config = { version = "0.9.0", features = ["serde_json"] }

[build-dependencies]
ortho_config = { version = "0.9.0", features = ["serde_json"] }

[dev-dependencies]
googletest = "0.14.3"
pretty_assertions = "1.4.1"
```

No `ortho_config_macros`, OrthoConfig `yaml`, OrthoConfig `metrics`, or agent
context dependency is required.

## Revision note

Initial draft, 2026-08-12: created the self-contained migration plan from the
v0.9.0 migration guide, current Netsuke configuration and release-help
architecture, repository testing policy, and the `execplans` and
`hexagonal-architecture` skills. The draft deliberately preserves Netsuke's
policy/adapter boundaries, adds hermetic discovery coverage, and defers
optional v0.9.0 product features. Implementation remains pending explicit
approval.

Revised, 2026-08-12: reconciled the draft with the accepted explicit config
selection ADR discovered during validation. The plan now treats that ADR as
controlling, schedules its stale environment-access detail for correction, and
adds its missing contents-index entry without changing the migration's
implementation boundary.

Completed, 2026-08-12: recorded the v0.9.0 implementation, the hermetic
discovery adapter, the release-help subcommand correction, validation evidence,
and the parser-metadata follow-up. The plan is now a completion record as well
as the approved migration guide.

Revised, 2026-08-14: recorded the review correction for the
`project_scope_file` signature, the already-fixed status of the remaining
findings, the conflict-aware narrow lockfile rebase, the clean rebase onto
`origin/main` at `69286cdf`, the final gate results, and publication of PR #558
at `c12ce72a`.

Revised, 2026-08-15: recorded the post-turn Whitaker ambient-I/O findings and
the capability-scoped conversions in `build.rs` and
`build_l10n_audit::read_source`. The broad discovery-module and build-script
crate exclusions were removed, while the focused library and build-script
`discovery::paths` module exclusions remain because the existing
cross-directory symlink test proves that `std::fs::canonicalize` is required
for OrthoConfig-compatible absolute comparison keys. Earlier gate results are
marked as pre-remediation; the final all-green gate result is recorded below.

Revised, 2026-08-15: recorded the post-remediation gate set, including 1,993
nextest tests, one skipped test, 100 doctests passed with 28 ignored,
`markdownlint` over 81 files with 0 errors, and the three Clippy `doc_markdown`
backtick omissions found and corrected before the final lint rerun.

Revised, 2026-08-15: corrected the Whitaker scope record after path-match
evidence showed that `excluded_paths` applies at module boundaries. The
unsupported inner `ambient` scope was replaced with the dedicated library and
build-script `discovery::paths` module exclusions; the broader discovery module
and build-script crate remain covered. The final post-remediation gates then
passed, including docs, Clippy, Whitaker, Markdown, Mermaid, and diff checks.

Revised, 2026-08-15: recorded the final all-green post-remediation gates:
`make check-fmt`, `make test`, `make typecheck`, `make lint`,
`make markdownlint`, `make nixie`, and `git diff --check`, with the exact test
and documentation counts captured in `Progress`, `Outcomes & retrospective`, and
`Artefacts and notes`.

Revised, 2026-08-16: recorded the review correction to use direct
`Dir::rename` for man-page replacement, preserving the current page if the
replacement fails, and corrected the `dylint.toml` comment to describe only the
module-scoped `build_script_build::cli::discovery::paths` exception.
