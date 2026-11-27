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
and the host's own `PATH`, it returns one of two results:

- a complete execution value: the shell's name, one resolved absolute
  executable, and its fixed arguments; or
- a typed failure: the name is unknown, the built-in shell is unsupported on
  this host, the executable is unavailable, the definition is misconfigured, or
  the host's own `PATH` or `PATHEXT` is malformed.

`PATHEXT` is the Windows variable that lists the file extensions tried when a
program name has none.

The security property is the reason for the task. A Netsukefile chooses a name
from an allow-list and nothing more. No manifest value, command environment,
working directory, or workspace search may supply the executable, and no
directory is searched unless the operator's own `PATH` lists it as an absolute
path.

The abstract syntax tree (AST) for structured commands does not exist yet, so
no manifest can reach this code. The result is observable through tests, not
through the command line: each failure class, both host families' rules on
either build host, the exact sequence of files inspected, and the rendered
diagnostics. Task 11.3.1 connects the value to manifests after 12.3.1 supplies
the AST and the execution intermediate representation (IR). Existing command
execution, legacy recipes, the `which` template helper, and configuration
loading keep their current behaviour.

## Open decisions for the approver

Both questions concern accepted ADR text, so they belong to the maintainer. The
plan proceeds on each stated default unless the approver chooses otherwise.
Each answer changes proof statements and test expectations, so both must be
settled before `EP-M2` starts.

- **OD-1. One inadmissible `PATH` component disables every searched shell.**
  ADR-0019 rejects the whole search when any component is empty or not
  absolute, and this plan adds quoted, non-Unicode, and device-namespace
  components (D5, D11). Such components are ordinary on real hosts. The .NET
  SDK installs a literal `~/.dotnet/tools` into `/etc/paths.d` on
  macOS.[^dotnet-path] A stray `;` yields an empty component on Windows; Go met
  that as a field defect and now skips empty Windows entries "consistent with
  what PowerShell does".[^go-empty] A malformed `PATHEXT` entry has the same
  effect. On such a host `bash`, `pwsh`, every configured bare name, and, on
  Windows, `shell: true` all fail until the operator repairs the variable.
  Default: implement ADR-0019 exactly, with a diagnostic that locates the entry
  and says how to list entries. Alternative: amend ADR-0019 so that an
  inadmissible component is skipped, never converted or searched. That keeps
  confinement and gives up the fail-closed signal. It replaces the "never skip
  it" constraint, `L-ADMIT-OK`, `L-ADMIT-LEAST`, `L-FAULT-EXISTS`, part of
  `L-CLOSED`, two scenarios, and the index contract. `PATHEXT` would stay
  strict unless the amendment says otherwise.
- **OD-2. How `shell: true` finds Windows PowerShell.** ADR-0019 Table 1
  resolves `powershell.exe` through trusted `PATH`. ADR-0071, accepted later
  for legacy recipes, rejected exactly that route (its Option B) because the
  first match "may be a workspace-controlled decoy", and binds the Known Folder
  path instead. Tools such as `npm run` and Python virtual environments prepend
  absolute directories inside the repository, so the same run could use the
  trusted PowerShell for a legacy recipe and a committed `powershell.exe` for
  `shell: true`. Default: follow ADR-0019, because it is the accepted contract
  for this task and the roadmap repeats it. Recommendation: amend Table 1 so
  that the `powershell` built-in uses `src/power_shell_location/`. That choice
  adds an injected locator, a variant to the public `BuiltInExecutable`, about
  250 lines of code, and changes one scenario; it also takes the Windows
  platform default out of OD-1.

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
- No resolver parameter may let a command's environment overlay, a manifest
  value, or a rendered template supply `PATH`, `PATHEXT`, a directory, an
  executable, or an argument.
- Do not modify `src/ast/`, `src/ir/`, `src/ninja_gen/`, `src/runner/`,
  `src/manifest/`, `src/recipe_shell/`, or `src/power_shell_location/`. Do not
  wire the resolver into a manifest, a command, or the action plan.
- The resolver is crate-private. `src/lib.rs` gains `mod trusted_shell;`, never
  `pub mod`. The registry must not serve MiniJinja `shell()` or `grep()`, legacy
  `RecipeShell`, shebang handling, or general executable discovery.
- Keep the observable behaviour of the `which` filter, the `which()` function,
  and `command_available` unchanged, including errors, telemetry, and cache
  keys. The lenient parsers in `src/stdlib/which/env/path_support.rs` are that
  feature's contract and stay as they are.
- `src/shell_selection/` must not depend on `src/cli/`, on the filesystem, or
  on the environment. `src/shell_selection/name/mod.rs` keeps importing only
  `std`, `serde`, and `thiserror`.
- Read the environment only through `mockable::Env` (ADR-0008). No test may
  mutate the harness process's environment or working directory.
- `std::fs` is permitted only inside `netsuke::stdlib::which::lookup`, the
  bounded exemption in `dylint.toml` that `tests/whitaker_boundary_contract.rs`
  pins. Do not add an exemption.
- A value that holds raw `PATH` or `PATHEXT` text must not derive `Debug`. Its
  handwritten implementation reports presence only.
- Add no dependency, production or development. `googletest`,
  `pretty_assertions`, `rstest`, `rstest-bdd`, `proptest`, `insta`, and
  `mockable` are already present.
- Follow `AGENTS.md`: no file above 400 lines, every item documented, en-GB
  Oxford spelling, item-scoped `#[expect]` only, no `expect()` outside tests,
  and an explanatory message on every assertion.
- The pinned nightly supplies Polonius and the next-generation trait solver.
  Add no compiler flag and no defensive clone.
