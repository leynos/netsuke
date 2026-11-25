# Resolve shell selections into a complete execution value (11.2.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

The plan awaits explicit approval. No production code, test, proof, or
configuration changes until the user approves it.

Branch: `11-2-3-execution-value-resolution`

Roadmap item: 11.2.3 in `docs/roadmap.md`. It requires 11.2.1 and 11.2.2, which
are both merged.

## Purpose / big picture

Netsuke compiles a YAML manifest (a "Netsukefile") into a Ninja build graph. A
future structured command will be able to say which shell interprets it:

```yaml
command:
  - invoke: cargo test --workspace
    shell: false
  - invoke: make report && publish
    shell: true
  - invoke: set -o pipefail; build-project
    shell: bash
```

Two pieces already exist. Roadmap 11.2.1 added the vocabulary: `ShellSelection`
(`Direct`, `PlatformDefault`, or `Named`), `ShellName`, `BuiltInShell`, and
`HostFamily` in `src/shell_selection/`. Roadmap 11.2.2 let an operator, meaning
the person who controls system or user configuration, declare extra shells in a
`[[shells]]` table. Nothing yet turns any of these into a program to run.

This task supplies that step. Given a selection, the operator's definitions,
and the host's own `PATH`, it returns exactly one of two results:

- a complete execution value: the shell's name, one resolved absolute
  executable, and its fixed arguments; or
- a typed failure that says which of five things went wrong: the name is
  unknown, the built-in shell is unsupported on this host, the executable is
  unavailable, the definition is misconfigured, or the host's own `PATH` or
  `PATHEXT` is malformed.

The security property is the reason for the task. A Netsukefile may choose a
name from an allow-list and nothing more. Repository content must not be able
to replace `bash` by committing a file called `bash`, and a command's own
environment settings must not be able to redirect the search.

No structured-command AST exists yet, so no manifest can reach this code. The
result is observable through tests, not through the command line: each failure
class, both host families' rules on either build host, the exact sequence of
files inspected, and the rendered diagnostics. Task 11.3.1 connects the value
to manifests after 12.3.1 supplies the AST. Existing command execution, legacy
recipes, the `which` template helper, and configuration loading keep their
current behaviour.

## Open decisions for the approver

The plan proceeds on the stated default for each item unless the approver
chooses otherwise. The first two question accepted ADR text, so they need the
maintainer rather than the implementor.

- **OD-1. Empty `PATH` components on Windows.** ADR-0019 rejects every empty
  component. A trailing or doubled `;` produces one, and such values are
  ordinary on Windows: Go hit the same rejection as a field defect and now
  skips empty Windows entries "consistent with what PowerShell
  does".[^go-empty] Under the accepted text, one stray `;` makes `bash`, `pwsh`,
  `powershell`, and therefore `shell: true`, fail on that host. Default:
  implement ADR-0019 exactly and make the diagnostic name the entry and the
  remedy. Alternative: amend ADR-0019 so that an empty component is dropped on
  Windows only, where it has no current-directory meaning in the resolver.
  Decide before 11.3.1 ships, because that is when users first meet it.
- **OD-2. How `shell: true` finds Windows PowerShell.** ADR-0019 Table 1
  resolves `powershell.exe` through trusted `PATH`. ADR-0071, accepted later
  for legacy recipes, binds it from the Known Folder API and never reads
  `PATH`. The two policies can select different files on one host. Default:
  follow ADR-0019. Alternative: amend Table 1 so the `powershell` built-in uses
  `src/power_shell_location/`. That would also remove OD-1 from the
  platform-default route.
- **OD-3. Localization now or in 11.3.1.** The roadmap asks this task to convert
  errors to user diagnostics once, and ADR-0019 requires that conversion to be
  localized. Every Fluent key must exist in all 35 catalogues. Default: deliver
  the localized conversion here with nine keys. Alternative: deliver the typed
  error only and move conversion to 11.3.1, which is where the action, target,
  and pipeline-stage context becomes available.
- **OD-4. Continuous integration (CI) for Verus kernels.**
  `.github/workflows/proofs.yml` runs a bare `make verus`, which proves only
  `verus/canonical_value/main.rs`. The kernel added in 11.2.2 is not proved in
  CI and neither would this one be. Default: make that workflow name all three
  kernels. Alternative: leave CI alone and rely on the milestone command.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- Preserve ADR-0019 with all of its addenda, and RFC 0011 sections 5 to 8. In
  particular: built-ins before configured entries; `true` maps through
  `BuiltInShell::host_default` and is never affected by configuration; `sh` is
  the fixed path `/bin/sh`; fixed arguments are passed unchanged.
- Reject malformed `PATH` and `PATHEXT` before any filesystem inspection. Never
  make a rejected component absolute, skip it, or fall back from it.
- Never read the process working directory, walk the workspace, or search a
  directory that is not an admitted `PATH` component.
- The resolver's interface must have no parameter through which a command's
  environment overlay, a manifest value, or a rendered template could supply
  `PATH`, `PATHEXT`, a directory, an executable, or an argument.
- Do not modify `src/ast/`, `src/ir/`, `src/ninja_gen/`, `src/runner/`,
  `src/manifest/`, `src/recipe_shell/`, or `src/power_shell_location/`. Do not
  wire the resolver into a manifest, a command, or the action plan.
- The resolver is crate-private. `src/lib.rs` gains `mod shell_resolution;`,
  never `pub mod`. The registry must not serve MiniJinja `shell()` or `grep()`,
  legacy `RecipeShell`, shebang handling, or general executable discovery
  (ADR-0019, "Configured shell registry").
- Keep the observable behaviour of the `which` filter, the `which()` function,
  and `command_available` byte-identical, including their errors, telemetry,
  and cache keys. The lenient parsers in `src/stdlib/which/env/path_support.rs`
  are that feature's contract and stay as they are.
- `src/shell_selection/` must not depend on `src/cli/`, on the filesystem, or on
  the environment. `src/shell_selection/name/mod.rs` keeps importing only `std`,
  `serde`, and `thiserror`.
- Read the environment only through `mockable::Env` (ADR-0008). No test may
  mutate the harness process's environment or working directory.
- `std::fs` is permitted only inside `netsuke::stdlib::which::lookup`, the
  bounded exemption in `dylint.toml` that `tests/whitaker_boundary_contract.rs`
  pins. Do not add an exemption.
- Add no dependency, production or development. `googletest`,
  `pretty_assertions`, `rstest`, `rstest-bdd`, `proptest`, `insta`, and
  `mockable` are already present.
- Follow `AGENTS.md`: no file above 400 lines, every item documented, en-GB
  Oxford spelling, item-scoped `#[expect]` only, no `expect()` outside tests,
  and an explanatory message on every assertion.
- The pinned nightly supplies Polonius and the next-generation trait solver. Add
  no compiler flag and no defensive clone. Posture: `polonius-alpha`.