- Completed ExecPlans are historical. Do not edit the 11.2.1 or 11.2.2 plans.
- Verus kernels contain no `assume`, `admit`, external body, or added axiom.
- Run gates sequentially, through `scrutineer`, and never bypass compile
  admission.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main` when implementation starts.
Limits are net lines, additions minus deletions, and each category is enforced
on its own. ExecPlan files count towards nothing. Count with:

```bash
git diff --no-ext-diff --numstat "$(git merge-base origin/main HEAD)"..HEAD
```

Classify each whole file by the first rule that matches:

1. Excluded: `docs/execplans/**`.
2. Documentation: `docs/**` and `CHANGELOG.md`, including
   `docs/sample-netsuke.toml`.
3. Localization: `locales/**`.
4. Code: `*.rs`, `*.py`, `*.feature`, `*.snap`, `Makefile`, and `verus/**`.
5. Configuration: everything else, including `*.toml`, `*.yml`, and
   `proptest-regressions/**`.

Budgets, justified by the survey in `Context and orientation`:

- Files: 110 non-ExecPlan files, of which 35 are locale catalogues.
- Code: 4,700 net lines. The estimate is 3,800 (production 1,050; tests and
  snapshots 2,300; Verus 380; extraction refactors 70), plus 24 per cent.
- Localization: 1,300 net lines. The estimate is nine keys, or about 30 lines,
  in each of 35 catalogues, plus 24 per cent.
- Configuration: 70 net lines.
- Documentation: 600 net lines. Task 11.2.2 landed 401, and this task has more
  guides to reconcile.
- Interface: no new or changed public item, except nine constants in the public
  `localization::keys` table. OD-2's alternative, if chosen, also authorizes one
  `BuiltInExecutable` variant. Anything else stops work.
- Dependencies: any addition or version change stops work.
- Architecture: any need to widen a filesystem exemption, read another
  environment variable, or change ADR-0019's contract beyond the addendum in
  `EP-M5` stops work.
- Iterations: three failed attempts at one test, proof, or lint finding stop
  work.
- Time: eight hours of active work on one milestone, excluding compiler-slot
  queueing, stops work for a report.
- Proof: if a lemma in `Invariants and lemmas` is false, or needs an axiom,
  stop and revise the design. If Verus exceeds its default resource limit after
  two decompositions, stop and report.
- Ambiguity: if ADR-0019 or RFC 0011 admits two readings that change an outcome
  and this plan does not settle it, stop and present both.

## Risks

- Risk: an ordinary stray `PATH` entry makes named shells unusable (OD-1).
  Severity: high. Likelihood: high. Mitigation: decided before `EP-M2`; an
  actionable diagnostic; a follow-up issue that gates 11.3.1 if the default
  stands.
- Risk: two accepted ADRs disagree about locating Windows PowerShell (OD-2).
  Severity: high. Likelihood: high. Mitigation: decided before `EP-M2`.
- Risk: an absolute `PATH` component lies inside the repository, through
  `npm run`, a virtual environment, `direnv`, `/proc/self/cwd`, or a `..`
  component. Severity: high. Likelihood: high in those environments.
  Mitigation: ADR-0019 trusts the operator's `PATH`; the users' guide, the ADR
  addendum, and `INV-CONFINE` say exactly that and claim no more.
- Risk: the resolved path is not the program that later runs. Rust's Windows
  `Command` appends `.exe` to an absolute path without that suffix and prefers
  the suffixed file when it exists,[^rust-command] so a resolved
  `C:\tools\bash.com` can launch `C:\tools\bash.com.exe`. Batch files run under
  `cmd.exe`, whose quoting differs,[^batbadbut] and `.JS` or `.MSC` files are
  not programs. Severity: high. Likelihood: low. Mitigation: this task launches
  nothing; the hazards go into the ADR addendum and the 11.3.1 hand-off as
  launch-time obligations.
- Risk: PowerShell 7 on Windows is normally an App Execution Alias, a
  zero-length reparse point.[^pwsh-msix] Severity: medium. Likelihood: high.
  Mitigation: D7 accepts reparse points; a Windows-gated test guards against a
  later change that rejects them.
- Risk: `bash` on Windows can resolve to the Windows Subsystem for Linux
  launcher in `System32`. Severity: medium. Likelihood: high. Mitigation: the
  ADR fixes the order; the users' guide warns and 11.3.1 must print the
  resolved executable on launch failure.
- Risk: crate-private code with no production caller trips `dead_code` under
  `-D warnings`, and module-wide suppression is forbidden. Severity: medium.
  Likelihood: high. Mitigation: D3, the planning experiment, and the `EP-M0`
  probe.
- Risk: the file is replaced between resolution and launch, the execute bit
  does not mean effective access, or a 32-bit process sees redirected system
  directories. Severity: medium. Likelihood: low. Mitigation: documented
  residuals, as in ADR-0071.
- Risk: an unresponsive network directory in `PATH` delays resolution. A bare
  name costs at most one inspection per directory on Unix-like hosts and one
  per directory and extension on Windows, and no deadline bounds a blocking
  inspection. Severity: low. Likelihood: low. Mitigation: recorded as an
  accepted residual; 11.3.1 must resolve each distinct selection once per run.
- Risk: an unreachable Windows share is reported as absent, so a later
  directory wins silently, while a drive that is not ready or a name the file
  system rejects stops the search; `\\host\pipe\` paths also satisfy admission
  as UNC paths. Severity: low. Likelihood: low. Mitigation: accepted residuals,
  recorded in the addendum.
- Risk: Windows semantics pass on Linux and fail on Windows. Severity: medium.
  Likelihood: medium. Mitigation: all rules are text functions of an explicit
  `HostFamily`; the filesystem adapter has Windows-gated tests; snapshots
  contain no host-supplied text.
- Risk: 35 translations of varying quality, and 11.3.1 rewriting them.
  Severity: low. Likelihood: medium. Mitigation: nine keys, the glossary and
  style guide, right-to-left rules, and the catalogue audit; 11.3.1 wraps these
  messages with location context and does not replace them.
- Risk: duplicated effort with 11.2.4. Severity: low. Likelihood: medium.
  Mitigation: the boundary in D13.

## Progress

- [x] (2026-11-25) Renamed the branch, set the session title, loaded the
      required skills, and confirmed that 11.2.1 and 11.2.2 are merged.
- [x] (2026-11-25) Three Wyvern reconnaissance reports: `which` reuse,
      consumers and error boundaries, and gates and proof registration.
- [x] (2026-11-25) External research: Go and Rust executable lookup,
      `CreateProcessW`, batch-file quoting, .NET `PATH` entries, and PowerShell
      packaging.
- [x] (2026-11-25) First draft committed as `fd4a2d91d`.
- [x] (2026-11-25) Four-panel community-of-experts review applied; see the
      revision note.
- [x] (2026-11-25) Planning experiment on D3; see `Artefacts and notes`.
- [x] (2026-11-25) Second review round applied; see the revision note.
- [ ] Plan approved by the user, with OD-1 and OD-2 answered or defaulted.
- [ ] `EP-M0` Feasibility probes recorded and discarded.
- [ ] `EP-M1` Shared lexical and inspection seams extracted.
- [ ] `EP-M2` Trusted search path admitted, with its Verus admission proof.
- [ ] `EP-M3a` Domain resolver delivers the execution value, with the Verus
      search proof.
- [ ] `EP-M3b` Adapters connect configuration and the real filesystem, with
      behavioural scenarios.
- [ ] `EP-M4` Typed failures become localized diagnostics once.
- [ ] `EP-M5` Documentation, ADR addendum, CI kernel selection, and roadmap
      closure.

Record each later entry with a time as well as a date.

## Surprises & discoveries

- Observation: `apply_config` in `src/cli/merge/apply.rs` copies selected
  fields from `CliConfig` into `Cli` and does not copy `shells`. Evidence: read
  at `a97bd9491`; the only consumer of `CliConfig.shells` is
  `validate_configured_shells` in `src/cli/config/validation.rs`. Impact:
  11.3.1 must build the resolver before the configuration is reduced to `Cli`.
- Observation: `cap_std` cannot serve as the inspection adapter. Evidence:
  `dylint.toml` records that `cap_std` refuses symbolic links that leave the
  opened directory, which is how `/bin/sh` and `/usr/bin/pwsh` are commonly
  installed. Impact: reuse the sanctioned `which::lookup` boundary (D6).
- Observation: `CwdMode::Never` does not disable relative `PATH` entries.
  Evidence: `parse_path_entries` joins them to the working directory before the
  mode is consulted. Impact: calling `lookup` with fallbacks off would violate
  ADR-0019.
- Observation: a module named `shell_resolution` cannot land. Evidence:
  `tests/workflow_contracts/rust_module_layout_test.py` pins the exact sibling
  set `{shell_selection/, shell_word.rs}` for the `shell` prefix and rejects an
  addition; a second `shell_*` file under `src/localization/` fails as well.
  Impact: D1 and D17.
- Observation: CI proves one of the two committed Verus kernels, although
  `proofs.yml` says that proof code no job runs is a comment. Evidence:
  `VERUS_FLAGS` in the `Makefile` and the workflow's bare `make verus`. Impact:
  D14.
- Observation: on Windows, the pinned standard library maps an unreachable share
  (`ERROR_BAD_NETPATH`, `ERROR_BAD_NET_NAME`) and an invalid drive to
  `ErrorKind::NotFound`. Evidence: `library/std/src/sys/io/error/windows.rs`.
  Impact: such a directory is indistinguishable from an absent candidate, so D8
  cannot promise to stop there; recorded as a residual.
- Observation: marking only the entry points does not satisfy `dead_code`.
  Evidence: the planning experiment in `Artefacts and notes` left six warnings.
  Impact: D3 states the measured recipe, and the diagnostic conversions are
  inherent functions.
- Observation: no Fluent keys were added by 11.2.1 or 11.2.2; their messages
  are English `thiserror` text. Impact: this task introduces the first
  localized shell-selection messages.

## Decision log

Date and author for every entry unless stated: 2026-11-25, planning agent.
Entries marked "review" were changed by the expert panel.

- D1 (review). Decision: put the feature in a new crate-private module,
  `src/trusted_shell/`. Rationale: 11.2.1's D1 and the developers' guide ask
  for a sibling of `shell_selection` and suggested `shell_resolution` as an
  example; the module-layout contract rejects that name. `src/cli/` would pull
  the code into the build script's compiled slice, and `src/shell_selection/`
  would gain an environment dependency.
- D2 (review). Decision: separate a pure domain from adapters. The domain
  modules are `search_path`, which declares the `TrustedEnvironment` value,
  `registry`, `resolver`, `resolved`, and `error`. The adapters, which connect
  the domain to the outside world, are `config`, `environment`, `filesystem`,
  and `diagnostic`. Permitted outward imports are exact: `config` imports
  `crate::cli::config` types; `environment` imports `mockable`; `filesystem`
  imports `crate::stdlib`; `diagnostic` imports `miette`,
  `crate::localization`, and the bounded renderer
  `crate::cli::config::shells::preview`. Domain modules import only `std`,
  `camino`, `indexmap`, `thiserror`, and `crate::shell_selection`, and never
  import an adapter module. Check with the commands in `Concrete steps`.
  Rationale: the dependency rule of hexagonal architecture. Roadmap text asks
  for a registry "from `CliConfig`" while the developers' guide says the
  resolver accepts definitions; an adapter that translates one into the other
  satisfies both.
- D3. Decision: keep every item `pub(crate)` or narrower and attach
  `#[cfg_attr(not(test), expect(dead_code, reason = "..."))]`, naming roadmap
  11.3.1, to the smallest set of items that nothing uses yet. Rationale:
  `AGENTS.md` permits a temporary item-scoped expectation linked to tracked
  work, and `InterpreterBinding::InspectionOnly` is a precedent. A planning
  experiment measured the rule on the pinned nightly. Marking an inherent
  method or free function makes its callees, its type, and the variants it
  constructs live. Trait implementations do not count as uses, so a type
  reached only through one needs its own mark, and one mark on an enumeration
  covers its variants and fields. Prefer marked free functions and inherent
  conversions over trait implementations, which keeps the set small; in
  particular, the diagnostic conversions are inherent functions and not `From`
  implementations. A re-export that no live item uses is `unused_imports`,
  which this mark cannot cover, so add no such re-export. When 11.3.1 uses a
  marked item, the expectation becomes unfulfilled and the build fails until it
  is removed. The experiment confirmed that making the module public leaves
  every mark unfulfilled, so the same mechanism enforces privacy. Rejected: a
  public module, which contradicts "feature-private"; compiling only under
  `cfg(test)`, which leaves production unbuilt; and a premature consumer.
- D4 (review). Decision: extract the host-parameterized absoluteness predicate
  from `src/cli/config/shells/spelling.rs` into
  `src/shell_selection/host_path/mod.rs` as a `pub(crate)` method,
  `HostFamily::is_absolute_path(self, &str) -> bool`. Keep candidate joining
  private to `search_path`. Rationale: `PATH` admission needs the rule that
  configuration already applies, and two copies of a security predicate would
  drift. `std::path` applies the build host's rules, and tests must exercise
  both families on either host. Re-use policy: shell selection and shell
  configuration only.
- D5. Decision: split `PATH` with `OsStr::as_encoded_bytes` on the family's
  separator byte, decode each component with `str::from_utf8`, and reject a
  component that is not Unicode. Rationale: the encoding is safe to split at
  ASCII, and decoding per component yields the exact index of the first bad one.
  `std::env::split_paths` applies only the build host's rules and silently
  removes quotes on Windows. Rejecting non-Unicode follows ADR-0017's UTF-8
  path contract and the existing `which` behaviour; ADR-0019 does not state it,
  so the addendum must. A present empty `PATH` is one empty component at index
  0; an unset `PATH` is different (D9).
- D6 (review). Decision: reuse only the file inspection inside
  `src/stdlib/which/lookup/mod.rs`. Extract
  `probe_executable(&Utf8Path) -> io::Result<bool>`, which keeps every I/O
  error, and express `is_executable` through it. Rationale: the reuse survey
  found every other function unsuitable. `lookup`, `resolve_direct`,
  `handle_miss`, and `search_workspace` consult the working directory or
  workspace. `parse_path_entries` converts relative components. `parse_pathext`
  skips malformed entries and falls back to the default. `EnvSnapshot` and
  `WhichResolver` read an extra variable, emit telemetry, and cache.
  `candidate_paths` is compiled only on Windows. `InterpreterExecutable`
  applies the build host's absoluteness rule and belongs to a different policy.
  `bash_runtime` skips empty components and converts relative ones. A single
  Boolean keeps `which` ignorant of the resolver's vocabulary and avoids two
  identical enumerations.
- D7 (review). Decision: inspection follows symbolic links and accepts reparse
  points that the operating system reports as regular files, and the result is
  the path that was inspected, never its canonical form. Rationale: `/bin/sh`
  is usually a link; several shells change behaviour with the name they are
  invoked by; and the pinned standard library currently reports an App
  Execution Alias without following it, through a fallback added for socket
  files. Adopting ADR-0071's reparse rejection here would break the normal
  PowerShell 7 installation.
- D8 (review). Decision: an inspection yields `Executable`, `Missing`
  (`NotFound` or `NotADirectory`), or `Unusable` (not a regular file, no
  execute bit, or `PermissionDenied`). A bare-name search continues past
  `Missing` and `Unusable`, remembers the first `Unusable` entry, and reports
  it if the search is exhausted. Any other I/O error stops the search with an
  unavailable outcome that retains the error. Rationale: a candidate that this
  process may not reach is one it cannot execute, so continuing matches
  `execvp`; containers and `su` sessions commonly inherit unreadable
  directories such as `/root/.cargo/bin`. Remembering it keeps the permission
  error visible, as `AGENTS.md` requires. Stopping on every other error avoids
  guessing past a fault. The first draft stopped on `PermissionDenied` too.
- D9. Decision: capture raw `PATH`, and `PATHEXT` on Windows only, once; parse
  them only when a bare name is resolved. Rationale: ADR-0008 prefers data
  captured at the boundary; one snapshot makes every command in a run agree;
  and a malformed `PATH` must not block `/bin/sh` or an absolute configured
  shell, which do not search. An unset `PATH` is an unavailable outcome,
  because an absent variable has no malformed component, and no default search
  path is ever assumed. If `PATHEXT` is malformed and `PATH` is unset, the
  trusted-environment outcome wins.
- D10. Decision: on Windows, validate `PATH` first and `PATHEXT` second for
  every bare name, including one that already has an extension. Rationale:
  ADR-0019 Table 1 resolves `powershell.exe` through "trusted host `PATH` and
  `PATHEXT`", and the contract rejects a malformed value before inspection
  without excepting extended names. An absolute executable uses neither, and
  receives no suffix.
- D11 (review). Decision: on Windows, reject a component that contains `"` or
  begins with `\\.\` or `\\?\` in any slash spelling; on both families, reject
  a component that contains NUL. Rationale: quotes mean different things to
  different consumers and ADR-0019 requires ambiguous cases to fail. Device and
  verbatim namespaces satisfy the UNC pattern in the shared predicate but do
  not name ordinary directories; opening one can reach a pipe or device.
- D12 (review). Decision: run `rstest-bdd` scenarios inside the library's
  unit-test target, with the feature file beside its steps at
  `src/trusted_shell/tests/trusted_shell.feature`. Rationale: the subject is
  crate-private, so an integration test cannot reach it, and
  `scenarios!("tests/features")` in `tests/bdd_tests.rs` discovers every file
  in that directory and would fail on undefined steps. Use concrete typed steps
  and no outline that substitutes a whole step. `EP-M0` proves this works; the
  fallback is named `rstest` cases that quote the scenario text.
- D13 (review). Decision: this task generates raw host environments against a
  fixed registry, and probe outcomes against admitted search paths. Task 11.2.4
  generates selections, registries, YAML values, and configuration layers
  together with environments, reusing these generators. Rationale: the Verus
  lemmas need a tested correspondence to production code in the same change,
  and the roadmap gives 11.2.4 the cross-primitive properties.
- D14 (review). Decision: prove admission and search in Verus, test the
  `PATHEXT` grammar by property, and test classification by exhaustive table.
  In addition, name every committed kernel in `.github/workflows/proofs.yml`
  and add a small contract test that each `verus/*/main.rs` is named there.
  Leave the `Makefile` default alone, because
  `tests/prover_tools_make_tests.rs` pins it. Rationale: see
  `Formal verification decision`; the workflow's own header and estate rule
  PT-006 require CI to run held proofs.
- D15. Decision: `ResolvedShell` stores its name as `String`, not `ShellName`,
  and has private fields, read-only accessors, no `Serialize`, and one
  constructor visible only to the resolver. Rationale: ADR-0019 keeps the
  manifest-facing name type outside the execution IR. A versioned persisted
  form belongs to 11.3.1 and 12.3.1.
- D16. Decision: emit no tracing event here. Rationale: RFC 0011 section 7
  makes it optional, and `AGENTS.md` requires new telemetry to be exercised
  through the application's collection path, which does not exist until 11.3.1.
- D17 (review). Decision: deliver localized conversion now, appending nine keys
  to `src/localization/shell_keys.rs`. Rationale: the roadmap, ADR-0019, and
  RFC 0011 section 7 require one localized conversion. Extending the existing
  19-line table avoids a new key source, which would need edits to the audit,
  the build script, three test lists, and the layout contract.
- D18. Decision: add no configuration key, flag, environment variable, or help
  text. Rationale: OrthoConfig input reaches this task through
  `CliConfig.shells`, whose layering, authority, and localized help 11.2.2
  settled. `docs/ortho-config-users-guide.md` is reference material here and
  changes only if merge behaviour changes, which is not expected.
- D19 (review). Decision: inject inspection as a closure,
  `&impl Fn(&Utf8Path) -> io::Result<Inspection>`, and make confinement
  structural with an `AdmittedDirectory` newtype whose only constructor applies
  admission. Rationale: `AGENTS.md` prefers existing interfaces or narrow
  closures over speculative layers, and one operation needs no trait. The first
  draft used a port trait and a generic resolver.
- D20 (review). Decision: separate construction from resolution.
  `ShellResolver::new` returns `MisconfiguredShell`; `resolve` cannot. Both
  convert into the one diagnostic type. Rationale: the first draft placed
  misconfiguration inside a precedence that the constructor made unreachable. A
  domain-owned check means that no caller can build a registry with a reserved,
  repeated, or badly spelled entry; agreement with `validate_composition` and
  with `ShellExecutable::parse` is tested.
- D21 (review). Decision: define "already extended" textually, independent of
  the build host: a Windows bare name is extended when its last `.` is neither
  its first nor its last character. `python3.12` is therefore extended and gets
  no suffix; `.dash` is not extended; a trailing `.` is refused by D22.
  Rationale: ADR-0019 says "final component already has an extension" without
  defining it, and `Utf8Path::extension` would apply the build host's rules.
- D22 (review). Decision: on Windows, resolving an executable whose final
  component is unsafe is an unavailable outcome with the cause `UnsafeName`,
  after environment admission where a search is needed and without any
  inspection. A component is unsafe when it contains a control character or any
  of `<>:"|?*`, ends in a dot or space, or has a reserved stem: the text before
  its first `.`, with trailing spaces removed, equals, ignoring ASCII case,
  `CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, or `COM` or `LPT` followed
  by one of `0` to `9`, `¹`, `²`, or `³`. The rule applies to bare names and to
  the final component of an absolute executable, whose prefix must also satisfy
  D11's namespace rule. Rationale: `check_windows_spelling` accepts `dash:evil`,
  `nul`, and `bash.`; the first opens an alternate data stream and the last is
  not the file that Windows opens, which would falsify D7. ADR-0019 makes
  "cannot be resolved safely" an unavailable outcome. Refusing at construction
  instead would let one unused entry disable `shell: true`, which configuration
  must never affect. Configuration loading keeps its accepted grammar; the
  users' guide documents the stricter resolution rule.
- D23. Decision: record settled details as an addendum to ADR-0019 instead of a
  new ADR. Rationale: they refine an accepted decision and reverse none of it.
- D24 (review). Decision: keep Verus although the panel showed that exhaustive
  enumeration of small grids over the shipped traversal gives strong bounded
  evidence. Rationale: the user asked for an unbounded proof of introduced
  contractual logic. The enumeration is adopted as well, because it exercises
  production code where the proof covers a model.

## Outcomes & retrospective

Not started. At completion, compare the result with the purpose above, record
the actual budget use, and write these hand-offs.

- To 11.2.4: the generators, oracles, and recording inspection that can be
  reused, and the D13 boundary.
- To 11.3.1: construct the resolver from `CliConfig` before `apply_config`;
  remove the D3 marks; resolve each distinct selection once per run; attach
  action, target or rule, item, and stage context around the diagnostic; add
  tracing, including a count of skipped `Unusable` entries; show the resolved
  executable in launch failures; prove behaviourally that a command's `PATH`
  overlay cannot change the result; and, at launch, use the resolved path
  exactly, guard against the `.exe` substitution, and decide the policy for
  batch and script extensions.
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
`ShellName`, a `ShellExecutable`, and `ShellArguments`. `validate_composition`
rejects built-in and repeated names. `preview::preview` bounds text to 64
characters and escapes it for a diagnostic. `CliConfig.shells` is at
`src/cli/config/mod.rs:212`. The definitions are already admitted only from
system and user operator files.

`src/stdlib/which/` is an in-tree module; there is no `which` crate in the
dependency graph. In ADR-0019 and the roadmap, "invoking `which`" means this
module. `lookup::is_executable` (`src/stdlib/which/lookup/mod.rs:219`) follows
links, requires a regular file, and on Unix requires an execute bit. The module
`which` is private to `stdlib`.

`src/stdlib/which/resolve_error.rs` is the precedent for a crate-internal typed
error converted once (ADR-0005). `src/runner/error.rs` is the precedent for a
localized `miette` diagnostic with a stable code.

`build_l10n_audit/` fails the build when a declared key is missing from any of
the 35 catalogues in `locales/*/messages.ftl`, when a catalogue carries an
undeclared key, or when variables differ from `en-US`. A message may use a
Fluent selector; `ninja_gen.progress.command_list` does.

`src/shell_selection/` is also compiled into the build script. A nested file
there needs a `cargo:rerun-if-changed` directive in `build.rs`, and nothing
enforces it. `BUILD_SLICE_RERUN_PATHS` in
`tests/build_module_slice_ui_tests.rs` lists only `src/cli/` paths, so do not
add the file there. `tools/kani/proof-scope.toml` must list every file that a
Kani harness can reach; `make test-workflow-contracts` prints any difference.
Siblings under `src/` must not share a first name prefix, and files with child
modules must be named `mod.rs`.

`make proptest` selects only modules whose names end in `proptests` or
`property_tests`.

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

Terms: an adapter connects the domain to an external facility. A plateau is a
milestone's end state that is safe to stop at. Behaviour-driven development
(BDD) scenarios are `Given`, `When`, `Then` examples. An operator is whoever
controls system or user configuration and the process environment.

### Budget survey

Task 11.2.1 budgeted 2,600 net lines and 26 files and landed 1,468 code, 51
configuration, and 133 documentation lines in 23 files; one revision (its D11)
moved an unrelated timeout repair to another pull request to stay inside the
file limit. Task 11.2.2 set no numeric budget and landed 4,923 code, 34
configuration, and 401 documentation lines in 72 files, counting
`docs/sample-netsuke.toml` as documentation. Its excess came from three review
rounds that asked for evidence through the production entry point, plus
Windows-only test repairs. Task 6.1.2, the other Verus-backed feature, landed
3,767 code lines in 37 files. This task has no discovery work but adds a proof,
a filesystem adapter, and localized diagnostics, so its estimate sits between
the last two. The contingency covers the observed pattern of late evidence
requests, and splitting the largest milestone gives an earlier warning.

## Conformance basis

Revisions inspected at `a97bd9491`:

- Terms of reference, draft v0.2, last revised 2026-09-25.
- `docs/netsuke-design.md`, "Structured-command shell selection vocabulary".
  `docs/contents.md` lists no separate component document for shell selection,
  so the design document and developers' guide are its homes.
- ADR-0019 (accepted 2026-09-02, with addenda to 2026-11-06), RFC 0011
  (proposed), and RFC 0001 sections 10 and 23.
- ADR-0005, ADR-0008, ADR-0014, ADR-0017, ADR-0035, ADR-0043, and ADR-0071.
- Roadmap 11.2.3; issue #593 is the umbrella and stays open.

Requirements:

- `REQ-REGISTRY`: a feature-private registry built from `CliConfig`.
- `REQ-ORDER`: bare names search trusted `PATH`, and on Windows `PATHEXT`, in
  order, and select the first regular executable.
- `REQ-ADMIT`: malformed `PATH` or `PATHEXT` is rejected before inspection,
  naming the variable and, where one exists, a bounded index.
- `REQ-CONFINE`: no working-directory, workspace, or overlay lookup, and no
  inspection outside operator-listed absolute directories.
- `REQ-PATHEXT`: the exact grammar, default, normalization, and
  already-extended rule of ADR-0019.
- `REQ-CLASS`: unknown, unsupported, unavailable, misconfigured, and
  trusted-environment misconfiguration stay distinct.
- `REQ-VALUE`: the value carries only name, executable, and fixed arguments.
- `REQ-DIAG`: one conversion to bounded, localized user diagnostics.

Gaps: `GAP-RESOLVER`, no code maps a selection to an executable; `GAP-STRICT`,
existing parsers are lenient; `GAP-PROOF-CI`, CI omits a kernel.

Trace:

```plaintext
ADR-0019 registry   -> REQ-REGISTRY -> D2,D20     -> EP-M3a,b -> registry, config tests
ADR-0019 resolution -> REQ-ORDER    -> INV-FIRST  -> EP-M3a   -> search tests, kernel
ADR-0019 resolution -> REQ-ADMIT    -> INV-CLOSED -> EP-M2,3a -> admission tests, kernel
RFC-0011 5.3        -> REQ-CONFINE  -> INV-CONFINE-> EP-M3a   -> confinement tests, kernel
ADR-0019 PATHEXT    -> REQ-PATHEXT  -> INV-EXT    -> EP-M2    -> pathext tests
ADR-0019 errors     -> REQ-CLASS    -> INV-CLASS  -> EP-M3a,b -> classification tests, BDD
ADR-0019 lowering   -> REQ-VALUE    -> INV-VALUE  -> EP-M3a,b -> value, filesystem tests
ADR-0005, RFC-0011 7-> REQ-DIAG     -> INV-BOUND  -> EP-M4    -> diagnostic tests
```

## Verification plan

### Abstract model

A selection is `Direct`, `PlatformDefault`, or `Named(n)`. A host family fixes
a separator byte, an admissibility predicate on components, and whether
extensions apply.

Admission maps a component sequence `C` to an ordered directory sequence, or to
the least index of an inadmissible component. The extension input `E` is
`NotApplicable` (Unix-like), `Unset`, `ValueFault`, or `Entries(s)`. Columns
are derived separately: one empty suffix when `E` is `NotApplicable` or the
name is already extended, the fixed default when `Unset`, and the normalized
entries otherwise. Columns are never empty.

The candidate grid is the product of admitted directories and columns, ordered
by directory and then by column. A probe maps each cell to `Hit`, `Miss`, or
`Fault`. Resolution of a bare name produces an outcome and one event trace,
drawn from `AdmitPath(i)`, `AdmitExt(i)`, and `Inspect(d, k)`.

Preconditions: none on `C`, `E`, or the probe. Admission failure is an outcome,
not a violated precondition.

### Representation relation

| Model                | Production                                                                      | Established by                    |
| -------------------- | ------------------------------------------------------------------------------- | --------------------------------- |
| `C`                  | `PATH` bytes split on the separator; length at least one                        | admission examples and properties |
| `ok(C[i])`           | the D5 and D11 rules for component `i`                                          | tables for both families          |
| `dirs[d]`            | `AdmittedDirectory`, the decoded component                                      | private constructor               |
| `E`, columns         | `PathExt` parse result; the suffix list                                         | `INV-EXT`, `INV-EXTENDED`         |
| `p(d, k)`            | `alpha` of inspecting the joined candidate                                      | `INV-JOIN`, recording inspection  |
| `alpha`              | `Executable` is `Hit`; `Missing` and `Unusable` are `Miss`; an error is `Fault` | exhaustive table                  |
| admission `Fault(k)` | `EntryIndex(k)`; the kind of fault is not modelled                              | fault-kind tables                 |
| trace                | paths received by a recording inspection, in order                              | search properties                 |

_Table 1: How the proof model corresponds to production values._

An empty directory sequence exists only in the model, because a split always
yields at least one component. An unset `PATH` is outside the model;
`INV-CLASS` rows cover it with valid and malformed `PATHEXT` (D9).

### Invariants and lemmas

Selected for proof:

- `L-ADMIT-OK`: admission succeeds exactly when every component is admissible,
  and then the result equals `C`: nothing dropped, added, or reordered.
- `L-ADMIT-LEAST`: admission fails with `k` exactly when `C[k]` is inadmissible
  and every earlier component is admissible. `L-FAULT-EXISTS`: if `C` is not
  admitted, such a `k` exists.
- `INV-FIRST` (`L-SEARCH-EXACT`): search returns `Found(c)` exactly when the
  probe hits at `c` and misses at every earlier cell; `Faulted(c)` likewise for
  a fault; `Exhausted` exactly when every cell misses. The converse direction
  rests on `L-SEARCH-UNIQUE`, that the least decisive cell is unique.
- `L-SEARCH-TRACE`: the inspected cells are strictly ascending, and a cell is
  inspected exactly when it is in the grid and not after the decisive cell.
- `INV-CLOSED` (`L-CLOSED`): any `Inspect` event in the trace implies that `C`
  is admitted and `E` is well formed. If `C` is not admitted, the outcome is the
  `PATH` fault at the least index, whatever `E` is; otherwise, if `E` is
  malformed, the outcome is the `PATHEXT` fault.
- `INV-CONFINE` (`L-CONFINE`): every `Inspect(d, k)` satisfies `0 <= d < |C|`
  and `ok(C[d])`. The claim is lexical and concerns searched names: every path
  inspected for a bare name lies directly inside a directory that the operator's
  `PATH` lists as absolute. It says nothing about where such a directory is.

Established by test:

- `INV-JOIN`: for an admitted directory and an accepted bare file name, the
  candidate is absolute, its final component is exactly that name, and removing
  it leaves the directory up to one trailing separator.
- `INV-EXT`: an admitted `PATHEXT` is non-empty, lower case, matches
  `\.[a-z0-9]+`, has no duplicates, and keeps first-occurrence order; every
  violation of the raw grammar is rejected at its least index.
- `INV-EXTENDED`: D21 decides whether a suffix is appended.
- `INV-SPELLING`: D22 decides which names are unsafe, and a registry holding
  such an entry leaves every other selection unchanged.
- `INV-CLASS`: construction fails only as misconfigured. Resolution is total,
  with the precedence unknown, unsupported, trusted environment, unavailable.
  An earlier outcome prevents every later step.
- `INV-BUILTIN`: a built-in name never reaches a configured entry, and
  `PlatformDefault` gives the same result for every admitted registry.
- `INV-DIRECT`: `Direct` performs no lookup, parse, or inspection. This is how
  RFC 0011's "direct selection never enters shell resolution" is read here: the
  entry point accepts the whole selection so that callers match once, and
  returns before any resolution step.
- `INV-VALUE`: a resolved executable is absolute on the host family and is the
  inspected text, and the fixed arguments equal the table's or the
  definition's, element for element.
- `INV-BOUND`: a diagnostic contains no `PATH` or `PATHEXT` text, quotes at
  most 64 characters of any operator-supplied value, and renders an index as
  decimal digits below the component count.

### Axioms (trusted, not verified here)

- `AXIOM-ENV`: `mockable::DefaultEnv::os_string` returns the process value
  unchanged; Windows lookup is case-insensitive while `MockEnv` is not.
  Boundary: `environment`.
- `AXIOM-OSSTR`: `OsStr::as_encoded_bytes` is a superset of UTF-8 that is safe
  to split at ASCII bytes. Boundary: `search_path`.
- `AXIOM-META`: `std::fs::metadata` follows links, and reports an App Execution
  Alias as a regular file without following it. Boundary: `which::lookup`.
  Exercised against the real filesystem.
- `AXIOM-PROBE`: inspection is a function of the path during one resolution.
  Concurrent change is a stated residual.
- `AXIOM-INDEXSET`: `IndexSet` keeps first-insertion order. Exercised by the
  quadratic oracle for `INV-EXT`.
- `AXIOM-MERGE`: `CliConfig.shells` contains only operator-authorized entries,
  as 11.2.2 established.
- `AXIOM-OPERATOR`: the operator's `PATH` directories and the configured fixed
  arguments are trusted policy.

### Methods per obligation

Throughout, the fake and recording inspections key on `String`, and assertions
compare `as_str()`, because `Path` equality ignores repeated separators and, on
Windows, separator spelling. Use `pretty_assertions` for traces and argument
vectors, and `googletest` matchers for composite error values whose I/O source
cannot be compared. Focused command, unless stated: the `trusted_shell`
selection in `Concrete steps`.

**Admission and search lemmas.** Method: Verus over unbounded lengths,
exhaustive enumeration of production on small inputs, and property tests beyond
them.

- Artefacts: `verus/trusted_shell/main.rs`;
  `src/trusted_shell/tests/admission_property_tests.rs`,
  `search_property_tests.rs`, and `exhaustive.rs`.
- Evidence: the kernel command reports zero errors. The enumeration runs every
  probe assignment on every grid up to three by three, which is 19,683 cases at
  the largest, and every admission sequence up to length five over two distinct
  admissible forms and each inadmissible form.
- Oracles: admission takes the minimum failing index from a scan that evaluates
  every component. Search selects the least decisive cell with
  `min_by_key((directory, column))` over an unordered table, and builds
  expected paths from literal fragments, never from the production join.
  Recorded inspections must equal the oracle's prefix. Generators include
  duplicate directories.
- Expected red: with a stub that skips inadmissible components, the property
  reports a non-empty recording; with a stub that always reports exhaustion,
  the oracle disagrees at a hit.
- Non-vacuity: a proof function exhibits accepted input, a hit at cell (1, 1)
  of a two-by-two grid after three misses, a fault before a hit, exhaustion,
  failures at the first and last index, a full resolution trace that contains
  an inspection, and simultaneous `PATH` and `PATHEXT` faults. Property tests
  classify cases and fail if a class is absent from 1,000 deterministic samples.
- Negative controls, each applied to the model and to production separately, so
  that a named lemma and a named test both fail:
  - exchange the loops so that extension dominates directory (`L-SEARCH-EXACT`);
  - skip an invalid component, and reverse the admitted directories
    (`L-ADMIT-OK`);
  - report the last invalid index (`L-ADMIT-LEAST`);
  - treat a fault as a miss (`L-SEARCH-EXACT`);
  - inspect every cell before choosing (`L-SEARCH-TRACE`);
  - inspect one cell before admission completes (`L-CLOSED`);
  - emit an inspection at directory index `|C|` (`L-CONFINE`);
  - drop the column clause from the cell order (`L-SEARCH-UNIQUE`, model only).

**`INV-JOIN`.** Method: property test and table for both families, including
`/`, `C:\`, `C:/`, UNC roots, and trailing separators. Artefact:
`src/trusted_shell/search_path/join_property_tests.rs`. Expected red: a join
that always inserts a separator fails at `/`. Control: dropping the directory.

**`INV-EXT` and `INV-EXTENDED`.** Method: an `rstest` table for each rule, a
property test against an oracle built on the `regex` development dependency
with a quadratic first-occurrence check, and an exhaustive check of every ASCII
character as an entry's only character.

- Artefacts: `src/trusted_shell/search_path/pathext/tests.rs` and
  `property_tests.rs`.
- Domain: absent, empty, non-Unicode, NUL, blank, leading, trailing, and
  doubled separators, tabs, non-ASCII letters and spaces, interior dots, case
  variants, duplicates with and without dots, and the eleven-entry default. For
  `INV-EXTENDED`: `x`, `x.exe`, `.x`, `python3.11`, `pwsh-7.4`, and
  `powershell.exe`.
- Expected red: a stub that returns the default list for every value fails the
  empty-value case.
- Controls: trim Unicode whitespace, keep the last duplicate, default on empty,
  skip blank, and append to an extended name. Each fails a named case.

**`INV-SPELLING`.** Method: a Windows table, an exhaustive check of every ASCII
character in an interior position, and an isolation case in which an unsafe
configured entry leaves the platform default and every other name unchanged.
Artefact: `src/trusted_shell/registry/tests.rs`. Expected red: a stub that
accepts every name fails the colon row. Controls: drop the colon, trailing-dot,
and device-stem rules in turn; refuse at construction.

**`INV-CLASS`, `INV-BUILTIN`, and `INV-DIRECT`.** Method: an exhaustive
`rstest` table over selection kind, host family, and the finite registry,
environment, and inspection states. Artefact:
`src/trusted_shell/tests/classification.rs`.

- Non-vacuity: every row for an earlier class, and for `Direct`, the Unix
  platform default, and absolute executables, supplies a malformed `PATH` and
  `PATHEXT`, and an inspection that panics on any path outside the exact
  permitted one. A wrong precedence therefore changes the outcome or panics.
- A test-only unchecked registry holds configured entries named `sh` and
  `bash`, so that configured-first lookup fails a named test.
- The snapshot test expects `PATH`, and on Windows `PATHEXT`, exactly once from
  a strict `MockEnv`, then resolves repeatedly. Resistance to a command's
  `PATH` is structural here, because no parameter can carry one; 11.3.1 owes
  the behavioural proof.
- Expected red: a stub that admits the environment first reports a trusted
  environment fault for an unknown name.
- Controls: configured-first lookup; checking support after the environment;
  parsing before the `Direct` return.

**`INV-VALUE` and the real boundary.** Method: filesystem tests through the
production adapter with a temporary directory. Cases: a regular executable, a
directory of the same name, a file without an execute bit, a symbolic link, a
dangling link, a file used as a `PATH` directory, an unreadable directory
(skipped when the test runs as root), and `/bin/sh` on Unix; on Windows, `.exe`
selection and, when an alias is present, an App Execution Alias, with an
explicit skip message otherwise.

- Artefact: `src/trusted_shell/filesystem/tests.rs`.
- Fixed arguments are compared with literals, never with `fixed_args()`.
- Expected red: a stub that reports every path as missing fails the
  regular-executable case.
- Controls: canonicalize the result; drop an argument; reject a link.

**`INV-BOUND`.** Method: `insta` snapshots of each class in the human and JSON
renderings, with semantic assertions that sentinel directory names never
appear, that a 10,000-character name or path is truncated at 64 characters, and
that `Debug` output of every environment, admission, and error type contains no
sentinel. `ResolvedShell` is exempt, because its path is the result. Artefact:
`src/trusted_shell/diagnostic/tests.rs` and its snapshots, which contain no
host-supplied path, error text, or separator. Expected red: a stub that renders
every class as unknown fails four code assertions. Controls: interpolate the
component; truncate at 65; derive `Debug`.

### Formal verification decision

Verus is warranted for admission and search because the claims quantify over
every sequence length, the state is small and stable, and a defect has security
consequences. The specifications are traversal-free and the probe is arbitrary,
so the model cannot restate the implementation. Admission is simple on its own;
it is proved because `L-CLOSED` and `L-CONFINE` need it as a premise.

Verus is not warranted for the `PATHEXT` grammar, a finite character-class
predicate best checked against an independent regular expression; for
de-duplication, which production delegates to `IndexSet` and an oracle
exercises; for classification, a finite table; or for diagnostics, which are
presentation.

Kani is not selected. Its value is exhaustive exploration of small bounded
domains, and direct enumeration already covers those over the shipped code.
Adding harnesses would also enlarge the Kani proof scope and its CI cost.

Residual gaps, recorded in the design document: the proof covers a model, not
the shipped traversal; correspondence is exhaustive only within the stated
bounds and sampled beyond them; the filesystem can change after resolution; and
launch behaviour is outside this task.

### Proof sketch

The statements below are hypotheses about the proof's eventual shape.
Implementation may refine, split, or drop them, and records why in the
`Decision log`. Triggers, auxiliary inductions, and sequence algebra wait until
the verification conditions exist. A two-index traversal with a lexicographic
measure avoids non-linear arithmetic.

```rust,ignore
// Specifications, independent of any traversal.
spec fn admitted(c: Seq<C>, ok: spec_fn(C) -> bool) -> bool {
    forall|i: int| 0 <= i < c.len() ==> ok(#[trigger] c[i])
}
spec fn least_fault(c: Seq<C>, ok: spec_fn(C) -> bool, k: int) -> bool {
    &&& 0 <= k < c.len() && !ok(c[k])
    &&& forall|i: int| 0 <= i < k ==> ok(#[trigger] c[i])
}
spec fn in_grid(n: nat, m: nat, a: (int, int)) -> bool { 0 <= a.0 < n && 0 <= a.1 < m }
spec fn before(a: (int, int), b: (int, int)) -> bool {
    a.0 < b.0 || (a.0 == b.0 && a.1 < b.1)
}
spec fn least_decisive(n: nat, m: nat, p: spec_fn(int, int) -> Probe, at: (int, int)) -> bool {
    &&& in_grid(n, m, at) && p(at.0, at.1) != Probe::Miss
    &&& forall|c: (int, int)| in_grid(n, m, c) && before(c, at) ==> p(c.0, c.1) == Probe::Miss
}

// Models of the production traversals; m > 0 because columns are never empty.
spec fn admit(c: Seq<C>, ok: spec_fn(C) -> bool) -> Admission decreases c.len();
spec fn walk(n: nat, m: nat, p: spec_fn(int, int) -> Probe, d: int, k: int) -> Walk
    decreases n - d, m - k;
spec fn resolve(c: Seq<C>, e: Ext, extended: bool, ok: spec_fn(C) -> bool,
    p: spec_fn(int, int) -> Probe) -> (Outcome, Seq<Event>);

// Principal lemmas.
proof fn admit_ok(c, ok)         ensures
    (admit(c, ok) is Dirs) <==> admitted(c, ok),
    admitted(c, ok) ==> admit(c, ok) == Admission::Dirs(c);
proof fn admit_least(c, ok, k)   ensures admit(c, ok) == Admission::Fault(k) <==> least_fault(c, ok, k);
proof fn fault_exists(c, ok)     requires !admitted(c, ok), ensures exists|k| least_fault(c, ok, k);
proof fn decisive_unique(n, m, p, a, b)
    requires least_decisive(n, m, p, a), least_decisive(n, m, p, b), ensures a == b;
proof fn walk_exact(n, m, p, c)  ensures
    walk(n, m, p, 0, 0).found(c)   <==> least_decisive(n, m, p, c) && p(c.0, c.1) == Probe::Hit,
    walk(n, m, p, 0, 0).faulted(c) <==> least_decisive(n, m, p, c) && p(c.0, c.1) == Probe::Fault,
    walk(n, m, p, 0, 0).exhausted() <==> forall|x| in_grid(n, m, x) ==> p(x.0, x.1) == Probe::Miss;
proof fn walk_trace(n, m, p, x)  ensures strictly_ascending(walk(n, m, p, 0, 0).cells),
    walk(n, m, p, 0, 0).cells.contains(x) <==>
        in_grid(n, m, x) && forall|c| least_decisive(n, m, p, c) ==> !before(c, x);
// Below, `r` abbreviates `resolve(c, e, extended, ok, p)`.
proof fn resolve_outcome(c, e, extended, ok, p, k) ensures
    least_fault(c, ok, k) ==> r.0 == Outcome::PathFault(k),
    admitted(c, ok) && !ext_ok(e) ==> r.0 == Outcome::ExtFault,
    admitted(c, ok) && ext_ok(e) ==>
        outcome_and_cells(r) == walk(c.len(), columns(e, extended).len(), p, 0, 0);
proof fn resolve_closed(c, e, extended, ok, p, i)
    requires 0 <= i < r.1.len(), r.1[i] is Inspect,
    ensures admitted(c, ok) && ext_ok(e);
proof fn resolve_confined(c, e, extended, ok, p, i)
    requires 0 <= i < r.1.len(), r.1[i] is Inspect,
    ensures 0 <= r.1[i].dir() < c.len() && ok(c[r.1[i].dir()]);
```

Using the sketch to test the design produced these consequences. Admission must
return the complete directory sequence before the search starts, so one lazy
combined iterator is ruled out. Faults must be outcomes, because treating them
as misses makes `L-SEARCH-EXACT` false; `PermissionDenied` is therefore
classified before the model sees it, as an unusable candidate. Columns must be
a non-empty sequence distinct from the entries, so the already-extended case is
one empty suffix while its entries are still validated. Confinement needs a
join premise, which became `INV-JOIN`, and an admitted type, which became
`AdmittedDirectory`.

### Suggested BDD features

Proposed until `EP-M3b` adopts them. Runner: the library's unit tests under
`cargo nextest`. Path: `src/trusted_shell/tests/trusted_shell.feature`. Links:
`REQ-CLASS`, `REQ-ADMIT`, `REQ-CONFINE`, `REQ-VALUE`.

```gherkin
Feature: Resolve a shell selection into an execution value

  Scenario: The platform default never searches and ignores configured shells
    Given a Unix-like host whose PATH is "relative"
    And "/bin/sh" is an executable file
    And a configured shell "dash" running "/opt/dash" with arguments "-c"
    When the platform default is resolved
    Then the execution value names "sh" at "/bin/sh" with arguments "-c"
    And exactly "/bin/sh" was examined

  Scenario: A configured bare executable is found in search order
    Given a Unix-like host whose PATH is "/opt/a:/opt/b"
    And "/opt/b/dash" is an executable file
    And a configured shell "dash" running "dash" with arguments "-c"
    When the shell "dash" is resolved
    Then the execution value names "dash" at "/opt/b/dash" with arguments "-c"
    And exactly "/opt/a/dash" then "/opt/b/dash" were examined

  Scenario: A configured absolute executable ignores the search path
    Given a Windows host whose PATH is "relative" and PATHEXT is ";"
    And "C:\tools\dash" is an executable file
    And a configured shell "dash" running "C:\tools\dash" with arguments "-c"
    When the shell "dash" is resolved
    Then the execution value names "dash" at "C:\tools\dash" with arguments "-c"

  Scenario: A Windows search tries extensions inside each directory
    Given a Windows host whose PATH is "C:\a;C:\b" and PATHEXT is ".COM;.EXE"
    And "C:\a\pwsh.exe" is an executable file
    And "C:\b\pwsh.com" is an executable file
    When the shell "pwsh" is resolved
    Then the resolved executable is "C:\a\pwsh.exe"

  Scenario: An already extended name receives no suffix
    Given a Windows host whose PATH is "C:\a" and PATHEXT is ".COM;.EXE"
    And "C:\a\powershell.exe" is an executable file
    When the platform default is resolved
    Then exactly "C:\a\powershell.exe" was examined

  Scenario: A repository-local candidate is never examined
    Given a Unix-like host whose PATH is "bin:/usr/bin"
    And "bin/bash" is an executable file
    And "/usr/bin/bash" is an executable file
    When the shell "bash" is resolved
    Then resolution fails because PATH entry 0 is not absolute
    And no file was examined

  Scenario: A malformed extension list is not replaced by the default
    Given a Windows host whose PATH is "C:\a" and PATHEXT is ".EXE;;.COM"
    And "C:\a\powershell.exe" is an executable file
    When the platform default is resolved
    Then resolution fails because PATHEXT entry 1 is empty
    And no file was examined

  Scenario: An unreadable directory does not hide a later executable
    Given a Unix-like host whose PATH is "/root/bin:/usr/bin"
    And examining "/root/bin/bash" is denied
    And "/usr/bin/bash" is an executable file
    When the shell "bash" is resolved
    Then the resolved executable is "/usr/bin/bash"

  Scenario: An inspection fault stops the search
    Given a Unix-like host whose PATH is "/mnt/a:/usr/bin"
    And examining "/mnt/a/bash" fails
    And "/usr/bin/bash" is an executable file
    When the shell "bash" is resolved
    Then resolution fails as unavailable
    And exactly "/mnt/a/bash" was examined

  Scenario: An unknown name is reported before the environment
    Given a Unix-like host whose PATH is "relative"
    When the shell "fish" is resolved
    Then resolution fails as unknown

  Scenario: An unsupported built-in is reported before the environment
    Given a Unix-like host whose PATH is "relative"
    When the shell "powershell" is resolved
    Then resolution fails as unsupported

  Scenario: An unsafe configured name disables only itself
    Given a Windows host whose PATH is "C:\a" and PATHEXT is ".EXE"
    And "C:\a\powershell.exe" is an executable file
    And a configured shell "dash" running "dash:evil" with arguments "-c"
    When the platform default is resolved
    Then the resolved executable is "C:\a\powershell.exe"
    When the shell "dash" is resolved
    Then resolution fails as unavailable

  Scenario: A definition spelled for the other host prevents construction
    Given a Windows host whose PATH is "C:\a" and PATHEXT is ".EXE"
    And a configured shell "dash" running "/usr/bin/dash" with arguments "-c"
    When the resolver is built
    Then construction fails as misconfigured at "shells[0].executable"
```

## Plan of work

Every milestone follows Red, Green, Refactor. Record the red result in
`Progress`: the new test fails for the named reason before production code
changes, using a stub that returns the wrong class where a type must exist for
the test to compile. A second, accept-everything stub must also fail the
rejection cases. Commit after each coherent step, not only at milestones.

### `EP-M0` feasibility probes (prototype, discarded)

Run `make install-verus` first. Answer these questions in a scratch commit that
is reverted before `EP-M1`, and record the transcript in `Artefacts and notes`.

1. In this crate, does the D3 recipe satisfy `make lint-clippy`,
   `make lint-default-features`, and `make test-default-features`? Record the
   minimal marked set for each plateau, and repeat the two controls: removing
   one mark, and making the module public. Include enumerations whose variants
   only tests construct or whose fields only tests read.
2. Do a `#[scenario]` and typed steps compile and run inside the library test
   target with `strict-compile-time-validation`, and does a renamed step fail
   to compile?
3. Does the catalogue audit accept a string selector on a message variable?
4. Does `make verus` accept the specification skeleton with the pinned release?
5. Survey the `PATH` values of the hosted Linux, macOS, and Windows runners,
   counting inadmissible components without recording their text, as evidence
   for OD-1. Record whether the Windows runner has an App Execution Alias; if
   it has none, state that the D7 guard is unexercised in CI.

Go: every question answered, with any fallback recorded as a decision. No-go:
no compliant answer to question 1; stop and escalate.

### `EP-M1` shared lexical and inspection seams

Two behaviour-preserving extractions, each its own commit.

First, `src/shell_selection/host_path/mod.rs` gains the predicate of D4 and
`spelling.rs` calls it. Add the `build.rs` directive and the Kani scope entry.
Red: table tests for both families, including drive-relative,
rooted-without-drive, incomplete UNC, and forward slashes. The existing
spelling tests pass unchanged. Use the focused form in `Concrete steps` with
`-E 'test(host_path) | test(shells::)'`.

Second, `probe_executable` is extracted inside
`src/stdlib/which/lookup/mod.rs`, visible only to `which` for now, and
`is_executable` delegates to it. Red: real-filesystem cases for an executable,
a directory, a file without an execute bit, and retained `NotFound` and
`NotADirectory` errors. The existing `which` tests pass unchanged. Focused
filter: `-E 'test(stdlib::which)'`.

Do not add a re-export yet: nothing would use it, and `unused_imports` would
fail the build.

### `EP-M2` trusted search path admission

Create `src/trusted_shell/` with `search_path/mod.rs` (the `TrustedEnvironment`
value and admitted directories), `search_path/path/mod.rs`,
`search_path/pathext/mod.rs`, `environment/mod.rs`, and the environment part of
`error/mod.rs`. Column derivation and candidate joining wait for `EP-M3a`,
which is their first caller. D3 marks at this plateau: `environment::capture`,
the admission entry point, and any error enumeration whose fields only tests
read. Proposed types:

```rust,ignore
/// Locate one rejected component without quoting it.
pub(crate) struct EntryIndex(usize);

pub(crate) enum PathFault { NotUnicode, Nul, Empty, Quoted, DeviceNamespace, NotAbsolute }
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
pub(crate) struct TrustedEnvironment { /* host, path, pathext: private */ }

/// A directory that admission accepted; no other constructor exists.
pub(in crate::trusted_shell) struct AdmittedDirectory(String);
```

`environment::capture(HostFamily, &impl Env) -> TrustedEnvironment` is a free
function in the adapter. It reads `PATH`, and reads `PATHEXT` on Windows only.
The value it builds, through a constructor visible only inside `trusted_shell`,
is the single owner of the host family. Rules apply in this order: for each
`PATH` component, left to right, not Unicode, NUL, empty, quoted, device
namespace, not absolute; for `PATHEXT`, not Unicode, NUL, and empty for the
whole value, then for each entry blank and malformed. An index is zero-based,
matching `shells[0]` and the existing `which` message.

The extension rules: an unset value is the fixed eleven-entry default; trim
only ASCII space and tab; require one or more ASCII alphanumerics after at most
one leading dot; lower-case; add the dot; keep first occurrences. Do not reuse
the test-gated `DEFAULT_PATHEXT` in `which`: it is private to that module and
serves a lenient contract. State the list here, and compare it in a test with
the literal ADR-0019 sequence.

Write `verus/trusted_shell/main.rs` with the admission lemmas, their witnesses,
and the admission controls. Acceptance includes:

```bash
make verus VERUS_FLAGS='--proof-file verus/trusted_shell/main.rs'
```

It must print `N verified, 0 errors`; record `N`.

### `EP-M3a` domain resolver and search proof

Add `registry`, `resolver`, and `resolved`, and complete `error`. All of it is
pure: inspection is a closure. D3 marks move to `ShellResolver::new`,
`ShellResolver::resolve`, and the `ResolvedShell` accessors, with one mark each
on `ResolvedSelection`, `Inspection`, `UnusableReason`, and each error
enumeration whose fields are still unread; remove the admission mark. Proposed
shape:

```rust,ignore
pub(crate) enum Inspection { Executable, Missing, Unusable(UnusableReason) }
pub(crate) enum UnusableReason { NotRegularOrExecutable, PermissionDenied }