- Completed ExecPlans are historical. Do not edit the 11.2.1 or 11.2.2 plans.
- Verus kernels contain no `assume`, `admit`, external body, or added axiom.
- Never hold a compile slot or cache hostage: run gates sequentially, through
  `scrutineer`, and never bypass compile admission.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main` when implementation starts.
Limits are net lines, additions minus deletions, and each category is enforced
on its own. ExecPlan files count towards nothing. Count with:

```bash
git diff --numstat "$(git merge-base origin/main HEAD)"..HEAD
```

Classify whole files by path:

- Code: `*.rs`, `*.feature`, `*.snap`, and `verus/**`.
- Localization: `locales/**`.
- Configuration: `*.toml`, `*.yml`, `Makefile`, and `proptest-regressions/**`.
- Documentation: `docs/**` and `CHANGELOG.md`, excluding `docs/execplans/**`.

Budgets, justified by the survey in `Context and orientation`:

- Files: 110 non-ExecPlan files, of which 35 are locale catalogues.
- Code: 4,600 net lines. The estimate is 3,850 (production 1,150; tests and
  snapshots 2,250; Verus 320; extraction refactors 130), plus 20 per cent.
- Localization: 1,300 net lines. The estimate is nine keys, or about 30 lines,
  in each of 35 catalogues, plus 24 per cent.
- Configuration: 70 net lines.
- Documentation: 500 net lines. Task 11.2.2 landed 401.
- Interface: no new `pub` item except the two lexical `HostFamily` methods named
  in D4. Any other public API change stops work.
- Dependencies: any addition or version change stops work.
- Architecture: any need to widen a filesystem exemption, read another
  environment variable, or change ADR-0019's contract beyond the addendum in
  EP-M5 stops work.
- Iterations: three failed attempts at one test, proof, or lint finding stop
  work.
- Proof: if a lemma in `Invariants and lemmas` is false, or needs an axiom, stop
  and revise the design. If Verus exceeds its default resource limit after two
  decompositions, stop and report.
- Ambiguity: if ADR-0019 or RFC 0011 admits two readings that change an outcome
  and this plan does not settle it, stop and present both.

Compiler-slot queueing is not a trigger.

## Risks

- Risk: stray empty `PATH` entries make named shells unusable on ordinary
  Windows hosts (OD-1). Severity: high. Likelihood: medium. Mitigation: an
  actionable diagnostic now, a recorded decision before 11.3.1, and one
  admission function so that a change of policy touches one branch and its
  tests.
- Risk: two accepted ADRs disagree about locating Windows PowerShell (OD-2).
  Severity: medium. Likelihood: high. Mitigation: surfaced for decision; the
  resolver isolates the built-in executable table so either answer is local.
- Risk: crate-private code with no production caller trips `dead_code` under
  `-D warnings`, and module-wide suppression is forbidden. Severity: medium.
  Likelihood: high. Mitigation: D3 and the EP-M0 probe.
- Risk: the resolved path is not the program that later runs. Rust's Windows
  `Command` appends `.exe` to an absolute path without that suffix and prefers
  the suffixed file when it exists,[^rust-command] so a resolved
  `C:\tools\bash.com` can launch `C:\tools\bash.com.exe`. Batch files are run by
  `cmd.exe`, whose quoting differs.[^batbadbut] Severity: high. Likelihood:
  low. Mitigation: this task launches nothing; the hazards go into the ADR
  addendum and the 11.3.1 hand-off as launch-time obligations.
- Risk: the file is replaced between resolution and launch. Severity: medium.
  Likelihood: low. Mitigation: documented residual, as in ADR-0071.
- Risk: an absolute `PATH` component lies inside the repository, for example one
  added by `direnv`. Severity: medium. Likelihood: medium. Mitigation: ADR-0019
  trusts the operator's `PATH`; the users' guide states the residual plainly.
- Risk: Windows semantics pass on Linux and fail on Windows. Severity: medium.
  Likelihood: medium. Mitigation: all rules are text functions of an explicit
  `HostFamily`; the real-filesystem adapter has Windows-gated tests; snapshots
  contain no host-supplied text.
- Risk: `scenarios!("tests/features")` in `tests/bdd_tests.rs` discovers every
  feature file in that directory and fails on a step it does not define.
  Severity: low. Likelihood: high if forgotten. Mitigation: the feature file
  lives in `tests/features_internal/` (D12).
- Risk: 35 translations of varying quality. Severity: low. Likelihood: medium.
  Mitigation: nine keys, the glossary and style guide, right-to-left rules, and
  the build-time catalogue audit.
- Risk: duplicated effort with 11.2.4, which owns property tests for shell
  selection. Severity: low. Likelihood: medium. Mitigation: the boundary in D13.

## Progress

- [x] (2026-11-25) Renamed the branch, set the session title, loaded the
  required skills, and confirmed that 11.2.1 and 11.2.2 are merged.
- [x] (2026-11-25) Three Wyvern reconnaissance reports: `which` reuse,
  consumers and error boundaries, and gates and proof registration.
- [x] (2026-11-25) External research: Go and Rust executable lookup,
  `CreateProcessW`, and batch-file quoting.
- [x] (2026-11-25) Drafted the contract, proof sketch, milestones, and
  verification plan.
- [ ] Community-of-experts design review applied.
- [ ] Plan approved by the user, with OD-1 to OD-4 answered or defaulted.
- [ ] `EP-M0` Feasibility probes recorded and discarded.
- [ ] `EP-M1` Shared lexical and inspection seams extracted.
- [ ] `EP-M2` Trusted search path admitted, with its Verus admission proof.
- [ ] `EP-M3` Registry and resolver deliver the execution value, with the Verus
  search proof and behavioural scenarios.
- [ ] `EP-M4` Typed failures become localized diagnostics once.
- [ ] `EP-M5` Documentation, ADR addendum, CI kernel selection, and roadmap
  closure.

## Surprises & discoveries

- Observation: `apply_config` in `src/cli/merge/apply.rs` copies selected fields
  from `CliConfig` into `Cli` and does not copy `shells`. Evidence: read at
  `a97bd9491`; the only consumer of `CliConfig.shells` is
  `validate_configured_shells` in `src/cli/config/validation.rs`. Impact: the
  runner cannot see the definitions today. This task builds the registry from
  `&CliConfig`; 11.3.1 must build it before the configuration is reduced to
  `Cli`, not after.
- Observation: `cap_std` cannot serve as the inspection adapter. Evidence:
  `dylint.toml` records that `cap_std` refuses symbolic links that leave the
  opened directory, which is how `/bin/sh` and `/usr/bin/pwsh` are commonly
  installed. Impact: reuse the sanctioned `which::lookup` boundary (D6).
- Observation: `CwdMode::Never` does not disable relative `PATH` entries.
  Evidence: `parse_path_entries` joins them to the working directory before the
  mode is consulted. Impact: calling `lookup` with fallbacks off would violate
  ADR-0019, so only its file inspection is reusable.
- Observation: CI proves one of the two committed Verus kernels. Evidence:
  `VERUS_FLAGS` in the `Makefile` and the command in
  `.github/workflows/proofs.yml`. Impact: OD-4 and D14.
- Observation: no Fluent keys were added by 11.2.1 or 11.2.2; their messages are
  English `thiserror` text. Impact: this task introduces the first localized
  shell-selection messages.

## Decision log

Date and author for every entry unless stated: 2026-11-25, planning agent.

- D1. Decision: put the feature in a new crate-private sibling module,
  `src/shell_resolution/`. Rationale: 11.2.1's D1 and the developers' guide
  reserve this place. Placing it under `src/cli/` would pull it into the build
  script's module slice; placing it in `src/shell_selection/` would give the
  data-only vocabulary an environment dependency.
- D2. Decision: split the module into a pure domain and four adapters. The
  domain is `registry`, `search_path`, `resolver`, `resolved`, `probe` (the
  port), and `error`. The adapters are `config` (reads `CliConfig`),
  `environment` (reads `mockable::Env`), `filesystem` (implements the port), and
  `diagnostic` (localizes). Only `config` may import `crate::cli`, only
  `environment` may import `mockable`, only `filesystem` may import
  `crate::stdlib`, and only `diagnostic` may import `miette` or
  `crate::localization`. Rationale: the dependency rule of hexagonal
  architecture. Roadmap text asks for a registry "from `CliConfig`" while the
  developers' guide says the resolver accepts definitions; an adapter that
  translates one into the other satisfies both.
- D3. Decision: keep every item `pub(crate)` or narrower and attach
  `#[cfg_attr(not(test), expect(dead_code, reason = "..."))]`, naming roadmap
  11.3.1, to the few entry points that nothing calls yet. Rationale:
  `AGENTS.md` permits a temporary item-scoped expectation linked to tracked
  work, and the form has a precedent on `InterpreterBinding::InspectionOnly`.
  Rustc treats an item under the expectation as live for its callees, so only
  the roots need it. When 11.3.1 calls a root, the expectation becomes
  unfulfilled and the build fails until it is removed. Making the module public
  would also leave the expectation unfulfilled, so privacy is enforced by the
  same mechanism. Rejected: a public module, which contradicts
  "feature-private"; compiling the module only under `cfg(test)`, which would
  leave production untested; and a premature consumer.
- D4. Decision: extract the host-parameterized absoluteness predicate from
  `src/cli/config/shells/spelling.rs` into
  `src/shell_selection/host_path/mod.rs` as
  `HostFamily::is_absolute_path(self, &str) -> bool`, and add
  `HostFamily::join_path(self, &str, &str) -> String`. Rationale: `PATH`
  admission needs the rule configuration already applies, and two copies of a
  security predicate would drift. `std::path` cannot be used because it applies
  the build host's rules, and tests must exercise both families on either host.
  The vocabulary module is the only place that both `src/cli/` and the resolver
  may depend on. Re-use policy: shell selection and shell configuration only;
  legacy `RecipeShell` and `which` must not adopt it without a separate
  decision.
- D5. Decision: split `PATH` with `OsStr::as_encoded_bytes` on the family's
  separator byte and decode each component with `str::from_utf8`. Rationale:
  the encoding is self-synchronizing over ASCII, so splitting on `:` or `;` is
  safe for non-Unicode values on both hosts, and decoding per component yields
  the exact index of the first bad one. `std::env::split_paths` applies only
  the build host's rules and silently removes quotes on Windows.
- D6. Decision: reuse only the file inspection inside
  `src/stdlib/which/lookup/`. Extract
  `inspect_executable(&Utf8Path) -> io::Result<ExecutableInspection>` there and
  express `is_executable` through it. Rationale: the reuse survey found every
  other function unsuitable, as follows.
  - `lookup`, `resolve_direct`, `handle_miss`, and `search_workspace` consult
    the working directory or workspace.
  - `parse_path_entries` converts relative components.
  - `parse_pathext` skips malformed entries and falls back to the default.
  - `EnvSnapshot` and `WhichResolver` read an extra variable, emit telemetry,
    and cache.
  - `candidate_paths` is compiled only on Windows.
  - `InterpreterExecutable` in `src/recipe_shell/` applies the build host's
    absoluteness rule and belongs to a different policy.
- D7. Decision: inspection follows symbolic links, and the result is the path
  that was inspected, never its canonical form. Rationale: `/bin/sh` is usually
  a link, and several shells change behaviour with the name they are invoked
  by, so canonicalizing `sh` to `dash` or `bash` would change semantics.
- D8. Decision: the probe reports `Executable`, `NotExecutable`, or `Missing`;
  `NotFound` and `NotADirectory` mean `Missing`. A bare-name search skips
  `Missing` and `NotExecutable`. Any other I/O error stops the search with an
  unavailable outcome that retains the error. Rationale: continuing past a
  directory that cannot be inspected would let a later directory win because an
  earlier one failed transiently, which is a guess. ADR-0019 makes "cannot be
  resolved safely" an unavailable outcome, and `AGENTS.md` forbids converting
  permission errors into absence. The existing `which` search stops in the same
  way.
- D9. Decision: capture raw `PATH`, and `PATHEXT` on Windows only, once when the
  resolver is created; parse them only when a bare name is resolved. Rationale:
  ADR-0008 prefers data captured at the boundary; one snapshot makes every
  command in a run agree; and a malformed `PATH` must not block `/bin/sh` or an
  absolute configured shell, which do not search. An unset `PATH` is an
  unavailable outcome, since an absent variable has no malformed component, and
  no default search path is ever assumed.
- D10. Decision: on Windows, validate `PATH` first and `PATHEXT` second for
  every bare name, including one that already has an extension. Rationale:
  ADR-0019 Table 1 resolves `powershell.exe` through "trusted host `PATH` and
  `PATHEXT`", and the contract rejects a malformed value before inspection
  without excepting extended names. An absolute executable uses neither, and
  receives no suffix.
- D11. Decision: reject a Windows component containing `"`, and any component
  containing NUL. Rationale: the Rust library and `cmd.exe` treat quotes as a
  way to embed `;`, while other consumers treat them as literal characters. The
  meaning is ambiguous, and ADR-0019 requires ambiguous cases to fail.
- D12. Decision: run `rstest-bdd` scenarios inside the library's unit-test
  target, with the feature file at
  `tests/features_internal/shell_resolution.feature`. Rationale: the subject is
  crate-private, so an integration test cannot reach it, and the existing
  directory is auto-discovered by another binary. EP-M0 proves this works; if
  it does not, the fallback is named `rstest` cases that quote the scenario
  text.
- D13. Decision: this task owns properties of one function against its own
  contract: the `PATHEXT` grammar, `PATH` admission, and search order. Task
  11.2.4 owns generated registries and YAML values, and properties that span
  selection, registry, environment, and result. Rationale: the Verus lemmas
  below need a tested correspondence to production code in the same change.
- D14. Decision: prove admission and search in Verus, test the `PATHEXT` grammar
  and de-duplication by property, and test classification by exhaustive table.
  Rationale: see `Formal verification decision`.
- D15. Decision: `ResolvedShell` stores its name as `String`, not `ShellName`,
  and has private fields, read-only accessors, no `Serialize`, and one
  constructor visible only to the resolver. Rationale: ADR-0019 keeps the
  manifest-facing name type outside the execution IR. A versioned persisted
  form belongs to 11.3.1 and 12.3.1.
- D16. Decision: emit no tracing event here. Rationale: RFC 0011 section 7 makes
  it optional, and `AGENTS.md` requires new telemetry to be exercised through
  the application's collection path, which does not exist until 11.3.1.
- D17. Decision: record settled details as an addendum to ADR-0019 instead of a
  new ADR. Rationale: they refine an accepted decision and reverse none of it.

## Outcomes & retrospective

Not started. At completion, compare the result with the purpose above, record
the actual budget use, and write these hand-offs.

- To 11.2.4: the generators, oracles, and recording probe that can be reused.
- To 11.3.1: construct the registry from `CliConfig` before `apply_config`;
  remove the D3 expectations; attach action, target or rule, item, and stage
  context around the diagnostic; add tracing; and, at launch, use the resolved
  path exactly, guard against the `.exe` substitution, and decide the policy
  for batch and script extensions.
- To 12.3.1: version the persisted form and classify an invalid persisted shell
  as misconfigured.

## Context and orientation

Inspected revision: `a97bd9491ac1b418e8940f5038dd706b5b9114bb`. Line numbers
below are for that revision.

`src/shell_selection/` is public, data-only vocabulary. `BuiltInShell` lists
the four built-ins with `lookup`, `is_supported_on`, `executable`,
`fixed_args`, and `host_default`. `BuiltInExecutable` is `AbsolutePath` or
`BareName`. `HostFamily` is `UnixLike` or `Windows`, and every host-dependent
query takes it as an argument, so both families' rules run on either build host.

`src/cli/config/shells/` owns the operator schema. `ShellDefinition` holds a
`ShellName`, a `ShellExecutable`, and `ShellArguments`.
`ShellExecutable::parse` takes a `HostFamily`; its `Deserialize` uses
`HostFamily::current()`. `validate_composition` rejects built-in and repeated
names. `preview::preview` bounds and escapes text for a diagnostic.
`CliConfig.shells` is at `src/cli/config/mod.rs:212`. The definitions are
already admitted only from system and user operator files.

`src/stdlib/which/` is an in-tree module; there is no `which` crate in the
dependency graph. Throughout ADR-0019 and the roadmap, "invoking `which`" means
this module. `lookup::is_executable` (`src/stdlib/which/lookup/mod.rs:219`)
follows links, requires a regular file, and on Unix requires an execute bit.

`src/power_shell_location/mod.rs` is the precedent for a narrow port with a
typed `thiserror` enumeration. `src/stdlib/which/resolve_error.rs` is the
precedent for a crate-internal typed error converted once (ADR-0005).
`src/runner/error.rs` is the precedent for a localized `miette` diagnostic with
a stable code.

`src/localization/shell_keys.rs` is the precedent for a split key table.
`build_l10n_audit/` fails the build when a declared key is missing from any of
the 35 catalogues in `locales/*/messages.ftl`, when a catalogue carries an
undeclared key, or when variables differ from `en-US`. A message may use a
Fluent selector; `ninja_gen.progress.command_list` does.

`src/cli/` and `src/shell_selection/` are also compiled into the build script.
Adding a file there means updating `build.rs`, `build_slice_fixture/slice.rs`,
and `BUILD_SLICE_RERUN_PATHS` in `tests/build_module_slice_ui_tests.rs`.
`tools/kani/proof-scope.toml` must list every file that a Kani harness can
reach; `make test-workflow-contracts` prints any difference. Files containing
child modules must be named `mod.rs`.

Relevant documentation: `docs/netsuke-design.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/ortho-config-users-guide.md`, `docs/rstest-bdd-users-guide.md`,
`docs/formal-verification-methods-in-netsuke.md`,
`docs/snapshot-testing-in-netsuke-using-insta.md`,
`docs/localization-styleguide.md`, `docs/localization-glossary.md`,
`docs/translators-guide.md`, and `docs/documentation-style-guide.md`.

Relevant skills: `execplans`, `codegraph-mcp`, `hexagonal-architecture`,
`rust-router` (then `rust-errors`, `rust-types-and-apis`, `rust-unit-testing`,
`rust-unused-code`), `rust-verification`, `proptest`, `verus`, `nextest`,
`addressing-whitaker-findings`, `localization-docs`, `en-gb-oxendict`,
`sdlc-implementation`, and `firecrawl-mcp`.

### Budget survey

Task 11.2.1 budgeted 2,600 net lines and 26 files and landed 1,468 code, 51
configuration, and 133 documentation lines in 23 files; one revision (its D11)
moved an unrelated timeout repair to another pull request to stay inside the
file limit. Task 11.2.2 set no numeric budget and landed 4,923 code, 34
configuration, and 401 documentation lines in 72 files. Its excess over a naive
estimate came from three review rounds that asked for evidence through the
production entry point, plus Windows-only test repairs. Task 6.1.2, the other
Verus-backed feature, landed 3,767 code lines in 37 files. This task has no
discovery work but adds a proof, a filesystem adapter, and localized
diagnostics, so its estimate sits between the last two. The contingency covers
the observed pattern of late evidence requests.

## Conformance basis

Revisions inspected at `a97bd9491`:

- Terms of reference, draft v0.2, last revised 2026-09-25.
- `docs/netsuke-design.md`, "Structured-command shell selection vocabulary".
- ADR-0019 (accepted 2026-09-02, with addenda to 2026-11-06), RFC 0011
  (proposed), and RFC 0001 sections 10 and 23.
- ADR-0005, ADR-0008, ADR-0014, ADR-0017, ADR-0043, and ADR-0071.
- Roadmap 11.2.3; issue #593 is the umbrella and stays open.

Requirements:

- `REQ-REGISTRY`: a feature-private registry built from `CliConfig`.
- `REQ-ORDER`: bare names search trusted `PATH`, and on Windows `PATHEXT`, in
  order, and select the first regular executable.
- `REQ-ADMIT`: malformed `PATH` or `PATHEXT` is rejected before inspection, with
  the variable and a bounded index only.
- `REQ-CONFINE`: no working-directory, workspace, overlay, or repository-local
  replacement.
- `REQ-PATHEXT`: the exact grammar, default, normalization, and already-extended
  rule of ADR-0019.
- `REQ-CLASS`: unknown, unsupported, unavailable, misconfigured, and
  trusted-environment misconfiguration stay distinct.
- `REQ-VALUE`: the value carries only name, executable, and fixed arguments.
- `REQ-DIAG`: one conversion to bounded, localized user diagnostics.

Gaps: `GAP-RESOLVER`, no code maps a selection to an executable; `GAP-STRICT`,
existing parsers are lenient; `GAP-PROOF-CI`, CI omits a kernel.

Trace:

```plaintext
ADR-0019 registry     -> REQ-REGISTRY -> D2,D3      -> EP-M3 -> tests::registry
ADR-0019 resolution   -> REQ-ORDER    -> INV-FIRST  -> EP-M3 -> tests::search, verus search
ADR-0019 resolution   -> REQ-ADMIT    -> INV-CLOSED -> EP-M2 -> tests::admission, verus admission
RFC-0011 §5.3         -> REQ-CONFINE  -> INV-CONFINE-> EP-M3 -> tests::confinement, verus search
ADR-0019 PATHEXT      -> REQ-PATHEXT  -> INV-EXT    -> EP-M2 -> tests::pathext
ADR-0019 diagnostics  -> REQ-CLASS    -> INV-CLASS  -> EP-M3 -> tests::classification, BDD
ADR-0019 lowering     -> REQ-VALUE    -> INV-VALUE  -> EP-M3 -> tests::resolved
ADR-0005, RFC-0011 §7 -> REQ-DIAG     -> INV-BOUND  -> EP-M4 -> tests::diagnostic, snapshots
```

## Verification plan

### Abstract model

A selection is `Direct`, `PlatformDefault`, or `Named(n)`. A host family `h`
fixes a separator byte, an absoluteness predicate, and whether extensions
apply. A snapshot is `(path, pathext)`, each absent or a byte string.

Admission maps a component sequence `C` to an ordered directory sequence, or to
the least index of an inadmissible component. Columns are a non-empty sequence
of suffixes: the single empty suffix on Unix-like hosts and for an extended
name, otherwise the admitted `PATHEXT` sequence. The candidate grid is the
product of admitted directories and columns, ordered by directory and then by
column. A probe maps each grid cell to `Hit`, `Miss`, or `Fault`. Search returns
`Found(cell)`, `Faulted(cell)`, or `Exhausted`, together with its trace: the
exact sequence of cells inspected.

Preconditions: none on `C`, the entries, or the probe. Every input has a
defined outcome, and admission failure is an outcome rather than a violated
precondition.

### Invariants and lemmas

Admission and search, selected for proof:

- `L-ADMIT-OK`: admission succeeds exactly when every component is admissible,
  and then the result equals `C`: nothing dropped, added, or reordered.
- `L-ADMIT-LEAST`: admission fails with `k` exactly when `C[k]` is inadmissible
  and every earlier component is admissible.
- `INV-FIRST` (`L-SEARCH-FIRST`): search returns `Found(c)` exactly when the
  probe hits at `c` and misses at every earlier cell; `Faulted(c)` likewise for
  a fault; `Exhausted` exactly when every cell misses.
- `L-SEARCH-TRACE`: the trace is exactly the cells up to and including the
  decisive one, in order, or the whole grid when exhausted.
- `L-SEARCH-UNIQUE`: the least decisive cell is unique, so the result does not
  depend on how the traversal is written.
- `INV-CLOSED` (`L-CLOSED`): if either admission fails, the trace is empty and
  the outcome names the failing variable and index, with `PATH` before
  `PATHEXT`.
- `INV-CONFINE` (`L-CONFINE`): every traced cell's directory is an element of
  `C` that satisfies the absoluteness predicate. A candidate below a relative
  component is therefore never inspected.

Established by test:

- `INV-EXT`: an admitted `PATHEXT` is non-empty, lower case, dotted, matches
  `\.[a-z0-9]+`, has no duplicates, and keeps first-occurrence order; every
  violation of the raw grammar is rejected at its least index.
- `INV-CLASS`: classification is total and its precedence is unknown, then
  unsupported, then misconfigured executable, then trusted environment, then
  unavailable. An earlier outcome prevents every later step, including reading
  the snapshot.
- `INV-BUILTIN`: a built-in name never reaches a configured entry, and
  `PlatformDefault` gives the same result for every configured registry.
- `INV-VALUE`: a resolved executable is absolute on the host family, and the
  fixed arguments equal the table's or the definition's, element for element.
- `INV-DIRECT`: `Direct` performs no registry lookup, parse, or inspection.
- `INV-BOUND`: a diagnostic contains no `PATH` or `PATHEXT` text, quotes at most
  64 characters of any operator-supplied value, and renders an index as decimal
  digits below the component count.

### Axioms (trusted, not verified here)

- `AXIOM-ENV`: `mockable::DefaultEnv::os_string` returns the process value
  unchanged; on Windows, lookup is case-insensitive. Boundary: `environment`.
- `AXIOM-OSSTR`: `OsStr::as_encoded_bytes` is a superset of UTF-8 that is safe
  to split at ASCII bytes, as the standard library documents. Boundary:
  `search_path`.
- `AXIOM-META`: `std::fs::metadata` follows links and reports type and mode.
  Boundary: `which::lookup`. Exercised against the real filesystem.
- `AXIOM-INDEXSET`: `indexmap::IndexSet` keeps first-insertion order. Boundary:
  `search_path::pathext`. Exercised by `INV-EXT`.
- `AXIOM-MERGE`: `CliConfig.shells` contains only operator-authorized entries,
  as 11.2.2 established.
- `AXIOM-OPERATOR`: the operator's `PATH` directories and the configured fixed
  arguments are trusted policy.

### Methods per obligation

Obligation: `L-ADMIT-OK`, `L-ADMIT-LEAST`, `L-SEARCH-FIRST`, `L-SEARCH-TRACE`,
`L-SEARCH-UNIQUE`, `L-CLOSED`, and `L-CONFINE`.

- Method: Verus proof over unbounded sequence lengths, plus property tests that
  tie the model to production.
- Rationale: these are the security contract, they hold for every `PATH` length,
  and example tests cannot establish "never inspected".
- Artefacts: `verus/shell_resolution/main.rs`;
  `src/shell_resolution/tests/search_properties.rs` and
  `src/shell_resolution/tests/admission_properties.rs`.
- Evidence:
  `make verus VERUS_FLAGS='--proof-file verus/shell_resolution/main.rs'`
  reports zero errors. The property tests compare production against
  independent oracles: admission against a scan that evaluates every component
  and takes the minimum failing index, and search against a fully enumerated
  product with the first decisive element selected by index. The recording
  probe's calls must equal the oracle's prefix.
- Non-vacuity: a proof function exhibits accepted input, a hit at cell (1, 1) of
  a two-by-two grid after three misses, a fault before a hit, exhaustion, an
  empty directory sequence, and a failure at index 0 and at the last index.
  Property tests classify cases and fail if any class is absent from 1,000
  deterministic samples.
- Negative controls: exchange the loops so that extension dominates directory;
  make admission skip an invalid component; make a fault count as a miss;
  inspect one cell before admission completes. Each must fail the model proof
  or the property, for the stated lemma, and then be reverted.

Obligation: `INV-EXT`.

- Method: an `rstest` table for each rule; a property test against an oracle
  built on the `regex` development dependency; an exhaustive check of every
  ASCII character as the sole character of an entry.
- Domain: absent, empty, non-Unicode, NUL, blank, leading, trailing, and doubled
  separators, tabs, non-ASCII letters and spaces, interior dots, case variants,
  duplicates with and without dots, and the eleven-entry default.
- Artefact: `src/shell_resolution/search_path/pathext/tests.rs`.
- Non-vacuity: generated accept and reject classes are counted; mutations
  "trim Unicode whitespace", "keep last duplicate", "default on empty", and
  "skip blank" must each fail a named case.

Obligation: `INV-CLASS`, `INV-BUILTIN`, and `INV-DIRECT`.

- Method: an exhaustive `rstest` table over selection kind, host family, and the
  finite registry, environment, and probe states. Behavioural scenarios restate
  the main outcomes.
- Non-vacuity: the probe and a strict `MockEnv` record calls, so a wrong
  precedence shows up as an unexpected call as well as a wrong variant.
- Artefact: `src/shell_resolution/tests/classification.rs`.

Obligation: `INV-VALUE` and `INV-CONFINE` at the real boundary.

- Method: real-filesystem tests through the production adapter with a temporary
  directory and `MockEnv`: a regular executable, a directory of the same name,
  a file without an execute bit, a symbolic link, a dangling link, a file used
  as a `PATH` directory, and `/bin/sh` on Unix; `.exe` selection on Windows.
- Artefact: `src/shell_resolution/filesystem/tests.rs`.

Obligation: `INV-BOUND`.

- Method: `insta` snapshots of each class in the human and JSON renderings,
  paired with semantic assertions that sentinel directory names never appear
  and that a 10,000-character name or path is truncated.
- Artefact: `src/shell_resolution/diagnostic/tests.rs` and its snapshots, which
  contain no host-supplied path, error text, or separator.

### Formal verification decision

Verus is warranted for admission and search because the claims quantify over
every sequence length, the state is small and stable, and a defect has security
consequences. The model is stated as a specification first, over abstract
components and an arbitrary probe, so it cannot restate the implementation.

Verus is not warranted for the `PATHEXT` grammar, a finite character-class
predicate best checked against an independent regular expression; for
de-duplication, which production delegates to `IndexSet` and whose
least-occurrence contract `verus/canonical_value/main.rs` already proves; for
classification, a finite table; or for diagnostics, which are presentation.

Kani is not selected. Its value is exhaustive exploration of small bounded
domains, and the byte-level predicates here are covered exhaustively by cheaper
tests, while the sequence properties are unbounded. Adding harnesses would also
enlarge the Kani proof scope and its CI cost.

Residual gaps, recorded in the design document: the proof covers a model, not
the shipped traversal; the correspondence is sampled; the filesystem can change
after resolution; and launch behaviour is outside this task.

### Proof sketch

The statements below are hypotheses about the proof's eventual shape.
Implementation may refine, split, or drop them, and records why in the
`Decision log`. Triggers, auxiliary inductions, and sequence algebra are left
until the verification conditions exist.

```rust,ignore
// Specifications, independent of any traversal.
spec fn admitted(c: Seq<C>, ok: spec_fn(C) -> bool) -> bool {
    forall|i: int| 0 <= i < c.len() ==> ok(#[trigger] c[i])
}
spec fn least_fault(c: Seq<C>, ok: spec_fn(C) -> bool, k: int) -> bool {
    &&& 0 <= k < c.len() && !ok(c[k])
    &&& forall|i: int| 0 <= i < k ==> ok(#[trigger] c[i])
}
spec fn before(a: (int, int), b: (int, int)) -> bool {
    a.0 < b.0 || (a.0 == b.0 && a.1 < b.1)
}
spec fn least_decisive(g: Grid, p: spec_fn(int, int) -> Probe, at: (int, int)) -> bool {
    &&& in_grid(g, at) && p(at.0, at.1) != Probe::Miss
    &&& forall|c: (int, int)| in_grid(g, c) && before(c, at) ==> p(c.0, c.1) == Probe::Miss
}

// Models of the two production traversals.
spec fn admit(c: Seq<C>, ok: spec_fn(C) -> bool) -> Admission decreases c.len();
spec fn walk(g: Grid, p: spec_fn(int, int) -> Probe, from: int) -> (Outcome, Seq<(int, int)>);

// Principal lemmas.
proof fn admit_ok(c, ok)      ensures admit(c, ok) == Admission::Dirs(c) <==> admitted(c, ok);
proof fn admit_least(c, ok, k) ensures admit(c, ok) == Admission::Fault(k) <==> least_fault(c, ok, k);
proof fn decisive_unique(g, p, a, b)
    requires least_decisive(g, p, a), least_decisive(g, p, b), ensures a == b;
proof fn walk_first(g, p)     ensures match walk(g, p, 0).0 {
    Outcome::Found(c)   => least_decisive(g, p, c) && p(c.0, c.1) == Probe::Hit,
    Outcome::Faulted(c) => least_decisive(g, p, c) && p(c.0, c.1) == Probe::Fault,
    Outcome::Exhausted  => forall|c| in_grid(g, c) ==> p(c.0, c.1) == Probe::Miss,
};
proof fn walk_trace(g, p)     ensures trace_is_prefix(g, walk(g, p, 0));
proof fn resolve_closed(..)   ensures !paths_ok || !exts_ok ==> trace.len() == 0;
proof fn resolve_confined(..) ensures forall|i| 0 <= i < trace.len()
    ==> ok(components[(#[trigger] trace[i]).0]);
```

Using the sketch to test the design produced three consequences. Admission must
return the complete directory sequence before the search starts, so a lazy
combined iterator is ruled out. Search must report faults as outcomes, because
treating them as misses makes `L-SEARCH-FIRST` false. Columns must be a
non-empty sequence, so the already-extended case is one empty suffix and not an
empty list.

### Suggested BDD features

Proposed until EP-M3 adopts them. Runner: the library's unit tests under
`cargo nextest`. Path: `tests/features_internal/shell_resolution.feature`.
Links: `REQ-CLASS`, `REQ-ADMIT`, `REQ-CONFINE`, `REQ-VALUE`; EP-M3.

```gherkin
Feature: Resolve a shell selection into an execution value

  Scenario: The platform default ignores configured shells
    Given a Unix-like host with a configured shell "dash"
    When the platform default is resolved
    Then the execution value names "sh" at "/bin/sh" with arguments "-c"
    And the trusted search path was never examined

  Scenario: A configured bare executable is found in search order
    Given a Unix-like host whose PATH is "/opt/a:/opt/b"
    And "/opt/b/dash" is an executable file
    And a configured shell "dash" running "dash" with arguments "-c"
    When the shell "dash" is resolved
    Then the execution value names "dash" at "/opt/b/dash" with arguments "-c"
    And exactly "/opt/a/dash" then "/opt/b/dash" were examined

  Scenario: A Windows search tries extensions inside each directory
    Given a Windows host whose PATH is "C:\a;C:\b" and PATHEXT is ".COM;.EXE"
    And "C:\a\pwsh.exe" and "C:\b\pwsh.com" are executable files
    When the shell "pwsh" is resolved
    Then the resolved executable is "C:\a\pwsh.exe"

  Scenario: A repository-local candidate is never examined
    Given a Unix-like host whose PATH is "bin:/usr/bin"
    And "bin/bash" and "/usr/bin/bash" are executable files
    When the shell "bash" is resolved
    Then resolution fails because PATH entry 0 is not absolute
    And no file was examined

  Scenario: A malformed extension list is not replaced by the default
    Given a Windows host whose PATH is "C:\a" and PATHEXT is ".EXE;;.COM"
    And "C:\a\powershell.exe" is an executable file
    When the platform default is resolved
    Then resolution fails because PATHEXT entry 1 is empty
    And no file was examined

  Scenario Outline: Failure classes stay distinct
    Given <situation>
    When the shell "<name>" is resolved
    Then resolution fails as <class>

    Examples:
      | situation                                   | name       | class       |
      | a Unix-like host with no configured shells  | fish       | unknown     |
      | a Unix-like host with no configured shells  | powershell | unsupported |
      | a Unix-like host whose PATH has no "bash"   | bash       | unavailable |
      | a definition spelled for the other host     | dash       | misconfigured |
```

## Plan of work

Every milestone follows Red, Green, Refactor. State the red result in
`Progress`: the new test fails for the named reason before production code
changes, using a stub that returns the wrong class where the type must exist
for the test to compile. A second, accept-everything stub must also fail the
rejection cases.

### `EP-M0` feasibility probes (prototype, discarded)

Answer five questions in a scratch commit that is reverted before EP-M1. Record
the transcript in `Artefacts and notes`.

1. Does an item-scoped `cfg_attr(not(test), expect(dead_code, ...))` on one
   entry function, its type's constructor, and its accessors satisfy
   `make lint-clippy` and a default-feature build, with callees unmarked?
2. Do a `#[scenario]` and its steps compile and run inside the library test
   target with `strict-compile-time-validation`?
3. Does the catalogue audit accept a string selector on a message variable?
4. Does `make verus` accept the specification skeleton with the pinned release?
5. Does a new `src/` module that no harness imports need an entry in
   `tools/kani/proof-scope.toml`?

Go: all five answered, with any fallback recorded as a decision. No-go: no
compliant answer to question 1; stop and escalate.

### `EP-M1` shared lexical and inspection seams

Two behaviour-preserving extractions, each its own commit, so later milestones
add new behaviour only.

First, `src/shell_selection/host_path/mod.rs` gains the predicate and join of
D4, `spelling.rs` calls the predicate, and the build-slice and Kani registries
gain the file. Red: table tests for both families, including drive-relative,
rooted-without-drive, incomplete UNC, forward-slash, and joins with and without
a trailing separator. The existing spelling tests must pass unchanged.

Second, `src/stdlib/which/lookup/inspect/mod.rs` gains `ExecutableInspection`
and `inspect_executable`, `is_executable` delegates to it, and the facade
re-exports both as `pub(crate)`. Red: real-filesystem cases for the three
results and for a retained `NotADirectory` error. The existing `which` tests
must pass unchanged.

### `EP-M2` trusted search path admission

Create `src/shell_resolution/` with `search_path/mod.rs` (directories, columns,
and the admitted `TrustedSearchPath`), `search_path/path/mod.rs`,
`search_path/pathext/mod.rs`, `environment/mod.rs`, and the
`TrustedEnvironmentError` part of `error/mod.rs`. Proposed types:

```rust,ignore
/// Locate one rejected component without quoting it.
pub(crate) struct EntryIndex(usize);

pub(crate) enum PathFault { NotUnicode, Nul, Empty, Quoted, NotAbsolute }
pub(crate) enum PathExtFault {
    ValueNotUnicode,
    ValueNul,
    ValueEmpty,
    EntryBlank { index: EntryIndex },
    EntryMalformed { index: EntryIndex },
}
pub(crate) enum TrustedEnvironmentError {
    Path { index: EntryIndex, fault: PathFault },
    PathExt(PathExtFault),
}

/// Hold raw host values captured once, before any command overlay exists.
pub(crate) struct TrustedEnvironment { path: Option<OsString>, pathext: Option<OsString> }

impl TrustedEnvironment {
    pub(crate) fn capture(host: HostFamily, env: &impl Env) -> Self;
}
```

`capture` reads `PATH`, and reads `PATHEXT` on Windows only. Rules apply in
this order: for `PATH`, left to right, not Unicode, NUL, empty, quoted on
Windows, not absolute; for `PATHEXT`, not Unicode, NUL, and empty for the whole
value, then for each entry blank and malformed. An index is zero-based, matching
`shells[0]` and the existing `which` message.

The extension rules: an unset value is the fixed eleven-entry default; trim
only ASCII space and tab; require one or more ASCII alphanumerics after at most
one leading dot; lower-case; add the dot; keep first occurrences. Do not reuse
the test-gated `DEFAULT_PATHEXT` in `which`: state the list here, because
ADR-0019 fixes it for shells and the `which` list may change independently. One
test asserts that the two agree today, so that a divergence is a deliberate
edit.

Write `verus/shell_resolution/main.rs` with the admission lemmas and witnesses.

### `EP-M3` registry, search, and execution value

Add `registry`, `config`, `probe`, `filesystem`, `resolver`, and `resolved`,
and complete `error`. Proposed shape:

```rust,ignore
pub(crate) enum Inspection { Executable, NotExecutable, Missing }

/// Inspect one candidate. Owned by shell resolution; not a general service.
pub(crate) trait ExecutableProbe {
    fn inspect(&self, candidate: &Utf8Path) -> io::Result<Inspection>;
}

pub(crate) enum ExecutableReference { Absolute(Utf8PathBuf), Bare(String) }

/// Built-ins plus operator definitions, checked as a whole for one host.
pub(crate) struct ShellRegistry { /* host, configured entries by name */ }

pub(crate) enum ResolvedSelection { Direct, Shell(ResolvedShell) }

pub(crate) struct ResolvedShell { /* name, executable, args: all private */ }

pub(crate) struct ShellResolver<'a, P> { /* registry, environment, probe */ }

impl<'a, P: ExecutableProbe> ShellResolver<'a, P> {
    pub(crate) fn new(registry: &'a ShellRegistry, env: &impl Env, probe: P) -> Self;
    pub(crate) fn resolve(
        &self,
        selection: &ShellSelection,
    ) -> Result<ResolvedSelection, ShellResolutionError>;
}

pub(crate) enum ShellResolutionError {
    Unknown { name: ShellName },
    Unsupported { shell: BuiltInShell, host: HostFamily },
    Unavailable { name: ShellName, cause: UnavailableCause },
    Misconfigured(MisconfiguredShell),
    TrustedEnvironment { name: ShellName, source: TrustedEnvironmentError },
}

pub(crate) enum UnavailableCause {
    SearchPathUnset,
    NotOnSearchPath { executable: String },
    Missing { executable: Utf8PathBuf },
    NotExecutable { executable: Utf8PathBuf },
    Inspection { entry: Option<EntryIndex>, source: io::Error },
}
```

The registry owns the host family, and the resolver takes it from there, so the
two cannot disagree. `ShellRegistry::from_config(&CliConfig, HostFamily)` in
`config` re-applies `validate_composition` and `ShellExecutable::parse` for
that family and returns `Misconfigured` naming the configuration key; it never
yields a partial registry. In production the configuration loader has already
rejected such input, so this is a fail-closed check on values constructed
another way.

Resolution proceeds as follows. `Direct` returns at once. `PlatformDefault`
becomes the host-default built-in. A name is looked up among built-ins, then in
the registry. An unsupported built-in fails. An absolute executable is
inspected exactly as spelled. A bare name admits the search path, generates
candidates, and inspects them in order.

Complete the Verus kernel with the search lemmas. Add the classification table,
the search and admission properties, the real-filesystem tests, and the
scenarios. Include the confinement case from the roadmap on both families: a
probe that would accept `bin/bash`, `./bash`, and `<workspace>/bin/bash`, and a
`PATH` whose first component is relative, empty, or doubled, with the recorded
calls asserted empty. Show resistance to a command overlay by construction: the
resolver holds a snapshot, and a test changes the mock environment after
construction and observes the same result.

### `EP-M4` localized diagnostics

Add `src/localization/shell_selection_keys.rs`, re-exported by `keys.rs`, the
nine messages in all 35 catalogues, and `diagnostic/mod.rs`:

```rust,ignore
#[derive(Debug, Error, Diagnostic)]
pub(crate) enum ShellDiagnostic { /* one variant per class */ }

impl From<ShellResolutionError> for ShellDiagnostic { /* the single conversion */ }
```

Codes: `netsuke::shell::unknown`, `::unsupported`, `::unavailable`,
`::misconfigured`, and `::trusted_environment`. Keys under `shell_selection.`:
`unknown`, `unsupported`, `unavailable`, `misconfigured`, `environment.entry`,
`environment.value`, and the help texts `help.registry`, `help.install`, and
`help.environment`. Selectors on a closed `$cause` or `$fault` token keep
causes inside one message, so translators see whole sentences. Retain the I/O
error as the source, not as interpolated text. Reuse
`cli::config::shells::preview` for operator-supplied text. The match over error
variants has no wildcard arm.

Traps: a Latin-first Arabic or Persian value needs a leading U+200F mark;
placeholders must be identical in every catalogue; run the localization tests
under the localizer lock.

### `EP-M5` documentation and delivery closure

- ADR-0019: an addendum, "2026-11-25: Resolution details", recording D6 to D11
  and D15, the launch-time hazards, and the answers to OD-1 and OD-2. Use the
  date of writing.
- `docs/netsuke-design.md`: extend the shell-selection section with the
  resolution model, the invariants and proof-driven consequences, and the
  residual gaps.
- `docs/developers-guide.md`: replace the "future 11.2.3 resolver" text
  (line 1139 onward); document ownership, permitted imports, and the re-use
  policy for `ExecutableProbe`, `host_path`, and `inspect_executable`; add the
  fourth accepted Verus kernel; and add the key table.
- `docs/users-guide.md`, "Configure shell definitions", and
  `docs/sample-netsuke.toml`: describe how an operator's executable will be
  found, the `PATH` and `PATHEXT` requirements, and the residual trust in
  absolute `PATH` entries. State that no command uses a named shell yet.
- `docs/formal-verification-methods-in-netsuke.md`,
  `docs/repository-layout.md`, `docs/polonius.md` if a borrow-returning lookup
  is added, `docs/snapshot-testing-in-netsuke-using-insta.md` if a convention
  is added, and `CHANGELOG.md` only if a user-visible line is warranted.
- Rustdoc that says "future resolver" or "future adapter": search with
  `rg -n "future (trusted )?(resolver|adapter)|11\.2\.3" src docs`, excluding
  `docs/execplans/`.
- `.github/workflows/proofs.yml`: OD-4.
- Mark 11.2.3 done in `docs/roadmap.md`, set this plan to `COMPLETE`, and fill
  in the retrospective.

The OrthoConfig guide changes only if merge behaviour changes, which is not
expected. Configuration layering and localized help stay as 11.2.2 left them;
no flag, environment variable, or configuration key is added.

## Milestones and plateaus

Each milestone ends with a clean tree, a passing gate set, and a reviewable
commit series. Recovery in every case is `git revert` of that milestone's
commits; nothing persists data. Compatibility decision for every milestone:
none, because no released interface changes.

- `EP-M1`: advances `GAP-STRICT`. End state: shared seams with unchanged
  behaviour. Evidence: existing spelling and `which` suites pass unmodified.
  Conformance: the two public methods of D4 are the only interface change.
  Remaining: everything else.
- `EP-M2`: discharges `REQ-ADMIT` and `REQ-PATHEXT`. End state: a host snapshot
  becomes an admitted search path or one typed fault. Evidence: admission and
  extension suites; the kernel verifies. Conformance: no filesystem access and
  no read beyond two variables.
- `EP-M3`: discharges `REQ-REGISTRY`, `REQ-ORDER`, `REQ-CONFINE`, `REQ-CLASS`,
  and `REQ-VALUE`. End state: selections resolve. Evidence: classification,
  search, confinement, filesystem, and behavioural suites; the kernel verifies;
  negative controls recorded. Conformance: D2 imports hold; no trust boundary
  added.
- `EP-M4`: discharges `REQ-DIAG`. End state: one conversion, 35 complete
  catalogues. Evidence: snapshots, bound assertions, and catalogue audit.
- `EP-M5`: discharges `GAP-PROOF-CI` and documentation. Evidence: Markdown
  gates, workflow contracts, and a reconciled conformance basis.

## Concrete steps

Work from the repository root. Start by rebasing on `origin/main` and rereading
`AGENTS.md`. Use `codegraph` or `leta` for callers before each extraction.

Focused commands, in the repository's selection shape so that Cargo does not
rebuild the dependency graph:

```bash
set -o pipefail
cargo nextest run --workspace --all-targets --all-features shell_resolution \
  2>&1 | tee "/tmp/test-netsuke-$(git branch --show-current).out"
make verus VERUS_FLAGS='--proof-file verus/shell_resolution/main.rs' \
  2>&1 | tee "/tmp/verus-netsuke-$(git branch --show-current).out"
```

Expected before EP-M2 production code exists: the new tests fail on the stubbed
class. Expected afterwards: all pass, and Verus prints `N verified, 0 errors`
with `N` recorded here.

Full gate set at the end of every milestone, run in order by `scrutineer` with
one log per gate:

```bash
make check-fmt
make lint
make typecheck
make doc-coverage
make test
make test-workflow-contracts
make test-package-verify
make test-documentation-contracts
make verus VERUS_FLAGS='--proof-file verus/configured_shells/main.rs --proof-file verus/shell_resolution/main.rs'
make markdownlint
make nixie
```

`make test-package-verify` applies from EP-M1 because `build.rs` changes.
`make test-workflow-contracts` applies because the Kani scope and layout
contracts change. A failed gate is repaired and the whole set is rerun.

## Validation and acceptance

Accept the work when all of the following hold.

- On one Linux host, both families' tests pass for: ordered matching, duplicate
  and first-order preservation, the fixed default, extended names, empty and
  malformed `PATHEXT`, and every rejected `PATH` form.
- Removing any admission rule makes a named test fail, and the recorded probe
  calls for every rejected environment are empty.
- `true` resolves to `/bin/sh` on Unix-like hosts and to the first
  `powershell.exe` on Windows, for every registry.
- The five classes have distinct variants, codes, and snapshots, and no
  rendering contains a `PATH` or `PATHEXT` value.
- Verus reports zero errors for the new kernel, and each negative control fails
  for the recorded lemma.
- The `which` and configuration suites pass with no edited expectation.
- `make check-fmt`, `make lint`, and `make test` pass, as do the other gates
  above.

No new end-to-end test is planned, because no externally observable workflow
changes. The existing `tests/cli_tests/shells_startup/success.rs` case, in
which a definition for an uninstalled program does not block a build, guards
against accidental wiring. Task 11.3.2 owns the cross-platform end-to-end
matrix.

## Idempotence and recovery

All steps can be repeated. Property-test regression files are committed under
`proptest-regressions/` only for real failures; delete any produced by a seeded
fault. Commit before applying a mutation, and revert it with `git checkout --`
on the named file. If a rebase conflicts in `tools/kani/proof-scope.toml` or
`build.rs`, keep both sides and let the contract tests arbitrate.

## Interfaces and dependencies

No dependency changes. New public items: `HostFamily::is_absolute_path` and
`HostFamily::join_path`. New crate-internal items: those in EP-M2 to EP-M4, and
`stdlib::which::{ExecutableInspection, inspect_executable}`.

Reuse: `BuiltInShell`, `HostFamily`, `ShellName`, `ShellSelection`,
`ShellExecutable`, `ShellArguments`, `validate_composition`, `preview`,
`mockable::Env`, `camino`, `indexmap`, `thiserror`, `miette`, and
`localization::message`.

## Artefacts and notes

Transcripts are added here as milestones complete.

### External evidence

Retrieved on 2026-11-25.

[^go-empty]: Go, `src/os/exec/lp_windows.go`,
    <https://github.com/golang/go/blob/master/src/os/exec/lp_windows.go>, and
    the defect it answers, <https://github.com/golang/go/issues/61493>.

[^rust-command]: Rust, `library/std/src/sys/process/windows.rs`, `resolve_exe`,
    read from the pinned toolchain and from
    <https://github.com/rust-lang/rust/blob/main/library/std/src/sys/process/windows.rs>.
    Microsoft documents that `CreateProcessW` assumes no extension for an
    explicit application name and that batch files need `cmd.exe`:
    <https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw>.

[^batbadbut]: Rust security advisory for CVE-2024-24576,
    <https://blog.rust-lang.org/2024/04/09/cve-2024-24576/>.

## Revision note

- 2026-11-25: initial draft.