/// One operator entry, translated from configuration without a CLI type.
pub(crate) struct ConfiguredShell { name: ShellName, executable: String, args: Vec<String> }

/// Private fields; `parse` applies the host's spelling rules, and
/// `from_built_in` is infallible.
pub(in crate::trusted_shell) struct ExecutableReference { /* ... */ }

/// Key, definition name, and a closed fault; no CLI type.
pub(crate) struct MisconfiguredShell { index: usize, name: ShellName, fault: ShellFault }

pub(crate) struct ShellResolver { /* environment, registry: private */ }

impl ShellResolver {
    pub(crate) fn new(
        environment: TrustedEnvironment,
        configured: Vec<ConfiguredShell>,
    ) -> Result<Self, MisconfiguredShell>;

    pub(crate) fn resolve(
        &self,
        selection: &ShellSelection,
        inspect: &impl Fn(&Utf8Path) -> io::Result<Inspection>,
    ) -> Result<ResolvedSelection, ShellResolutionError>;
}

pub(crate) enum ResolvedSelection { Direct, Shell(ResolvedShell) }
pub(crate) struct ResolvedShell { /* name, executable, args: private */ }

pub(crate) enum ShellResolutionError {
    Unknown { name: ShellName },
    Unsupported { shell: BuiltInShell, host: HostFamily },
    TrustedEnvironment { name: ShellName, source: TrustedEnvironmentError },
    Unavailable { name: ShellName, cause: UnavailableCause },
}

pub(crate) enum UnavailableCause {
    SearchPathUnset,
    NotOnSearchPath { executable: String, first_unusable: Option<(EntryIndex, UnusableReason)> },
    Missing { executable: Utf8PathBuf },
    Unusable { executable: Utf8PathBuf, reason: UnusableReason },
    UnsafeName { executable: String },
    Inspection { entry: Option<EntryIndex>, source: io::Error },
}
```

`new` checks reserved names, repeated names, and spelling for the environment's
host, and never yields a partial registry. In production the configuration
loader has already rejected most such input, so this is a fail-closed check on
values constructed another way. Properties over both families assert that its
collision verdict equals `validate_composition`'s and that
`ExecutableReference::parse` accepts exactly what `ShellExecutable::parse`
accepts.

Resolution proceeds as follows. `Direct` returns at once. `PlatformDefault`
becomes the host-default built-in. A name is looked up among built-ins, then in
the registry. An unsupported built-in fails. An absolute executable passes D22
and is inspected exactly as spelled. A bare name admits the search path, passes
D22, generates candidates, and inspects them in order.

Complete the kernel with the search lemmas and their controls, and add the
classification table, the enumeration, and the search properties. Include the
roadmap's confinement case on both families: an inspection that would accept
`bin/bash`, `./bash`, and `<workspace>/bin/bash`, and a `PATH` whose first
component is relative, empty, or doubled, with recorded inspections asserted
empty. Acceptance includes the kernel command above with the new count.

### `EP-M3b` adapters and behaviour

Add `config` and `filesystem`, and the scenarios. `config` turns `&CliConfig`
into `Vec<ConfiguredShell>`. `filesystem::inspect` calls `probe_executable`,
which now gains `pub(crate)` visibility with re-exports through
`src/stdlib/which/mod.rs` and `src/stdlib/mod.rs`, because `which` is a private
module; it alone translates `NotFound` and `NotADirectory` to `Missing` and
`PermissionDenied` to `Unusable`. D3 marks added: the two adapter functions;
removed: those on `Inspection` and `UnusableReason`, which the filesystem
adapter now constructs.

Add the real-filesystem tests and the `rstest-bdd` scenarios, whose steps build
the domain values directly. One test drives `CliConfig::merge_from_layers`
through `config` into `ShellResolver::new`, so the production merge is
exercised and not only the domain constructor.

### `EP-M4` localized diagnostics

Append nine keys to `src/localization/shell_keys.rs` and the messages to all 35
catalogues, and add `diagnostic/mod.rs`:

```rust,ignore
#[derive(Debug, Error, Diagnostic)]
pub(crate) enum ShellDiagnostic { /* one variant per class */ }

impl ShellDiagnostic {
    pub(crate) fn from_resolution(error: ShellResolutionError) -> Self;
    pub(crate) fn from_configuration(error: MisconfiguredShell) -> Self;
}
```

| Class               | Code                                  | Message key                   | Help key           |
| ------------------- | ------------------------------------- | ----------------------------- | ------------------ |
| unknown             | `netsuke::shell::unknown`             | `unknown`                     | `help.registry`    |
| unsupported         | `netsuke::shell::unsupported`         | `unsupported`                 | `help.registry`    |
| unavailable         | `netsuke::shell::unavailable`         | `unavailable`                 | `help.install`     |
| misconfigured       | `netsuke::shell::misconfigured`       | `misconfigured`               | `help.registry`    |
| trusted environment | `netsuke::shell::trusted_environment` | `environment.entry`, `.value` | `help.environment` |

_Table 2: Diagnostic classes, codes, and message keys under `shell_selection.`._

`help.registry` lists the built-ins supported on this host and the `[[shells]]`
key, as ADR-0019 requires. `misconfigured` names the configuration key and the
definition. `help.environment` gives a command, passed as a placeable, that
lists the variable's entries with the same zero-based numbering. `unavailable`
mentions the first unusable entry's index when there is one. Selectors on a
closed `$cause` or `$fault` token keep causes inside one message, so
translators see whole sentences. Retain the I/O error as the source, not as
interpolated text. Render operator-supplied text with `preview`. Matches over
error variants have no wildcard arm.

Also update the prefix table and key-source text in
`docs/translators-guide.md`, the module documentation in `shell_keys.rs`, and,
if a catalogue test enumerates prefixes, that list. Traps: a Latin-first Arabic
or Persian value needs a leading U+200F mark; placeables must be identical in
every catalogue; run the localization tests under the localizer lock; touching
`locales/` triggers the Kani smoke job. Focused filter:
`-E 'test(trusted_shell) | test(localization) | test(l10n)'`.

D3 marks at this plateau: the two conversions join the `EP-M3b` set, and the
marks on the error enumerations go, because the conversions read every field.

### `EP-M5` documentation and delivery closure

- ADR-0019: a dated addendum recording D5 to D11, D15, D21, and D22, the
  launch-time hazards and other residuals from `Risks`, and the answers to OD-1
  and OD-2.
- `docs/netsuke-design.md`: extend the shell-selection section with the
  resolution model, the invariants and the API consequences that the proof
  sketch drove, and the residual gaps.
- `docs/developers-guide.md`: replace the "future 11.2.3 resolver" text (line
  1139 onward); document ownership, permitted imports, and the re-use policy for
  `host_path`, `probe_executable`, and the inspection closure; update "Three
  are accepted" (line 8470), the kernel list near line 13037, and the key-table
  text near line 1003.
- `docs/users-guide.md`, "Configure shell definitions", and
  `docs/sample-netsuke.toml`: describe how an operator's executable will be
  found, the `PATH` and `PATHEXT` requirements, the stricter Windows bare-name
  rule, the dotted-name consequence, the Windows Subsystem for Linux warning,
  and the trust placed in absolute `PATH` entries. State that no command uses a
  named shell yet.
- `docs/formal-verification-methods-in-netsuke.md` (the kernel statements near
  lines 66, 200, and 282), `docs/repository-layout.md`, `docs/polonius.md` if a
  borrow-returning lookup is added, and `CHANGELOG.md` only if a user-visible
  line is warranted.
- Rustdoc that says "future resolver" or "future adapter": use the
  last search in `Concrete steps`.
- `.github/workflows/proofs.yml`, its header comment, and the contract test of
  D14.
- If OD-1 stands at its default, open an issue that gates 11.3.1 and reference
  it from the addendum.
- Mark 11.2.3 done in `docs/roadmap.md`, annotate 11.2.4 with the D13 boundary,
  set this plan to `COMPLETE`, and fill in the retrospective.

## Milestones and plateaus

Each milestone ends with a clean tree, a passing gate set, and a reviewable
commit series. Recovery is `git revert` of that milestone's commits; nothing
persists data. Compatibility decision for every milestone: none, because no
released interface changes. At each boundary, rerun the D2 import check,
compare budgets, and confirm that OD-1 and OD-2 still match the implementation.

- `EP-M1`: advances `GAP-STRICT`. End state: shared seams with unchanged
  behaviour. Evidence: existing spelling and `which` suites pass unmodified.
  Remaining: everything else.
- `EP-M2`: discharges `REQ-PATHEXT` and the parsing half of `REQ-ADMIT`. End
  state: a host snapshot becomes an admitted search path or one typed fault.
  Evidence: admission and extension suites; admission lemmas verified. No
  filesystem access, and no read beyond two variables.
- `EP-M3a`: discharges `REQ-ORDER`, `REQ-CONFINE`, `REQ-CLASS`, and the rest of
  `REQ-ADMIT`. End state: selections resolve against an injected inspection.
  Evidence: classification, enumeration, search, and confinement suites; all
  lemmas verified; controls recorded.
- `EP-M3b`: discharges `REQ-REGISTRY`, `REQ-VALUE`, and `GAP-RESOLVER`. End
  state: selections resolve from real configuration and files. Evidence:
  filesystem, merge, and behavioural suites on Linux and Windows.
- `EP-M4`: discharges `REQ-DIAG`. End state: one conversion, 35 complete
  catalogues. Evidence: snapshots, bound assertions, and the catalogue audit.
- `EP-M5`: discharges `GAP-PROOF-CI` and documentation. Evidence: Markdown
  gates, workflow contracts, and a reconciled conformance basis.

## Concrete steps

Work from the repository root. Start by rebasing on `origin/main` and rereading
`AGENTS.md`. Use `codegraph` or `leta` for callers before each extraction.

Focused tests, in the gate's selection shape so that Cargo does not rebuild the
dependency graph:

```bash
set -o pipefail
cargo nextest run --workspace --all-targets --all-features \
  -E 'test(trusted_shell)' \
  2>&1 | tee "/tmp/test-netsuke-$(git branch --show-current).out"
```

Expected before a milestone's production code exists: the new tests fail with
the stubbed class. Expected afterwards: all pass.

Import check for D2; each line must print nothing outside the named adapter:

```bash
git grep -n 'crate::cli' -- src/trusted_shell ':!src/trusted_shell/config' \
  ':!src/trusted_shell/diagnostic' ':!src/trusted_shell/tests'
git grep -n 'mockable' -- src/trusted_shell ':!src/trusted_shell/environment' \
  ':!src/trusted_shell/tests'
git grep -nE 'crate::stdlib|std::fs|std::env' -- src/trusted_shell \
  ':!src/trusted_shell/filesystem' ':!src/trusted_shell/tests'
git grep -nE 'miette|crate::localization' -- src/trusted_shell \
  ':!src/trusted_shell/diagnostic' ':!src/trusted_shell/tests'
```

```bash
git grep -nE '(super|trusted_shell)::(config|environment|filesystem|diagnostic)' \
  -- src/trusted_shell/search_path src/trusted_shell/registry \
  src/trusted_shell/resolver src/trusted_shell/resolved src/trusted_shell/error
```

The first block exempts `src/trusted_shell/tests/`, where every test that
imports `crate::cli` lives. The second must print nothing.

Search for stale forward references before closing `EP-M5`:

```bash
git grep -nE 'future (trusted )?(resolver|adapter)|11\.2\.3' -- src docs \
  ':!docs/execplans'
```

Full gate set at the end of every milestone, run in order by `scrutineer` with
one log per gate:

```bash
make check-fmt
make lint
make lint-default-features
make typecheck
make doc-coverage
make test
make test-default-features
make proptest
make test-workflow-contracts
make test-package-verify
make test-documentation-contracts
make markdownlint
make nixie
```

From `EP-M2`, add the proof gate, which must report zero errors for each file:

```bash
KERNELS="--proof-file verus/canonical_value/main.rs"
KERNELS="$KERNELS --proof-file verus/configured_shells/main.rs"
KERNELS="$KERNELS --proof-file verus/trusted_shell/main.rs"
make verus VERUS_FLAGS="$KERNELS"
```

`make test-package-verify` applies because `build.rs` changes in `EP-M1`. A
failed gate is repaired and the whole set is rerun. `make check-fmt`,
`make lint`, and `make test` are mandatory at every milestone without exception.

## Validation and acceptance

Accept the work when all of the following hold.

- On one Linux host, both families' tests pass for ordered matching, duplicate
  and first-order preservation, the fixed default, extended names, empty and
  malformed `PATHEXT`, and every rejected `PATH` form.
- Removing any admission rule makes a named test fail, and the recorded
  inspections for every rejected environment are empty.
- `true` resolves to `/bin/sh` on Unix-like hosts and, unless OD-2 changes it,
  to the first `powershell.exe` in `PATH` order on Windows, for every admitted
  registry.
- The five classes have distinct variants, codes, and snapshots, and no failure
  rendering, including `Debug`, contains a `PATH` or `PATHEXT` value.
- Verus reports zero errors for all three kernels, and each negative control
  fails for the recorded lemma and the recorded test.
- The `which` and configuration suites pass with no edited expectation.
- All gates above pass, on Linux locally and on Windows in CI.

No new end-to-end test is planned, because no externally observable workflow
changes. The existing `tests/cli_tests/shells_startup/success.rs` case, in
which a definition for an uninstalled program does not block a build, guards
against accidental wiring. Task 11.3.2 owns the cross-platform end-to-end
matrix. No compile-time `trybuild` test is planned; D3 makes a public module
fail the ordinary build.

## Idempotence and recovery

All steps can be repeated. Property-test regression files are committed under
`proptest-regressions/` only for real failures; delete any produced by a seeded
fault. Commit before applying a mutation, and restore the named file from that
commit. If a rebase conflicts in `tools/kani/proof-scope.toml` or `build.rs`,
keep both sides and let the contract tests arbitrate.

## Interfaces and dependencies

No dependency changes. No new public item except the nine key constants. New
crate-internal items: those in `EP-M2` to `EP-M4`,
`HostFamily::is_absolute_path`, and `stdlib::probe_executable`.

Reuse: `BuiltInShell`, `HostFamily`, `ShellName`, `ShellSelection`,
`ShellDefinition` and its field types in the `config` adapter, `preview`,
`mockable::Env`, `camino`, `indexmap`, `thiserror`, `miette`, and
`localization::message`. `validate_composition` serves as an independent oracle
for the domain's collision rule.

## Artefacts and notes

### Planning experiment on D3

Date: 2026-11-25. Hypothesis: marking only the entry points with the D3
attribute is sufficient. Method: a standalone scratch crate on the pinned
nightly (`nightly-2026-11-10`) with a crate-private module modelled on the
first draft, checked with `cargo check`, `cargo clippy -- -D warnings`, and
`cargo test --no-run`. The scratch crate was removed. Verdict: falsified.

```plaintext
no marks                                         11 warnings
marks on constructor, resolve, and accessors      6 warnings
  remaining: types used only by a trait impl, variants constructed only there,
  error fields read only by a From impl, one variant never constructed
plus marks on the two such structs                4 warnings
plus one mark on each of the two enumerations     0 warnings, all three commands
control: remove one method mark                   dead_code reported
control: make the module public                   9 unfulfilled expectations
control: unused crate-visible re-export           unused_imports reported
```

### External evidence

Retrieved on 2026-11-25.

[^go-empty]: Go, `src/os/exec/lp_windows.go`,
             <https://github.com/golang/go/blob/master/src/os/exec/lp_windows.go>,
             and the defect it answers,
             <https://github.com/golang/go/issues/61493>.

[^dotnet-path]: <https://github.com/dotnet/sdk/issues/9415> and
                <https://github.com/dotnet/sdk/issues/23165>.

[^rust-command]: Rust, `library/std/src/sys/process/windows.rs`, `resolve_exe`,
                 read from the pinned toolchain. Microsoft documents that
                 `CreateProcessW` assumes no extension for an explicit
                 application name and that batch files need `cmd.exe`:
                 <https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw>.

[^batbadbut]: Rust security advisory for CVE-2024-24576,
              <https://blog.rust-lang.org/2024/04/09/cve-2024-24576/>.

[^pwsh-msix]: Microsoft, "Install PowerShell 7 on Windows",
              <https://learn.microsoft.com/powershell/scripting/install/install-powershell-on-windows>:
              `winget` installs the MSIX package by default from 7.6.0, and
              there is no MSI from 7.7.0. The pinned standard library's `stat`
              falls back to opening the reparse point itself when following it
              fails.

## Revision note

- 2026-11-25: initial draft (`fd4a2d91d`).
- 2026-11-25: revised after a four-panel community-of-experts review (structure
  and contracts; security and failure modes; verification; alternatives and
  viability), all of which returned "revise". Renamed the module to
  `trusted_shell` and kept the keys in the existing table, because the layout
  contract rejects the first draft's names. Separated construction from
  resolution, which made the failure precedence reachable. Replaced the port
  trait with a closure and an admitted-directory type. Changed inspection
  policy so that an unreadable directory is skipped and reported. Tightened
  Windows admission and bare names, defined "already extended", and recorded
  App Execution Aliases. Widened OD-1, restated OD-2 with ADR-0071's rejected
  option, and turned the localization and CI questions into decisions.
  Strengthened the lemmas to biconditionals over one event trace, added the
  representation relation, join invariant, exhaustive enumeration, and
  per-obligation controls, and corrected the BDD scenarios. Split `EP-M3`,
  fixed the gate commands, and removed all new public API except key constants.
  The remaining work is unchanged in purpose.
- 2026-11-25: applied the conditions from a second review by three of the
  panels, all of which returned "proceed with conditions". An unsafe Windows
  name now fails only its own resolution (D22) and extends to absolute
  executables. The environment value moved into the domain. The proof gained an
  outcome lemma and stronger controls, `INV-SPELLING` was added, the dead-code
  marks per plateau were completed, and residuals and expected red results were
  filled in.
