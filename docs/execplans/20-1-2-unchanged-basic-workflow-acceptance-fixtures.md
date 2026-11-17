# Add unchanged-basic-workflow acceptance fixtures (20.1.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

The expert-panel review has been applied, and the plan now awaits user
approval. Implementation must not begin until the user explicitly approves this
plan.

Branch: `20-1-2-unchanged-basic-workflow-acceptance-fixtures`

Roadmap item: `20.1.2` in `docs/roadmap-progressive-enhancement.md`.

## Purpose / big picture

Netsuke is a build tool. A user writes a YAML manifest called a `Netsukefile`.
Netsuke renders its Jinja templates and builds a dependency graph. It then
writes a temporary Ninja build file and runs the Ninja build tool on it. The
progressive-enhancement programme (roadmap phases 20 to 25) will add optional
semantic declarations: managed states, typed inputs, owned artefacts,
contention classes, and maturity policies. Its hypothesis is that those
declarations can add guarantees without increasing the prerequisites or
vocabulary of a first build.

Today that promise is asserted rather than demonstrated. The first-build
manifest appears in three documents. The tests exercising it check only the
exit status and the content of `hello.txt`. No test observes several things:

- what Netsuke actually starts;
- what Netsuke writes outside the declared output;
- whether Netsuke touches the network;
- whether opting one node into a feature leaves its neighbours untouched.

A later feature could therefore make advanced syntax "contagious" without any
test turning red.

After this work a maintainer can run one command and see the shallow end
demonstrated:

```sh
cargo nextest run --test shallow_end_acceptance_tests --test shallow_end_non_contagion_tests
```

The acceptance suite runs the exact quickstart manifest and three other small
manifests. Each run uses the real `netsuke` binary and a real Ninja. The suite
does the following:

- It records Ninja's argv, the command plan Ninja receives, and, on Linux, the
  process tree the kernel saw.
- It snapshots the default human diagnostics.
- It compares the whole observed filesystem before and after each run.
- It asserts that the run started no process beyond Ninja and the recipe
  shells.
- It asserts that the run wrote nothing outside the workspace and created no
  generated runtime state.
- It asserts that the run attempted no network connection.

A non-contagion suite enumerates every existing per-node opt-in feature on
every node of two small base manifests. For each node it opts in, it shows that
no other node needs a declaration and that no other node's lowered build edge
changes. A mixed fixture places an opted-in node and plain nodes under an
ordinary aggregate action. Building an unrelated target from it is
indistinguishable from building that target in the plain project.

Terms used throughout:

- *Shallow end*: the minimal manifest subset a newcomer needs. The
  terms-of-reference glossary lists targets, commands, and defaults
  (`docs/terms-of-reference.md`, glossary and goal G1). Roadmap item 20.1.2
  adds ordinary `vars` and the list-of-mappings `actions` and `targets`
  structure.
- *Per-node opt-in*: an optional declaration on one action or target that adds
  a semantic guarantee to it. Schema `1.0.0` offers `console`, `always`,
  `order_only_deps`, `command_isolation`, and `dependency_order`. The features
  RFCs 0021 to 0024 propose (states, typed inputs, owned artefacts, contention
  classes) will be the future opt-ins; none is implemented yet.
- *Own declarations*: the declarations a feature legitimately needs beyond
  the opted-in node itself. Examples are a schema-minor bump under ADR-0075 or
  a root registry such as `contention_classes` under ADR-0080. Own declarations
  are not contagion.
- *Contagion*: annotating one node requires a declaration on another node, or
  changes the lowered build edge or observable behaviour of a node that was not
  opted in. *Non-contagion* is its absence.
- *Declared global effects*: the global effects a feature is permitted beyond
  its own node's edge, such as a Ninja header line or ADR-0012 dyndep sidecars.
  This deliberately avoids the word "footprint", which roadmap item 20.2.2 uses
  for onboarding.
- *State record*: the persisted readiness evidence that ADR-0076 and ADR-0077
  place in a versioned runtime namespace separate from dyndep sidecars. It is
  unimplemented and its location is not fixed, so these fixtures prove its
  absence by comparing whole trees, not by checking one path.
- *Generated runtime state*: anything Netsuke itself writes for its own use.
  Today this means only the ADR-0012 dyndep sidecars under `.netsuke/dyndep` and
  `.netsuke/serial`, the fetch cache under `.netsuke/fetch`, and the temporary
  Ninja build file.
- *Probe execution*: a readiness inspection, built-in or external, as ADR-0077
  defines it. The process-closure assertion below is stricter, so a probe
  implemented either as a process or as an in-process filesystem inspection
  would surface either as an extra process or as an observed effect.
  Template-time helpers (`fetch`, `which`, the command helpers) are governed by
  their own policy (RFC 0025 section 2). The corpus does not call them.
- *Installation*: writing executables, packages, or caches outside the
  workspace's declared outputs, or invoking a package manager or downloader.
- *Network activity*: an attempt to connect or send to an Internet-family
  (`AF_INET` or `AF_INET6`) socket address, or a request sent to an HTTP proxy.

This plan changes no production behaviour. It adds frozen fixtures, an
observation harness in `test_support`, integration-test crates, one
behaviour-driven development (BDD) feature, continuous-integration (CI) tracer
steps, ADR-0084 with an amendment banner on ADR-0076, and developer
documentation. It closes roadmap item 20.1.2 and supplies the regression
fixtures that ADR-0076's shallow-end section assigns to it.

## Context and orientation

### Where the shallow end lives today

The quickstart manifest from RFC 0025 section 2 appears verbatim in three
places:

- `docs/quickstart.md`, Step 2 (lines 36 to 45). The page has no
  `tested-example` markers, and nothing loads its fences.
  - Step 3 (lines 62 to 64) shows `[1/1] echo 'Hello from Netsuke!' >
    hello.txt`. That is Ninja's own status line on standard output, and it is
    untested.
  - Step 4 (lines 86 to 98) adds ordinary `vars`.
  - Step 5 adds `foreach` over a glob, which is outside the shallow-end
    vocabulary.
- `README.md`, "Your first build" (lines 126 to 150), with markers
  `readme-first-build-manifest` and `readme-first-build-commands`.
- `docs/users-guide.md`, "Run the first build" (lines 341 to 369), with
  markers `guide-first-build-manifest` and `guide-first-build-commands`.

Existing coverage of these manifests is real but shallow:

- `tests/documentation_examples_e2e_tests.rs` (`#![cfg(unix)]`), case
  `documented_first_run_produces_the_reported_output`, runs the README and
  guide manifests with real Ninja and compares `hello.txt` to
  `Hello from Netsuke!\n`.
- `tests/documentation_examples_tests.rs` runs `generate` on them and runs
  them against a fake Ninja.
- `tests/features/documentation_examples.feature` uses a fake Ninja that
  writes `hello.txt` itself.
- `tests/novice_flow_smoke_tests.rs` asserts `Stage 6/6` and `Build complete.`
  against a fake Ninja.

None of these records the Ninja argv, the commands Ninja runs, the complete
default diagnostics, any file beyond `hello.txt`, or network activity.

### The pipeline under test

1. `src/runner/mod.rs` `run` resolves the Ninja program: `NETSUKE_NINJA`,
   otherwise `ninja` on `PATH` (`src/runner/process/ninja/program.rs`).
2. It loads the manifest through `src/manifest/mod.rs`, where
   `parse_admitted_document` applies the ADR-0075 schema gate
   (`src/manifest/schema_version/mod.rs`, which admits only `1.0.0`).
3. It lowers the manifest to the intermediate representation (IR) with
   `BuildGraph::from_manifest` in `src/ir/from_manifest/mod.rs`.
4. It generates Ninja text in `src/ninja_gen/mod.rs`.
5. It writes that text to a temporary file in the system temporary directory
   (`src/runner/process/file_io.rs`, `create_temp_ninja_file`).
6. It spawns Ninja in `src/runner/process/mod.rs` with argv
   `[-j N] -f <build-file> [-- targets]` (`src/runner/process/configure.rs`,
   `configure_ninja_build_command`).

On POSIX, Ninja runs each edge's command through `/bin/sh -c` and writes
`.ninja_log` in its build root (Ninja manual).

On Linux, `src/runner/recipe_shell/mod.rs` returns without probing. The `fetch`
Jinja helper (`src/stdlib/network/`) is the only network client in `src/`. It
uses `ureq` 3, whose default configuration reads the proxy variables
(`docs.rs/ureq`: "Proxy configuration is picked up from environment when using
`Config::default()`"), and its default policy is `https_only`. The command and
`which` helpers in `src/stdlib/` run only when a template calls them. Sidecars
under `.netsuke/` are written only when the graph needs them
(`src/runner/dyndep/publication.rs`). There is no installer, run ledger, or
state-record store.

### Per-node opt-in features that exist today

None of the RFC 0021 to 0024 features is implemented. Every row except `core`
in the schema registry is `reserved`: `docs/netsuke-design.md` lines 260 to
274, between `<!-- schema-registry:start -->` and
`<!-- schema-registry:end -->`, pinned by
`tests/schema_version_registry_tests.rs`. `NetsukeManifest` and `Target` both
use `deny_unknown_fields`, so a test cannot invent a key. The `core` feature,
schema `1.0.0`, already has these per-node opt-ins:

- `console: true` places the edge in Ninja's built-in `console` pool
  (`src/ninja_gen/display_edge.rs`). This is the closest existing analogue of
  an RFC 0024 contention class.
- `always: true` emits `restat = 1` on that edge only.
- `order_only_deps` emits an edge-local `||` list.
- `command_isolation: isolated` applies to a command list (ADR-0045). It
  changes only that action's hash and rule. Its own declarations include
  turning a scalar `command` into a list.
- `dependency_order: serial` takes effect with two or more `deps`. It is the
  one feature with global effects: the dyndep bundle path, a global
  `ninja_required_version = 1.10` header, and ADR-0012 sidecars written by the
  runner. Unrelated edges' text is unchanged.

Rule names are SHA-256 hashes of canonical action JSON (`src/hasher.rs`), and
actions are deduplicated by that hash (`src/ir/from_manifest/support/mod.rs`,
`register_action`). Rules are sorted by hash and edges by output path
(`src/ninja_gen/order.rs`). This argues that annotating node A cannot rename
node B's rule. Because actions are shared, however, a lowering defect could
change a shared action, so the fixtures test the argument with a shared-rule
base instead of assuming it. `BuildGraph`, `BuildEdge`, and `Action`
(`src/ir/graph/mod.rs`) are public and derive `PartialEq`. An integration test
reaches them through `netsuke::manifest::from_str` and
`netsuke::ir::BuildGraph::from_manifest`. `from_manifest` uses
`RecipeShell::host_default()`, so its output is host-dependent and is never
snapshotted across platforms.

### Existing harness pieces to reuse

- `test_support::netsuke::run_netsuke_in_with_env` clears the environment and
  runs the binary. `test_support::env` composes environments.
- `test_support::bounded_process::output_within` gives every child process a
  deadline, as AGENTS.md requires.
- `test_support::ninja::ninja_integration_workspace` probes real Ninja and
  honours `NETSUKE_REQUIRE_NINJA`, read through `mockable::Env`
  (`test_support/src/ninja.rs:75`).
- `test_support::exec::write_exec_with_content` writes stub executables.
- `test_support::fs` is the crate's only ambient filesystem boundary and the
  only `no_std_fs_operations` exemption (`test_support/dylint.toml`). It is 381
  lines and lacks `read_dir`, `symlink_metadata`, and `read_link`, so it must
  be split into an `fs/` directory before it can grow.
- `test_support::hash::sha256_hex` computes content digests.
- `test_support::http::spawn_http_server_expecting_no_requests` records any
  HTTP request it receives. It counts requests that send bytes, not bare
  connections.
- `test_support::temp_path_filter` normalizes temporary paths. The crate-local
  `stable_snapshot_settings` is `pub(crate)` and unavailable to integration
  tests, so the snapshot settings follow `tests/release_help/mod.rs`
  `snapshot_settings`, with an explicit `snapshot_path`.
- `test_support::ninja_edge` already checks `pool = console` bindings.
- `test_support::build_tools::sandbox` shows the allowlisted-`PATH` pattern:
  symlinks to named host utilities in a private `bin/`.
- `tests/prover_tools_kani_check_tests/trace.rs` (218 lines, tests in
  `trace/tests.rs`) reads `strace` logs and reports file and network attempts.
  It selects the `%file,%network` syscall classes. Its `attempts()` returns
  finding strings, not structured paths. `support/sandbox/probe.rs` turns a
  missing tracer into a failure under a strict variable.
  `.github/workflows/ci.yml` already installs `strace` for the Kani boundary
  lane ("Install the syscall tracer").
- `googletest` 0.14.3, `pretty_assertions` 1.4.1, `proptest` 1.11.0, `insta`,
  `assert_cmd`, `mockable`, and `rstest-bdd` 0.6.0 are development dependencies
  already.

### CI facts that shape the design

`.github/workflows/ci.yml` job `build-test` runs tests in exactly one step,
"Test and Measure Coverage" (pull requests only). That step runs
`cargo llvm-cov nextest`, so the `netsuke` child process is
coverage-instrumented. With its environment cleared, the child would write
`default_*.profraw` into the workspace. The trunk lane `coverage-upload` in
`.github/workflows/coverage-main.yml` runs the same test set.
`NETSUKE_REQUIRE_NINJA` is set in both lanes and in
`.github/workflows/ci-default-features.yml`. The settings in
`coverage-main.yml` and `ci-default-features.yml` are pinned by
`tests/workflow_contracts/runner_placement_test.py` and
`tests/workflow_contracts/ci_default_features_test.py`. The Kani lane's tracer
step is conditional on `steps.scope.outputs.run-proofs`, which belongs only to
that job. GitHub's Ubuntu 24.04 image does not list `strace`.

### Documentation placement

- `docs/netsuke-design.md` section 2.9, "Progressive-enhancement contracts",
  maps RFCs 0021 to 0025 to ADR-0075 through ADR-0081.
- `docs/developers-guide.md` has the relevant sections:
  - "Test execution", with "Required real-Ninja coverage", at line 7859;
  - "User-facing documentation examples", at line 8527, which lists the three
    loaded documents;
  - "Test isolation utilities".
- `docs/repository-layout.md` "Placement conventions" puts reusable
  integration helpers in `test_support/`.
- `docs/contents.md` indexes ADRs by hand.
- `docs/documentation-style-guide.md` governs ADR amendments: an
  "Amended by" banner and an `## Amendments` section.
- There is no translated users' guide.

### Budget survey

`docs/execplans/3-14-5-regression-coverage-for-conditional-action-dependency-manifests.md`
was a test-only regression plan. It set "roughly 900 lines net", with no
production change beyond one test seam, and its record shows no raise.

`docs/execplans/20-1-1-ratify-progressive-enhancement-contracts.md` raised its
EP-M3 ceiling once, at the user's direction on 2026-11-03, to 2,000 net Rust
lines. The cause was missed scope: a file at the 400-line cap forced a split.
This plan faces the same pattern in `test_support/src/fs.rs` and plans the
split up front.

Bottom-up estimate of net code lines, before contingency:

| Area                                                                                                  | Net lines |
| ----------------------------------------------------------------------------------------------------- | --------- |
| `fs` split, net of moved lines, plus new walk functions                                               | 80        |
| Trace-reader move, roughly net zero, plus `execve` and structured-path readers with tests             | 350       |
| `shallow_end` harness with unit tests and controls                                                    | 750       |
| Acceptance tests and control fixtures                                                                 | 500       |
| Non-contagion projection, catalogue, enumeration, property test, seeded faults, and registration test | 700       |
| BDD steps                                                                                             | 200       |
| Fixture manifests                                                                                     | 90        |
| Python workflow contracts                                                                             | 60        |
| Total                                                                                                 | 2,730     |

Table 1: Bottom-up estimate of net code lines.

## Signposts

Read these before implementing:

- `AGENTS.md`: "Testing", the environment-mutation ban and subprocess
  exemption, "Executable documentation", and the helper policy under
  "Refactoring heuristics".
- `docs/rfcs/rfc-0025-progressive-enhancement-and-maturity-policies.md`,
  sections 2, 6, 8, and 9.
- `docs/adrs/adr-0076-progressive-enhancement-contract-boundaries.md`,
  "Shallow end and persisted formats".
- `docs/adrs/adr-0077-managed-state-schema.md`, on probes and state records.
- `docs/adrs/adr-0075-manifest-schema-versions-and-admission.md`.
- `docs/adrs/adr-0080-contention-class-schema.md`.
- ADR-0012 (dyndep sidecars).
- `docs/adrs/adr-0045-isolate-command-list-entries.md`.
- `docs/adrs/adr-0008-environment-seam-taxonomy.md`.
- `docs/netsuke-design.md` section 2.9 and the schema registry, Table 2.
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/reliable-testing-in-rust-via-dependency-injection.md`.
- `docs/rust-doctest-dry-guide.md`, for the doc comments on new public
  `test_support` items. Examples that spawn processes are `no_run`.
- `docs/ortho-config-users-guide.md`, only to know which discovery variables
  to clear. This plan adds no configuration (D-10).
- `docs/developers-guide.md`, "Test execution" and "User-facing documentation
  examples".
- `docs/documentation-style-guide.md`, for the ADR template, amendments, and
  ExecPlan status rules.

Skills to load during implementation:

- `rust-router`, then `rust-unit-testing` for rstest, googletest, and insta;
- `proptest`;
- `nextest`;
- `hexagonal-architecture`, because the harness is a driving adapter around
  the binary (D-4);
- `execplans`;
- `en-gb-oxendict-style`;
- `rust-verification`, only if the verification plan changes.

## Conformance basis

Upstream artefacts at `main` revision `5c231fed1`:

- `docs/terms-of-reference.md`:
  - TOR-G1, "A shallow end": "the existing quick start is the regression
    fixture for this goal".
  - TOR-G2, "Progressive enhancement, never contagion".
- `docs/roadmap-progressive-enhancement.md` item 20.1.2, last changed at
  `19b235138`. Its sub-bullets are:
  - RM-a: exact quickstart, scalar shell recipes, ordinary variables, and the
    list-of-mappings structure.
  - RM-b: actual child commands, default diagnostics, and files; absence of
    state records, probe execution, installation, and new network activity.
  - RM-c: the mixed aggregate fixture. Success means enabling one feature does
    not require declarations on unrelated nodes.
- RFC 0025, status Proposed. ADR-0076 and ADR-0081 ratify its contracts.
  - RFC25-SE (section 2): the shallow-end contract and its quickstart block.
  - RFC25-NC (section 6): scope and non-contagion.
  - RFC25-QUIET (section 8, last paragraph): "Supported basic manifests must
    have no new maturity messages under default settings".
  - RFC25-ACC (section 9): compare "observed child arguments and filesystem
    effects"; "Assert that unrelated invocation starts no probes and creates
    no state records".
- ADR76-SE: ADR-0076 (Accepted 2026-11-09), "Shallow end and persisted
  formats". It says "its fixtures belong to roadmap item 20.1.2"; state records
  live in a namespace separate from dyndep sidecars.
- ADR77-PROBE: ADR-0077's built-in and external probe definitions.
- ADR75-ADMIT: ADR-0075; `1.0.0` manifests remain admitted with no opt-in list.
- The schema registry (REG): `docs/netsuke-design.md` Table 2.
- ADR-0012 (sidecars), ADR-0045 (`command_isolation`), and ADR-0080 (the
  contention-class schema).
- ADR-0008: environment seams and the subprocess exemption.
- Standards: `AGENTS.md`, `docs/documentation-style-guide.md`, and
  `docs/developers-guide.md`.

No task-specific technical design exists. ADR-0084 records this plan's fixture
and registration policy as an amendment to ADR-0076 (D-7).

Roadmap item 20.1.2 supplies the harness and the baseline for RM-c. The
evidence for each RFC feature arrives when that feature is admitted:

| Roadmap task | Feature             |
| ------------ | ------------------- |
| 21.1.2       | `inputs`            |
| 22.1.1       | `contention`        |
| 23.1.1       | `states`            |
| 24.1.1       | `artefacts`         |
| 25.3.1       | the combined matrix |

Table 2: Admission tasks that each register a catalogue case under ADR-0084.

Each of those tasks registers a catalogue case under ADR-0084.

Trace links:

```plaintext
TOR-G1 -> RFC25-SE -> ADR76-SE -> RM-a -> EP-M1 -> shallow_end_corpus_tests (OBL-1)
TOR-G1 -> RFC25-ACC -> RM-b -> EP-M3/EP-M4 -> shallow_end_acceptance_tests (OBL-2, OBL-4..OBL-6)
RFC25-QUIET -> RM-b (default diagnostics) -> EP-M4 -> OBL-3
ADR77-PROBE -> RM-b (probe execution) -> EP-M4 -> OBL-5
TOR-G2 -> RFC25-NC -> RM-c -> EP-M5 -> shallow_end_non_contagion_tests (OBL-7)
RFC25-ACC -> RM-c -> EP-M5 -> shallow_end_acceptance_tests::unrelated_invocation (OBL-8)
ADR76-SE + REG -> D-7 -> ADR-0084 -> EP-M5/EP-M6 -> catalogue_covers_admitted_features (OBL-9)
ADR75-ADMIT -> RM-a -> EP-M1 -> OBL-1
```

## Constraints

- Production code under `src/` must not change. If a fixture exposes a
  production defect, record it and file an issue. Then escalate before encoding
  either behaviour.
- The frozen quickstart fixture must stay byte-identical to four sources:
  - RFC 0025 section 2's block;
  - the `docs/quickstart.md` Step 2 fence;
  - the README first-build fence;
  - the users' guide first-build fence.

  Never edit a frozen fixture to make a test pass.
- No in-process environment mutation (AGENTS.md, ADR-0008). Every environment
  the harness builds applies to a child `Command` after `env_clear`. The test
  process may read `NETSUKE_REQUIRE_NINJA`, `NETSUKE_REQUIRE_STRACE`,
  `LLVM_PROFILE_FILE`, and the host `PATH` through `mockable::Env`.
- No retries or blanket skips. A missing Ninja or tracer may skip only while
  its `NETSUKE_REQUIRE_*` variable is unset. CI sets both where the trace tier
  runs.
- No new dependency. If `googletest` and `rstest` interoperate poorly, fall
  back to `verify_that!` with `googletest::Result`, or to `anyhow::ensure!`.
- No new public command-line grammar, configuration key, JSON envelope,
  `explain` command, or exit code.
- No RFC 0021 to 0025 syntax may be implemented or reserved. The schema
  registry may be read, never edited.
- Zero new dylint exemptions. All harness filesystem access goes through
  `test_support::fs`; integration-test crates use `cap_std` or `test_support`
  helpers. If an exemption proves unavoidable, it must be a narrow
  `excluded_paths` entry with a written reason, and it is an escalation trigger.
- Respect `AGENTS.md`:
  - Files stay under 400 lines.
  - Every module carries `//!` documentation and every function carries `///`
    documentation, with `no_run` examples where they spawn processes.
  - Every assertion carries a diagnostic message.
  - `make doc-coverage` stays at or above 80%.
  - Paths use `camino::Utf8Path`.
  - The Polonius and next-generation-solver rules apply to any Rust written.
- No source-compatibility shims: callers of moved test helpers change in the
  same commit.
- Completed ExecPlans, including
  `docs/execplans/20-1-1-ratify-progressive-enhancement-contracts.md`, stay
  untouched.
- `docs/users-guide.md` and `docs/quickstart.md` prose are unchanged apart
  from adding markers. Any discrepancy found in Step 3 is filed as an issue for
  20.2.2.
- Implementation begins only after the user approves this plan.

## Tolerances (exception triggers)

The comparison base is `5c231fed1`; re-base the counts if the branch is
rebased. Each count excludes this ExecPlan and every `*.snap` file.

Changes fall into three categories:

- Code: everything under `src/`, `test_support/`, and `tests/`, including
  fixture manifests, `.feature` files, and `tests/workflow_contracts/*.py`.
- Configuration: `.github/`, `Makefile`, `dylint.toml`,
  `test_support/dylint.toml`, `.config/`, `Cargo.toml`, and
  `test_support/Cargo.toml`.
- Documentation: `docs/` except `docs/execplans/`, plus `README.md` and
  `CHANGELOG.md`.

Count net lines (additions minus deletions) per category with:

```sh
net() { git diff --numstat 5c231fed1...HEAD -- "$@" ':!docs/execplans/*' ':!*.snap' \
  | awk '{a+=$1; d+=$2} END {print a-d}'; }
net src test_support tests                                    # code
net .github Makefile dylint.toml test_support/dylint.toml .config Cargo.toml test_support/Cargo.toml  # configuration
net docs README.md CHANGELOG.md                               # documentation
git diff --name-only 5c231fed1...HEAD -- ':!docs/execplans/*' ':!*.snap' | wc -l   # files
git diff --name-only --diff-filter=A 5c231fed1...HEAD -- '*.snap' | wc -l          # new snapshots
```

Escalation triggers:

- Production code: if `git diff --numstat 5c231fed1...HEAD -- src` is
  non-empty, stop and escalate.
- Files: if more than 56 non-ExecPlan, non-snapshot files change, stop and
  escalate. The planned set is about 50 files, listed under `Concrete steps`.
- Snapshots: if more than 24 new `*.snap` files are added, stop and escalate.
- Code: if code exceeds 3,200 net lines, stop and escalate. That is the
  2,730-line estimate plus 17% contingency for trace-parsing detail.
- Configuration: if configuration exceeds 90 net lines, stop and escalate.
  The estimate is two tracer steps of about 25 lines each, two environment
  lines, and about 4 `Makefile` lines.
- Documentation: if documentation exceeds 500 net lines, stop and escalate.
  The estimate is about 410 lines:
  - ADR-0084: 190;
  - the ADR-0076 amendment banner and section: 15;
  - the developers' guide: 130;
  - the design document: 20;
  - repository layout and contents: 20;
  - quickstart markers: 6;
  - roadmap ticks: 4.
- Interface: if a public `netsuke` item must change, stop and escalate. New
  public `test_support` items are expected.
- Dependencies: if a dependency must be added, stop and escalate.
- Exemptions: if a dylint exemption must be added, stop and escalate.
- Architecture: if the non-contagion property fails in a way that would need
  a change to generation, the IR, or an ADR, stop and escalate with the
  evidence. Never weaken the property.
- Iterations: if a focused test still fails after three attempts while
  production behaviour appears correct, stop and escalate.
- Sabotage: if a negative control or seeded fault does not fail for its
  intended reason, the verification is vacuous; stop and redesign it.
- Observation: if M0 shows the CI runner pools refusing `strace` attachment,
  or `ureq` ignoring the proxy variables, stop and present the options.
- Time: if any milestone takes more than six hours of active work, stop and
  escalate.
- Ambiguity: if the user's reading of "annotated" differs from D-2, stop and
  present options.

## Risks

- Risk: coverage instrumentation in CI writes `*.profraw` files from the
  child `netsuke` process into the workspace. That breaks the file-closure
  assertion in CI only. Severity: high. Likelihood: high. Mitigation: D-12. When
  `LLVM_PROFILE_FILE` is set, the harness forwards it, with its directory
  outside every observed tree, and adds that directory to the trace tier's
  declared harness allowances. M0 includes one `cargo llvm-cov nextest` run of
  a spike test to confirm.
- Risk: Ninja's `[1/1] …` status line on standard output varies with the
  Ninja release, so a snapshot that includes it could churn. Severity: medium.
  Likelihood: medium. Mitigation: M0 records the real output. The snapshot
  normalizes only Ninja's line, and Netsuke's own lines stay exact. If Ninja's
  format churns, that line becomes a semantic assertion, recorded in the
  decision log.
- Risk: `strace` may be unavailable or refused on a CI runner pool.
  Severity: medium. Likelihood: low, because the Kani lane already uses it on
  the same pools. Mitigation: copy that lane's install step without its job
  condition. The first CI run of EP-M4 shows whether the tier executes; the
  portable tier stands regardless.
- Risk: `ureq` might ignore the proxy variables in Netsuke's agent
  configuration. Severity: low. Likelihood: low. Mitigation: OBL-6's control
  proves reachability. If the tripwire sees nothing, drop that tier, record
  why, and rely on the trace tier.
- Risk: the `fetch` control could reach a real network. Severity: medium.
  Likelihood: low. Mitigation: target an RFC 6761 `.invalid` host and start
  only after the tripwire listens.
- Risk: `dependency_order: serial` sidecars appear when building an unrelated
  target. Severity: low. Likelihood: medium. Mitigation: sidecars are ADR-0012
  generated runtime state, not state records (ADR76-SE). They are a declared
  global effect of `serial` (D-5). Escalate only if sidecars appear for a
  manifest with no serial edge.
- Risk: relative paths in `strace` output need each process's working
  directory before they can be classified as inside or outside the workspace.
  Severity: medium. Likelihood: high. Mitigation: trace with `-y`, which
  decodes file descriptors to paths, and track each process's working directory
  from `clone` inheritance and `chdir`/`fchdir` calls. Classify a write by its
  absolute target. Ignore `/proc` and `/dev/*`: Netsuke itself issues a failing
  `renameat` against `/proc`, which the panel observed. If resolution stays
  ambiguous, classify the write as a violation, failing closed.
- Risk: under `-f`, Rust and Ninja spawn children with a vfork-style clone,
  so `strace` splits each `execve` into an `<unfinished ...>` line and a
  `resumed` line. A reader that keeps only the first line would count failed
  `execve` attempts. Severity: medium. Likelihood: high. Mitigation: pair the
  two lines by process ID, count only calls that return `= 0`, and attribute
  parents from `clone`/`clone3` return values.
- Risk: recipe shells legitimately read `/etc/nsswitch.conf` and `/etc/passwd`
  and may use `AF_UNIX` sockets. Severity: low. Likelihood: high. Mitigation:
  read-only attempts and `AF_UNIX` are not violations. Only Internet-family
  addresses and write-intent calls are.
- Risk: moving the trace reader breaks the Kani-check suite, which runs fully
  only in its own lane. Severity: medium. Likelihood: low. Mitigation: EP-M2 is
  a separate refactor commit. Run `make test-kani-check-isolation` locally
  under strict mode where `unshare` is available.
- Risk: real-Ninja and traced runs lengthen `make test`. Severity: low.
  Likelihood: medium. Mitigation: the fixtures are tiny, the enumeration is
  in-process, and each subprocess has a deadline.

## Progress

- [x] (2026-11-17 18:10Z) Branch renamed and Lody session titled.
- [x] (2026-11-17 18:40Z) Wyvern reconnaissance completed on six fronts:
  quickstart and examples, end-to-end harness, manifest schema and features,
  side-effect surfaces, annotation locality, and documentation placement.
- [x] (2026-11-17 19:05Z) External facts resolved with Firecrawl:
  - the Ninja manual: `/bin/sh -c`, the `.ninja_log` location, and
    `-t commands`;
  - `ureq` proxy defaults;
  - GitHub's Ubuntu 24.04 image contents.
- [x] (2026-11-17 19:10Z) ADR number swept across all 79 remote branches.
  ADR-0082 and ADR-0083 are held by open branches, so this plan claims ADR-0084.
- [x] (2026-11-17 19:40Z) Initial draft written.
- [x] (2026-11-17 20:30Z) Expert panel reviewed the draft on four lenses:
  contract conformance, test architecture, Linux tracing and CI, and repository
  conventions. Revision applied; see `Revision note`.
- [ ] Plan approved by the user.
- [ ] `EP-M0` Observation spike (prototype; no product commits).
- [ ] `EP-M1` Frozen shallow-end corpus and documentation provenance.
- [ ] `EP-M2` Split `test_support::fs` and promote the syscall-trace reader
  (refactors).
- [ ] `EP-M3` Shallow-end observation harness.
- [ ] `EP-M4` Unchanged-basic-workflow acceptance suite, BDD, and CI tracer.
- [ ] `EP-M5` Non-contagion enumeration, mixed aggregate, and registration
  contract.
- [ ] `EP-M6` ADR-0084, documentation, and roadmap closure.

## Surprises & discoveries

- Observation: `docs/quickstart.md` is not loaded by the documentation-example
  harness, so its manifests could drift from the README and the users' guide
  unnoticed. Evidence: the `DOCUMENTS` list in
  `tests/documentation_examples/mod.rs`. Impact: EP-M1 loads it as `MarkedOnly`.
- Observation: quickstart Step 3 is stale. It shows
  `[1/1] echo 'Hello from Netsuke!' > hello.txt`, but a real run prints
  `[1/1] Building ⁨hello.txt⁩` on standard output. The name is wrapped in Unicode
  isolation marks, and the raw command line does not appear. Tests asserting
  `Stage 6/6` use a fake Ninja, so they never contradicted Step 3. Evidence:
  the Linux and CI panel member ran an existing debug `netsuke` binary in
  scratch space. The same run showed argv `-f <TMPDIR>/netsuke.XXXXXX.ninja`,
  with no `-j` and no `--`, and a build file that honours `TMPDIR`. Impact: the
  snapshot normalizer must keep the isolation marks byte-exact. M0 confirms the
  result on the branch's own build. The documentation mismatch is filed as an
  issue for 20.2.2 and not edited here.
- Observation: no RFC 0021 to 0025 feature exists, so non-contagion can be
  exercised today only on the `core` feature's per-node opt-ins. Evidence:
  `deny_unknown_fields` on `Target`; the schema registry rows are `reserved`.
  Impact: see decisions D-2 and D-7.
- Observation: ADR-0080 line 90 states that "No current surface requests the
  console pool", but `console: true` exists. Evidence:
  `src/ast/target.rs:55-59`. Impact: noted for the contention feature's owner.
  A future node that declares both `console` and a contention class is a
  per-node incompatibility, not contagion. ADR-0084 says so; this plan does not
  edit ADR-0080.

## Decision log

- D-1. Decision: freeze the corpus as committed files under
  `tests/data/shallow_end/`, one directory per fixture:
  - `quickstart-hello` is Step 2, the RFC 0025 block.
  - `quickstart-vars` is Step 4, which uses ordinary variables.
  - `three-command` is RFC 0025 section 9's "small three-command project",
    with `vars`, list-of-mappings `targets` and `actions`, an aggregate
    action, and `defaults`.
  - `mixed-aggregate` is `three-command` plus one per-node opt-in.

  Provenance tests compare the quickstart fixtures byte for byte with their
  fences. Step 5's `foreach` manifest is excluded: `foreach` and globbing are
  outside the shallow-end vocabulary (TOR glossary), and freezing them would
  widen the release contract. Rationale: deriving fixtures from the
  documentation at test time would let a documentation rewrite silently become
  the new baseline. A frozen file plus equality makes any drift fail loudly and
  forces an explicit compatibility decision. Date/Author: 2026-11-17, planning
  agent, revised after panel review.
- D-2. Decision: today "annotated" means a node opted into one of the `core`
  feature's existing per-node opt-ins: `console`, `always`, `order_only_deps`,
  `command_isolation: isolated`, or `dependency_order: serial`.
  - The end-to-end mixed fixture uses `console: true`, the existing pool
    assignment and the nearest analogue of an RFC 0024 contention class.
  - Target `description` is excluded because it has no edge effect, so a
    locality check over it would be vacuous.
  - 20.1.2 delivers RM-c as the harness, the catalogue, the baseline, and
    `core` evidence. Each RFC feature's own evidence arrives with its
    admission task (Table 2 under `Conformance basis`), enforced by OBL-9.

  Rationale: no RFC 0021 to 0024 syntax exists, and `deny_unknown_fields`
  forbids test-only keys. The success condition, "enabling one feature does not
  require declarations on unrelated nodes", is checkable now against real
  features. Open to user override. Date/Author: 2026-11-17, planning agent,
  revised after panel review.
- D-3. Decision: observe child commands in two tiers, run separately.
  - Portable tier (every Unix host): `NETSUKE_NINJA` points at a recording
    shim. Using absolute paths for its own tools, the shim:
    1. writes Netsuke's Ninja argv with `printf '%s\0'`, plus its working
       directory and the witness environment variable;
    2. copies the `-f` build file to a log directory outside every observed
       tree;
    3. records `ninja -f <copy> -t commands` with the same targets, inserting
       `-t commands` before any `--` and sending both output streams to the
       log, never to Ninja's standard output, which Netsuke parses;
    4. `exec`s the real Ninja with an unchanged argv.

    `exec` preserves argv after the program name and the working directory.
    The shell may add `PWD` and `SHLVL`, so the environment is checked only
    through the witness variable. With no targets, `-t commands` prints the
    default targets' closure in depth-first order. It is compared as a set
    of command strings; execution order belongs to the trace tier.
  - Linux trace tier: a separate run with `NETSUKE_NINJA` set to the real
    Ninja's absolute path and no shim. The tracer wraps the `netsuke` binary
    itself, by absolute path, because `strace` would otherwise search the
    child's `PATH`. The command is:

    ```sh
    strace -f -qq -y -s 65536 -e signal=none \
      -e trace=%file,%network,fchdir,clone,clone3,fork,vfork \
      -o <log> <abs-netsuke> <args>
    ```

    - `%file` includes `execve` and `chdir`.
    - `-s 65536` prevents the default 32-character truncation of strings
      and argv arrays.
    - `-y` decodes file descriptors to paths.
    - `-e signal=none` suppresses `SIGCHLD` lines.
    - `-v` is deliberately not used, because it would write every process's
      environment into the log.

    Every `execve`, including each `/bin/sh -c` command Ninja runs, comes
    from the kernel's record.

  Rationale: the shim cannot see absolute-path execs, and a shim inside the
  traced tree would pollute the process set. Splitting the runs keeps each tier
  honest. Date/Author: 2026-11-17, planning agent, revised after panel review.
- D-4. Decision: put the reusable harness in `test_support::shallow_end`. Its
  submodules are `recording_ninja`, `tree`, `tripwire`, `trace_run`
  (Linux-only), and `run`.
  - Promote the strace reader to `test_support::syscall_trace` instead of
    writing a second one.
  - Split `test_support::fs` into `fs/` before adding the walk primitives.
  - Reuse `bounded_process`, `env`, `exec`, `hash`, `http`, `ninja`,
    `ninja_edge`, and `temp_path_filter`.
  - Reject `build_tools::sandbox` and `check_ninja` as bases. The first is
    scoped to `make` and `scripts/`; the second fakes Ninja instead of
    recording a real one. Their patterns (an allowlisted `bin/`, stubs via
    `exec`) are reused.

  The adapters report observations; policy (permitted paths, processes, and
  effects) stays in the test crates and the catalogue. Rationale: AGENTS.md's
  helper policy, and hexagonal separation of adapter from policy. Date/Author:
  2026-11-17, planning agent, revised after panel review.
- D-5. Decision: each catalogue case declares its own declarations and its
  declared global effects, as an explicit set of Ninja header lines and
  generated runtime state paths. In-process, the property asserts five things:
  1. every unannotated edge is equal to the baseline;
  2. the key set and `default_targets` are equal;
  3. the opted-in edge differs (the witness);
  4. every generated Ninja line outside the opted-in edge and its rule is
     byte-equal, except for declared header lines;
  5. the observed global delta is a subset of the declared global effects.

  End to end, generated runtime state paths must be a subset of the declared
  effects. Rationale: `serial` legitimately has global effects. Asserting that
  nothing global changes would be false, and checking nothing would hide
  contagion. Date/Author: 2026-11-17, planning agent, revised after panel
  review.
- D-6. Decision: define "default diagnostics" as a run in which:
  - the CLI receives no flags;
  - there is no configuration file;
  - standard output and standard error are piped;
  - `NETSUKE_LOCALE=en-GB` is set in the child environment.

  The locale variable is a deliberate, recorded deviation from a pure default.
  English assertions otherwise fail on non-English hosts, and the variable
  keeps the argv default where `--locale` would not. M0 also captures a
  pseudo-terminal run for the record, which is not snapshotted. Date/Author:
  2026-11-17, planning agent, revised after panel review.
- D-7. Decision: record the fixture policy in ADR-0084, "Shallow-end
  acceptance fixtures and non-contagion registration", as an amendment to
  ADR-0076's shallow-end section. ADR-0076 gains an "Amended by ADR-0084"
  banner and an `## Amendments` entry. ADR-0084 states three rules:
  - The corpus is frozen.
  - Every schema-registry feature whose status becomes `admitted` or
    `released` must have a catalogue case, or an explicit exemption with a
    reason, plus an end-to-end unrelated-invocation case.
  - Removing or weakening a case needs an explicit compatibility decision.

  OBL-9 enforces the registration rule by parsing the registry. CI wiring and
  tier mechanics live in the developers' guide, not in the ADR. Rationale: this
  binds phases 21 to 25 and extends an accepted ADR. The style guide records
  such extensions as amendments. Date/Author: 2026-11-17, planning agent,
  revised after panel review.
- D-8. Decision: execution tests are `#[cfg(unix)]`. The in-process
  projection, enumeration, and registration tests run on every platform but
  never snapshot host-dependent recipe text. Rationale: the quickstart recipe
  is a POSIX shell string, and Windows recipe-shell resolution is a separate,
  documented behaviour. Date/Author: 2026-11-17, planning agent.
- D-9. Decision: make the Linux trace tier mandatory where the full test set
  runs in CI. `build-test` in `ci.yml` and `coverage-upload` in
  `coverage-main.yml` each gain three changes:
  - a copy of the Kani lane's tracer step, without that job's `if:`
    condition and gated like the job's test step;
  - `NETSUKE_REQUIRE_STRACE: '1'`;
  - Python contracts in `tests/workflow_contracts/` that pin the variable and
    the step order, following `runner_placement_test.py`.

  `ci-default-features.yml` leaves the variable unset, so the trace tier skips
  there visibly; its job does not run the coverage set. Rationale: a tier that
  silently skips in CI proves nothing. Date/Author: 2026-11-17, planning agent,
  revised after panel review.
- D-10. Decision: no configuration surface is added. The harness clears every
  configuration discovery variable: `NETSUKE_CONFIG`, `HOME`, `USERPROFILE`,
  `XDG_CONFIG_HOME`, `XDG_CONFIG_DIRS`, `APPDATA`, and `LOCALAPPDATA`. It points
  `HOME` and the XDG base directories at observed empty directories and writes
  no `.netsuke.toml`. OrthoConfig gains no key. Rationale: the shallow end must
  work with no configuration. Date/Author: 2026-11-17, planning agent.
- D-11. Decision: neither Verus nor Kani is warranted; see `Verification plan`.
  Date/Author: 2026-11-17, planning agent, confirmed by the panel.
- D-12. Decision: forward `LLVM_PROFILE_FILE` to the child when the test
  process has it. Its directory is listed as a declared harness allowance,
  outside every observed tree. Do not filter `*.profraw` by name. Rationale:
  coverage must still be collected from the child, and filtering by file name
  would also hide a real stray write that happened to match. Date/Author:
  2026-11-17, planning agent.
- D-13. Decision: the process closure for the corpus is defined by parent:
  - Netsuke's direct children: exactly one Ninja.
  - Ninja's direct children: exactly one `/bin/sh -c <declared command>` per
    edge Ninja runs, with byte-equal command strings.
  - Descendants of a recipe shell: recipe work, which is recorded and
    snapshotted (for example `tr`) but not counted as probes.

  The `bin/` directory on `PATH` holds the shim (portable tier only) and
  symlinks to an allowlist of host utilities. Each fixture declares its
  allowlist; for example, `three-command` declares `tr`. Rationale: recipes
  legitimately start their own processes, and the corpus's question is what
  Netsuke and Ninja add. Shell builtins (`echo`, `true`, `printf`) never
  `execve`, so controls must use real executables. Date/Author: 2026-11-17,
  planning agent, from panel review.
- D-14. Decision: verify non-contagion in two layers.
  - An exhaustive, deterministic enumeration: every assignment of at most one
    applicable feature per node, over two bases. That is at most about 6^4
    assignments per base, a few seconds in-process.
  - A `proptest` property over multi-feature subsets, run by a manual
    `TestRunner` seeded with `TestRng::deterministic_rng` (ChaCha) over
    `sample::subsequence` of `(node, feature)` pairs. Size and feature
    classification buckets are recorded during the run. Coverage is asserted
    after `runner.run`, because proptest has no built-in coverage primitive.

  `annotate` edits the YAML tree and reloads it through
  `netsuke::manifest::from_str`, so the loader judges validity. Rationale: the
  finite space is better covered exhaustively, and the property covers
  combinations the enumeration does not. Date/Author: 2026-11-17, planning
  agent, from panel review.

## Outcomes & retrospective

Not started. On completion, record the evidence achieved, the observation tiers
retained or dropped, any production defects found, and lessons for 20.2.2.

## Verification plan

This work adds observation of an existing binary, not production logic. Every
obligation below is a falsifiable claim about the shipped `netsuke` and its
documentation. No new invariant is introduced into `src/`.

### Obligations

OBL-1, fixture fidelity and admission (RM-a, ADR75-ADMIT).
`tests/data/shallow_end/quickstart-hello/Netsukefile` equals four sources byte
for byte:

- the single YAML fence in RFC 0025 section 2;
- the Step 2 fence in `docs/quickstart.md`, marker `quickstart-first-manifest`;
- `readme-first-build-manifest`;
- `guide-first-build-manifest`.

`quickstart-vars` equals the Step 4 fence, marker `quickstart-vars-manifest`.
`mixed-aggregate` differs from `three-command` in exactly one line. Every
corpus manifest declares `netsuke_version: "1.0.0"` and loads with no other
declaration.

- Method: named `rstest` cases with `pretty_assertions` diffs in
  `tests/shallow_end_corpus_tests.rs`. Each quickstart marker is also added to
  `documented_manifest_generates_ninja`, so it is validated in its own domain.
- Command: `cargo nextest run --test shallow_end_corpus_tests`.
- Non-vacuity: the red run precedes the fixture files and fails on the missing
  file. A seeded one-byte change to a scratch copy must fail with a diff,
  checked by hand and recorded. The RFC reader must find exactly one fence.

OBL-2, child-command fidelity (RM-b). For each corpus fixture under the default
invocation:

- The portable tier records exactly one Ninja invocation. Its argv matches the
  shape M0 records (`-f [BUILD_FILE]`, plus `-j` or targets only if observed),
  and its working directory is the workspace. The witness variable reaches
  Ninja unchanged (RFC 0025 section 2: ambient environment retained). Ninja's
  planned commands equal the expected scalar strings byte for byte after
  interpolation; for `quickstart-hello` that is
  `echo 'Hello from Netsuke!' > hello.txt`.
- The trace tier's process tree satisfies D-13 with byte-equal
  `/bin/sh -c` strings.
- Each declared output's bytes equal the expected content.
- Method: `tests/shallow_end_acceptance_tests.rs`, combining an `insta`
  snapshot of the normalized observation with semantic assertions.
- Non-vacuity: `three-command` has three distinct commands, an interpolated
  `{{ ins }}`/`{{ outs }}` pair, and a real external program, `tr`. The trace
  must show `tr` as a recipe descendant. A recorder that kept only the first
  command, the uninterpolated text, or only direct children fails.

OBL-3, quiet default diagnostics (RM-b, RFC25-QUIET). Under D-6, every
standard-error line matches an allowlisted en-GB catalogue template for the
stage, status, and completion messages, rendered from
`locales/en-GB/messages.ftl`. Timing values are normalized. No other line
appears. Standard output equals the snapshot, with only Ninja's version-shaped
line normalized.

- Method: an allowlist matcher plus `insta` snapshots, with the
  `snapshot_path` set explicitly to `tests/shallow_end_acceptance/snapshots`.
- Non-vacuity: the matcher must reject a standard-error capture that contains
  one extra warning line. The snapshot catches change; the allowlist catches
  content.

OBL-4, filesystem effect closure (RM-b: files, state records, installation).
Before and after each run, the harness captures a tree snapshot (path, kind, and
`sha256_hex` of content; symlinks recorded, not followed). The tree covers the
workspace and the harness-supplied `HOME`, XDG configuration, cache, data, and
state directories, and `TMPDIR`.

- The workspace delta equals the declared outputs plus `.ninja_log`, a Ninja
  axiom (A-2).
- No generated runtime state appears.
- Every other observed tree is unchanged, which shows the temporary build file
  was removed.
- On the trace tier, no write-intent `open`, `creat`, `mkdir`, `rename`,
  `link`, or `unlink` resolves outside the workspace, `TMPDIR`, and the
  declared harness allowances. That catches writes that were later deleted.
  Ambiguous paths fail closed.
- Non-vacuity: three control fixtures under
  `tests/shallow_end_acceptance/controls/`. A recipe that also writes
  `.netsuke/state/x` must be reported as generated runtime state. A recipe that
  writes `$HOME/.cache/x` must be reported by both tiers. A recipe that creates
  and deletes `$HOME/y` must be reported by the trace tier only, which shows
  why that tier exists.

OBL-5, no probe or installation execution (RM-b, ADR77-PROBE). The process
closure in D-13 holds on the trace tier. On the portable tier, the shim runs
exactly once, and `PATH` holds only `bin/`, so a name-based probe of a
non-allowlisted tool cannot succeed. OBL-4 covers installation writes.

- Non-vacuity: a control manifest calling the template `shell` helper with an
  allowlisted program must produce an extra `execve` whose parent is Netsuke,
  not Ninja, reported as a closure violation.

OBL-6, no new network activity (RM-b). Both tiers set `HTTP_PROXY`,
`HTTPS_PROXY`, `ALL_PROXY`, and their lowercase forms to a local recording
listener, with `NO_PROXY` empty. The listener must record zero proxy requests.
The field is named `proxy_requests`: it counts requests, and the
connection-level claim belongs to the trace tier. On the trace tier, no
`connect`, `sendto`, `sendmsg`, or `sendmmsg` may target an `AF_INET` or
`AF_INET6` address.

- Non-vacuity: a control manifest renders
  `{{ fetch('https://shallow-end-tripwire.invalid/') }}` inside a target's
  `command` field. A `fetch` in an unused `vars` entry is never rendered, so it
  would test nothing. The tripwire answers `403`; a `200` would start a TLS
  handshake. The listener must record a request beginning
  `CONNECT shallow-end-tripwire.invalid:443`, and Netsuke must exit non-zero.
  The trace must show an `AF_INET` `connect` to the listener's port. The panel
  observed exactly this sequence in a scratch experiment.

OBL-7, non-contagion (RM-c, RFC25-NC, TOR-G2). Let `P(m)` map each edge's
explicit outputs to the whole `BuildEdge`, with `action_id` replaced by the
resolved `Action`. Both types derive `PartialEq`, so future fields join the
comparison automatically. The comparison also covers the key set and
`default_targets`.

There are two bases:

- `three-command`;
- `shared-rule`, an in-process base in which two targets use one `rules:`
  entry and two other targets have identical inline recipes, so they share an
  action by hash.

For every assignment `A` produced by D-14, with `a = annotate(base, A)`:

1. `a` loads and lowers. Only each case's own declarations are added.
2. For every node not in `A`, `P(a)[n] == P(base)[n]`.
3. For every node in `A`, `P(a)[n] != P(base)[n]`, the witness.
4. The Ninja text from `ninja_gen::generate`, or from the bundle generator
   for `serial`, is byte-equal outside the opted-in edges and rules, apart from
   declared header lines.

- Method: an exhaustive `rstest`-driven enumeration plus the manual
  `TestRunner` property in `tests/shallow_end_non_contagion_tests.rs`.
- Command: `cargo nextest run --test shallow_end_non_contagion_tests`.
- Non-vacuity, in four parts:
  - Condition 3 shows every case is live.
  - After `runner.run`, every feature and every subset-size bucket must have
    been reached.
  - A seeded fault in a `seeded_faults` module mutates the annotated
    `BuildGraph` by setting `restat` on a neighbour's shared action. The
    comparator must reject it, and the failing value must name the neighbour.
    This is checked through `TestError::Fail`.
  - A `serial` case that declares no global effects must fail condition 4.

OBL-8, unrelated invocation (RM-c, RFC25-ACC). For each catalogue feature,
annotate `shout.txt` in `three-command` and run `netsuke build report.txt` end
to end. The observation must equal the same invocation on plain `three-command`
in the following compared fields:

- Ninja argv, normalized;
- planned commands;
- standard output and standard error;
- the workspace delta's paths and kinds;
- the process closure;
- proxy requests and network attempts.

`.ninja_log` is compared by presence only. Build-file text is not compared,
because it legitimately contains the opted-in edge. Generated runtime state
must be a subset of the feature's declared global effects. `shout.txt` must be
absent afterwards, which shows the run avoided the opted-in node.

Running the aggregate `all` on `mixed-aggregate` must succeed and produce every
declared output. The recorded build file must bind `pool = console` on
`shout.txt` only, checked with `test_support::ninja_edge`. That proves the
fixture is genuinely mixed.

OBL-9, registration (D-7, REG). `catalogue_covers_admitted_features` parses the
schema registry between its markers. Every row whose status is `admitted` or
`released` must have at least one catalogue case or an exemption with a reason.
Every catalogue case must name a registered feature.

- Non-vacuity: the `core` row is `released` today, so the check is live. A
  second case over a synthetic table containing an `admitted` row with no case
  must fail.

### Axioms

- A-1: Ninja runs each command with `/bin/sh -c` on POSIX (Ninja manual).
- A-2: Ninja writes `.ninja_log` in its build root unless `builddir` is set.
  Netsuke sets no `builddir`; M0 confirms it.
- A-3: `ninja -t commands <targets>` prints the commands that would rebuild
  those targets, assuming all outputs are out of date. Before dyndep loading it
  can omit dyndep-discovered work, so the trace tier, not the shim, is
  authoritative for `serial`.
- A-4: `strace -f` records every `execve` and every traced socket and file
  call of the traced process tree. The `%file` and `%network` classes are
  architecture-independent, and the existing Kani boundary test relies on this.
- A-5: `ureq` 3's default configuration reads proxy variables (docs.rs). OBL-6
  checks this rather than trusting it.
- A-6: RFC 6761 reserves `.invalid`, so it never resolves.
- A-7: shell builtins do not `execve`.

### Where formal methods are, and are not, warranted

Verus is not warranted. This task introduces no contractual business logic and
no lemma over a pure kernel. The non-contagion relation is a property of the
whole lowering and generation pipeline. A Verus proof would need a model of
that pipeline, which does not exist and is not a small, stable pure kernel, and
it would not establish the shipped behaviour anyway. Kani is not warranted
either: no bounded pure transition function or `unsafe` block is introduced.

The informal argument that hash-keyed rules isolate nodes is tested by the
`shared-rule` base and the seeded fault rather than assumed. If a later feature
introduces a pure locality kernel, such as an RFC 0024 class resolver, its own
plan may choose Verus. ADR-0084 records that the catalogue case remains
required alongside any proof.

## Plan of work

Each milestone below states its end state, acceptance, and recovery. The
`Milestones and plateaus` section adds traceability, conformance checks,
compatibility, and remaining gaps.

### EP-M0: Observation spike (prototype)

This milestone is a prototype, and nothing from it is committed except plan
updates. Work in `/tmp/shallow-end-spike-20-1-2/`, which is scratch space and
never a build target. Build once with `cargo build --bin netsuke`, then run the
quickstart fixture by hand in five ways:

1. With a handwritten recording shim.
2. Under the D-3 `strace` command with the real Ninja.
3. With a proxy tripwire and the `fetch` control.
4. With D-6 defaults, both piped and under a pseudo-terminal
   (`script -qec`).
5. Once under `cargo llvm-cov nextest` through a throwaway test, to observe
   `*.profraw` placement.

Also run the `serial` variant with `build report.txt`.

The Linux and CI panel member has already answered three of these questions in
scratch space, using a sibling worktree's debug binary (see
`Surprises & discoveries`):

- the quickstart's Ninja argv is `-f <TMPDIR>/netsuke.XXXXXX.ninja`, with no
  `-j` and no `--`;
- standard output is `[1/1] Building ⁨hello.txt⁩`;
- the proxy tripwire receives the `fetch` control's `CONNECT`.

M0 re-confirms all three on this branch's build.

Record the transcripts under `Artefacts and notes` and settle these questions:

- the exact argv, including whether `-j` or targets appear;
- the exact standard output and standard error;
- the workspace delta, including whether `.ninja_deps` appears;
- whether the tripwire sees `fetch`;
- whether the trace shows expected `AF_UNIX` or `getpwuid` noise;
- where the profile files land;
- whether `serial` sidecars appear for an unrelated target.

End state: every question answered with evidence. At the decision gate, each
tier is kept, adjusted, or dropped and recorded in the decision log, escalating
if a tolerance trips. Recovery: delete the scratch directory.

### EP-M1: Frozen shallow-end corpus and documentation provenance

1. Red: add `tests/shallow_end_corpus_tests.rs` with the OBL-1 cases.
2. Add `("docs/quickstart.md", FencePolicy::MarkedOnly)` to `DOCUMENTS` in
   `tests/documentation_examples/mod.rs` and update that module's `//!`
   documentation.
3. Mark the Step 2 and Step 4 fences with `tested-example` markers
   (`quickstart-first-manifest` and `quickstart-vars-manifest`).
4. Add both identifiers to `EXPECTED_EXAMPLE_IDS` and to
   `documented_manifest_generates_ninja` in
   `tests/documentation_examples_tests.rs`.
5. Run `cargo nextest run --test shallow_end_corpus_tests` and confirm the
   failure names the missing fixture files.
6. Green: create the fixtures under `tests/data/shallow_end/` (D-1), copied
   byte for byte. The `three-command` fixture is:

   ```yaml
   netsuke_version: "1.0.0"

   vars:
     greeting: "Hello"

   targets:
     - name: greeting.txt
       command: "echo '{{ greeting }}' > greeting.txt"
     - name: shout.txt
       sources: greeting.txt
       command: "tr 'a-z' 'A-Z' < {{ ins }} > {{ outs }}"
     - name: report.txt
       command: "echo 'report' > report.txt"

   actions:
     - name: all
       deps:
         - shout.txt
         - report.txt
       command: "echo 'all built'"

   defaults:
     - all
   ```

   `mixed-aggregate` adds one line, `console: true`, to `shout.txt`.
7. Add `shallow_end_corpus_tests` to the `make test-documentation-contracts`
   filter (`Makefile` line 331) and run `mbake validate Makefile`.

End state: the corpus is frozen and its provenance is enforced.

Acceptance:

- `cargo nextest run --test shallow_end_corpus_tests` passes.
- The seeded fault is recorded.
- These gates pass, run sequentially: `make check-fmt`, `make lint`,
  `make test`, `make doc-coverage`, `make test-documentation-contracts`,
  `make markdownlint`, and `make test-workflow-contracts`. The last two are
  required by AGENTS.md "Executable documentation".

Recovery: revert the commit.

### EP-M2: Split `test_support::fs` and promote the syscall-trace reader (refactors)

This milestone is three commits:

1. A behaviour-preserving refactor moves `test_support/src/fs.rs` to
   `test_support/src/fs/mod.rs` and adds `fs/walk.rs` with `read_dir`,
   `symlink_metadata`, and `read_link` wrappers, each with a unit test. The
   `test_support::fs` exemption covers child modules unchanged; confirm it with
   `make lint`.
2. A second refactor moves
   `tests/prover_tools_kani_check_tests/trace.rs` and `trace/tests.rs` to
   `test_support/src/syscall_trace/{mod.rs,tests.rs}`. Filesystem access goes
   through `test_support::fs`, and `TRACE_SYSCALLS` stays behind
   `#[cfg(target_os = "linux")]`. The Kani-check crate's imports are updated in
   the same commit, with no re-export left behind.
3. A red-green extension adds three readers:
   - `execve` records with pid, parent pid, and full argv. The reader pairs
     each `<unfinished ...>` line with its `resumed` line by pid, keeps only
     calls that return `= 0`, and takes parent pids from `clone`/`clone3`
     return values;
   - structured attempt records with the syscall, the resolved path or socket
     family and address, and the pid;
   - per-process working-directory tracking from `chdir` and `fchdir`, with
     `-y` descriptor decoding.

   Its unit tests use captured trace lines, including `<unfinished ...>` and
   `resumed` splits under `-f`, escaped strings, and an ambiguous relative path
   that must fail closed.

End state: one shared, structured trace reader, and an `fs` boundary that
supports walking trees.

Acceptance:

- `cargo nextest run -p test_support syscall_trace` passes.
- `cargo nextest run -p test_support fs` passes.
- `cargo nextest run --test prover_tools_kani_check_tests` passes.
- `make test-kani-check-isolation` passes where `unshare` and `strace` exist;
  record the host result.
- The full gates pass.

Recovery: revert commits independently, in reverse order.

### EP-M3: Shallow-end observation harness

Add `test_support/src/shallow_end/`. The `//!` documentation in `mod.rs` states
its scope and reuse policy: it serves only integration tests that must observe
a real `netsuke` run's processes, files, and network. Ordinary CLI tests keep
using `test_support::netsuke`. The submodules are:

- `recording_ninja.rs`:
  - `RecordingNinja::install(bin: &Utf8Path, log: &Utf8Path, real_ninja: &Utf8Path)`
    writes the D-3 shim through `exec::write_exec_with_content`.
  - `invocations()` returns `NinjaInvocation { argv, cwd, witness,
    build_file_text, planned_commands }`.
- `tree.rs`: `TreeSnapshot::capture(&Utf8Path)` through `fs::walk`, and
  `TreeDelta::between`.
- `tripwire.rs`: `ProxyTripwire::start()` over
  `http::spawn_http_server_expecting_no_requests`. It provides `proxy_env()`,
  which returns all six variables, and `proxy_requests()`.
- `trace_run.rs`, under `#[cfg(target_os = "linux")]`:
  - It wraps a `Command` in the D-3 `strace` invocation and returns
    `TraceFindings`, which hold the process tree and the attempts.
  - `tracer_available(env: &impl mockable::Env)` probes with
    `strace -f -o /dev/null /bin/true` under a deadline. `strace -V` would
    prove only that the tracer is installed, not that it can attach. The
    function honours `NETSUKE_REQUIRE_STRACE`, as `ninja.rs` honours
    `NETSUKE_REQUIRE_NINJA`.
- `run.rs`:
  - `ShallowEndWorkspace::from_fixture(name, utilities: &[&str])` copies a
    corpus directory into a fresh temporary root. Beside the copy it creates
    `home/`, `xdg/{config,cache,data,state}/`, `tmp/`, `log/`, and `bin/`.
    `bin/` holds symlinks to the declared host utilities.
  - `run_portable(args)` and `run_traced(args)` build the environment from
    `test_support::env` after `env_clear`, then run the binary under
    `bounded_process::output_within`. The environment is:
    - `PATH` set to `bin/`;
    - `NETSUKE_NINJA`;
    - `HOME` and the four XDG variables;
    - `TMPDIR`;
    - `NETSUKE_LOCALE=en-GB`;
    - the proxy variables;
    - `SHALLOW_END_WITNESS`;
    - `LLVM_PROFILE_FILE`, forwarded when present (D-12).
  - The result is `ShallowEndObservation { status, stdout, stderr, ninja,
    delta, proxy_requests, trace }`.

Each submodule gets red-green unit tests, including its negative controls:

- the shim records two invocations when called twice;
- the tree delta reports an added, removed, and modified file and a symlink;
- the tripwire counts a real request;
- the trace adapter reports a synthetic `AF_INET` connect;
- a missing tracer fails under `NETSUKE_REQUIRE_STRACE=1`, tested with
  `mockable::MockEnv`.

Register the new modules in `test_support/src/lib.rs` and its crate
documentation.

End state: a working harness with no policy inside it.

Acceptance:

- `cargo nextest run -p test_support shallow_end` passes.
- Every control fails for its intended reason, recorded in
  `Artefacts and notes`.
- The full gates pass.

Recovery: revert the commit.

### EP-M4: Unchanged-basic-workflow acceptance suite, BDD, and CI tracer

1. Red: write `tests/shallow_end_acceptance_tests.rs` (`#![cfg(unix)]`) with
   support modules under `tests/shallow_end_acceptance/`. Add one `rstest` case
   per corpus fixture per tier for OBL-2 to OBL-6, and the OBL-4 to OBL-6
   control cases under `tests/shallow_end_acceptance/controls/`. Run them and
   confirm the expected failure: missing snapshots, or the allowlist matcher
   not yet written.
2. Green: implement the allowlist matcher and the normalizers, then review
   and accept the snapshots.
3. Add `tests/features_unix/shallow_end.feature` (below) and
   `tests/bdd/steps/shallow_end.rs`. The steps delegate to
   `test_support::shallow_end`, and existing equivalent steps are reused after
   a check of `tests/bdd/steps/`. Register the steps in
   `tests/bdd/steps/mod.rs`.
4. Wire CI per D-9: the tracer step and `NETSUKE_REQUIRE_STRACE: '1'` in
   `ci.yml` `build-test` and in `coverage-main.yml` `coverage-upload`, placed
   before each job's test step under the same event condition. Add Python
   contracts that pin both.

End state: RM-a and RM-b demonstrated on every Unix host, with kernel-level
evidence in CI.

Acceptance:

- `NETSUKE_REQUIRE_NINJA=1 NETSUKE_REQUIRE_STRACE=1 cargo nextest run --test shallow_end_acceptance_tests`
  passes.
- `cargo nextest run --test bdd_tests` passes.
- `make test-workflow-contracts` passes.
- The full gates pass.
- The first CI run shows the trace tier executing on both jobs.

Recovery: revert the commit. The CI steps share a commit with their strict
consumer, so they cannot drift apart.

### EP-M5: Non-contagion enumeration, mixed aggregate, and registration contract

1. Red: write `tests/shallow_end_non_contagion_tests.rs`, which runs on every
   platform and in-process. Put its support under
   `tests/shallow_end_non_contagion/`:
   - `projection.rs` holds `project`, which builds the whole-edge projection
     with `Action` resolved.
   - `bases.rs` holds `three-command` and the in-process `shared-rule` base.
   - `catalogue.rs` holds `FeatureCase { feature_id, case_id, applies_to,
     annotate, own_declarations, declared_global_effects }`, keyed by
     registry feature identifier.
   - `seeded_faults.rs` holds the fault entries.

   Add the enumeration, the property (D-14), and
   `catalogue_covers_admitted_features` (OBL-9). Run them and confirm they fail
   on the empty catalogue.
2. Green: implement the catalogue entries for the `core` feature and confirm
   the seeded faults are rejected.
3. Extend the acceptance suite with OBL-8, one unrelated-invocation case per
   feature plus the aggregate run on `mixed-aggregate`. Add the matching BDD
   scenarios.

End state: RM-c demonstrated for `core`, and the registration rule enforced.

Acceptance:

- `cargo nextest run --test shallow_end_non_contagion_tests` passes and
  reports reaching every bucket.
- `cargo nextest run --test shallow_end_acceptance_tests` passes, including
  OBL-8.
- The seeded faults are rejected.
- The full gates pass.

Recovery: revert the commit.

### EP-M6: ADR-0084, documentation, and roadmap closure

1. Re-sweep ADR numbers across every remote branch immediately before
   creating the file. If 0084 is taken, take the next free number and update
   every reference.
2. Write
   `docs/adrs/adr-0084-shallow-end-acceptance-fixtures-and-non-contagion-registration.md`.
   It records D-1, D-2, D-5, D-7, D-13, and D-14 and the registration rule,
   and notes the console-plus-contention incompatibility.
3. Add the "Amended by ADR-0084" banner and `## Amendments` entry to
   ADR-0076, and index ADR-0084 in `docs/contents.md`.
4. Add one paragraph to `docs/netsuke-design.md` section 2.9 that references
   ADR-0084 and names the fixtures as RFC25-SE acceptance evidence.
5. Add a "Shallow-end acceptance fixtures" subsection under "Test execution"
   in `docs/developers-guide.md`. It covers:
   - the corpus and its freeze rule;
   - the two tiers;
   - `NETSUKE_REQUIRE_STRACE`;
   - utility allowlists;
   - registering a catalogue case with its own declarations and declared
     global effects;
   - the harness reuse policy.
6. Update "User-facing documentation examples" in the same guide to list the
   quickstart as `MarkedOnly`.
7. Record `test_support::shallow_end`, `syscall_trace`, and the `fs/` split in
   `docs/repository-layout.md`.
8. File the Step 3 issue if M0 found a mismatch.
9. Tick 20.1.2 and its sub-bullets in
   `docs/roadmap-progressive-enhancement.md`.
10. Reconcile `Surprises` against the conformance basis, then mark this plan
    `COMPLETE`.

No CHANGELOG or users' guide change is needed: there is no behaviour change,
and no translated guide exists.

End state: the policy is recorded, indexed, and discoverable.

Acceptance:

- `make fmt` has been run.
- These gates pass: `make check-fmt`, `make lint`, `make test`,
  `make doc-coverage`, `make markdownlint`, `make nixie`,
  `make test-documentation-contracts`, and `make test-workflow-contracts`.
- `typos.toml` is unchanged, or its regeneration is handled per AGENTS.md
  "Spelling".

Recovery: revert the commit.

## Milestones and plateaus

| Milestone | Requirements                       | Plateau (safe stopping state)                             | Conformance check                                     | Compatibility                         | Remaining gaps                                                  |
| --------- | ---------------------------------- | --------------------------------------------------------- | ----------------------------------------------------- | ------------------------------------- | --------------------------------------------------------------- |
| EP-M0     | Risks, A-2, A-5                    | No repository change; decisions recorded                  | Each tier's assumptions confirmed or the plan revised | None                                  | All implementation                                              |
| EP-M1     | RM-a, ADR75-ADMIT, OBL-1           | Corpus frozen, provenance enforced                        | Fixture equals RFC and documentation; no prose change | None                                  | Observation (RM-b), non-contagion (RM-c)                        |
| EP-M2     | D-4                                | Shared `fs` walk and trace readers; Kani suite green      | No policy in readers; no shim left                    | None: callers move in the same commit | Harness                                                         |
| EP-M3     | D-3, D-12, D-13                    | Harness usable but unused by acceptance tests             | Adapters report and do not judge; no exemptions       | None                                  | Acceptance assertions, CI                                       |
| EP-M4     | RM-a, RM-b, OBL-2 to OBL-6, D-9    | Shallow end demonstrated on Unix; trace tier strict in CI | Every RM-b term has an obligation and a control       | None                                  | RM-c                                                            |
| EP-M5     | RM-c, OBL-7 to OBL-9, D-14         | Non-contagion and registration enforced for `core`        | Every catalogue case is live; registry parse is live  | None                                  | Policy record and docs                                          |
| EP-M6     | D-7, AGENTS.md documentation rules | Complete                                                  | Every trace link resolves; amendment recorded         | None                                  | None for 20.1.2; feature evidence arrives with 21.1.2 to 25.3.1 |

Table 3: Milestone plateaus and their checks.

## Concrete steps

Run every command from the repository root. Pipe each gate through `tee`, for
example
`make test 2>&1 | tee /tmp/test-netsuke-20-1-2-unchanged-basic-workflow-acceptance-fixtures.out`.
Gates run sequentially for the candidate tree, delegated to the `scrutineer`
agent.

1. EP-M0: run `cargo build --bin netsuke` once, then follow the spike steps in
   `/tmp/shallow-end-spike-20-1-2/`.
2. For each milestone, run the focused red command first and confirm it fails
   for the stated reason. Then implement, rerun the focused command, and run
   the milestone's gates.
3. Commit after each green milestone with an imperative subject naming the
   item, for example `Freeze the shallow-end corpus (20.1.2)`.

The planned file set (about 50 files) is as follows.

New `test_support` files:

- `test_support/src/fs/mod.rs` (moved from `fs.rs`) and `fs/walk.rs`.
- `test_support/src/syscall_trace/mod.rs`, `syscall_trace/tests.rs`, and
  `syscall_trace/execve.rs`.
- `test_support/src/shallow_end/` with `mod.rs`, `recording_ninja.rs`,
  `tree.rs`, `tripwire.rs`, `trace_run.rs`, and `run.rs`.

New test files:

- `tests/shallow_end_corpus_tests.rs`.
- `tests/shallow_end_acceptance_tests.rs`, with
  `tests/shallow_end_acceptance/{mod.rs,diagnostics.rs,controls/*}`.
- `tests/shallow_end_non_contagion_tests.rs`, with
  `tests/shallow_end_non_contagion/{projection,bases,catalogue,seeded_faults}.rs`.
- Four `tests/data/shallow_end/*/Netsukefile` fixtures.
- `tests/features_unix/shallow_end.feature`.
- `tests/bdd/steps/shallow_end.rs`.
- Two Python contracts under `tests/workflow_contracts/`.

Modified files:

- `test_support/src/lib.rs`.
- `tests/prover_tools_kani_check_tests.rs` and its `support/` modules (the old
  `trace.rs` is removed).
- `tests/documentation_examples/mod.rs` and
  `tests/documentation_examples_tests.rs`.
- `tests/bdd/steps/mod.rs`.
- `.github/workflows/ci.yml` and `.github/workflows/coverage-main.yml`.
- `Makefile`.

Documentation files:

- The new ADR-0084.
- `docs/adrs/adr-0076-*`.
- `docs/contents.md`.
- `docs/netsuke-design.md`.
- `docs/developers-guide.md`.
- `docs/repository-layout.md`.
- `docs/quickstart.md`.
- `docs/roadmap-progressive-enhancement.md`.

Expected focused output after EP-M4 is shown below. It is illustrative; M0
fixes the real case names:

```plaintext
    PASS [   0.412s] netsuke::shallow_end_acceptance_tests corpus_runs_unchanged::case_1_quickstart_hello_portable
    PASS [   0.530s] netsuke::shallow_end_acceptance_tests corpus_runs_unchanged::case_2_quickstart_hello_traced
    ...
    Summary [   4.120s] 18 tests run: 18 passed, 0 skipped
```

## Proposed BDD feature

The feature lives at `tests/features_unix/shallow_end.feature`. It traces to
RM-a, RM-b, and RM-c through milestones EP-M4 and EP-M5.

```gherkin
Feature: Unchanged basic workflow
  A first Netsuke build needs only Netsuke and Ninja, and opting one node
  into a feature never spreads to nodes that do not use it.

  Background:
    Given an isolated environment providing only Netsuke and Ninja
    And a proxy tripwire that records network requests

  Scenario: The quickstart manifest builds exactly as documented
    Given the shallow-end fixture "quickstart-hello"
    When netsuke is run without arguments in the shallow-end workspace
    Then the shallow-end run should succeed
    And Ninja should have planned exactly the command "echo 'Hello from Netsuke!' > hello.txt"
    And the shallow-end file "hello.txt" should contain exactly "Hello from Netsuke!\n"
    And the workspace should gain only the declared outputs and Ninja's log
    And no generated runtime state should exist
    And the proxy tripwire should have recorded no requests

  Scenario: Ordinary variables interpolate into a scalar shell recipe
    Given the shallow-end fixture "quickstart-vars"
    When netsuke is run without arguments in the shallow-end workspace
    Then the shallow-end run should succeed
    And Ninja should have planned exactly the command "echo 'Hello, World!' > greeting.txt"

  Scenario: Building an unrelated target in a mixed project changes nothing
    Given the shallow-end fixture "mixed-aggregate"
    When netsuke is run with shallow-end arguments "build report.txt"
    Then the shallow-end run should succeed
    And the observations should equal fixture "three-command" with arguments "build report.txt"
    And the shallow-end file "shout.txt" should not exist

  Scenario: An unannotated aggregate builds opted-in and plain targets
    Given the shallow-end fixture "mixed-aggregate"
    When netsuke is run without arguments in the shallow-end workspace
    Then the shallow-end run should succeed
    And the recorded build file should bind "pool = console" only on "shout.txt"
    And the shallow-end file "shout.txt" should contain exactly "HELLO\n"
```

Step wording carries the "shallow-end" prefix wherever a generic step would
otherwise collide. Check `tests/bdd/steps/` for an equivalent step before
adding one.

## Validation and acceptance

The work is accepted when all of the following hold:

- `make test` runs `shallow_end_corpus_tests`,
  `shallow_end_acceptance_tests`, `shallow_end_non_contagion_tests`, and the
  four BDD scenarios, and they pass. Each focused red step was observed and
  recorded in `Progress`.
- CI's `build-test` and `coverage-upload` logs show the trace tier executing,
  not skipping.
- Every negative control and seeded fault is recorded as rejected for its
  intended reason.
- The final tree passes `make check-fmt`, `make lint`, `make test`,
  `make doc-coverage`, `make markdownlint`, `make nixie`,
  `make test-documentation-contracts`, and `make test-workflow-contracts`.

A reviewer can read the reviewed `quickstart_hello` snapshots under
`tests/shallow_end_acceptance/snapshots/` and see the evidence directly:

- one Ninja invocation;
- the single planned command;
- on the traced snapshot, the process tree Netsuke, Ninja, and
  `/bin/sh -c "echo 'Hello from Netsuke!' > hello.txt"`;
- the default diagnostics;
- the workspace delta `hello.txt` and `.ninja_log`.

## Idempotence and recovery

Every test builds its own temporary root and leaves the repository untouched.
Spike work stays under `/tmp/shallow-end-spike-20-1-2/`. Each milestone is one
to three commits and reverts independently. Snapshot acceptance is explicit
(`cargo insta review`), so no rerun rewrites expectations silently. Proptest
regression files are not committed: the property uses a deterministic seed, so
a failure reproduces from the seed alone.

## Artefacts and notes

The EP-M0 transcripts, the negative-control evidence, and the seeded-fault
results will be recorded here as each milestone completes.

## Interfaces and dependencies

The new `test_support` surface is sketched below. Each item carries `///`
documentation with an example, marked `no_run` where the example spawns a
process.

```rust
pub mod shallow_end {
    pub struct ShallowEndWorkspace { /* temp root, corpus copy, isolated dirs, bin allowlist */ }
    impl ShallowEndWorkspace {
        pub fn from_fixture(name: &str, utilities: &[&str]) -> anyhow::Result<Self>;
        pub fn run_portable(&self, args: &[&str]) -> anyhow::Result<ShallowEndObservation>;
        #[cfg(target_os = "linux")]
        pub fn run_traced(&self, args: &[&str]) -> anyhow::Result<Option<ShallowEndObservation>>;
    }
    pub struct ShallowEndObservation {
        pub status: std::process::ExitStatus,
        pub stdout: String,
        pub stderr: String,
        pub ninja: Vec<NinjaInvocation>,        // portable tier
        pub delta: TreeDelta,
        pub proxy_requests: usize,
        pub trace: Option<TraceFindings>,      // trace tier
    }
}
pub mod syscall_trace { /* promoted reader plus execve, structured-path, and cwd readers */ }
```

`run_traced` returns `Ok(None)` only when the tracer is absent and
`NETSUKE_REQUIRE_STRACE` is unset. The non-contagion catalogue is local to the
test crate (`tests/shallow_end_non_contagion/catalogue.rs`) because it encodes
policy and grows with each admitted feature. No public `netsuke` API changes
and no dependency is added.

## Revision note

- 2026-11-17: Initial draft from Wyvern reconnaissance and Firecrawl checks.
- 2026-11-17: Revised after a four-member expert panel. The revisions fall
  into eight areas:
  - **Process accounting.** The isolated `PATH` and the shim broke the
    process accounting. The fix is a per-fixture utility allowlist, a
    separate traced run without the shim, and the parent-based process
    closure (D-13).
  - **Controls.** A vacuous builtin control was replaced with the `tr`
    witness.
  - **Projection and verification.** The OBL-7 projection now covers the
    whole edge and its resolved action. The plan adds a `shared-rule` base, a
    graph-level seeded fault, and in-process Ninja-text comparison. The
    property is now an exhaustive enumeration plus a deterministic manual
    `TestRunner` with post-run coverage (D-14).
  - **Coverage.** Coverage `*.profraw` files are handled (D-12).
  - **Definitions.** The definitions of state record, generated runtime
    state, and probe were realigned with ADR-0076 and ADR-0077. `serial`
    sidecars are ADR-0012 declared global effects.
  - **Policy record.** ADR-0084 became an amendment to ADR-0076, enforced by
    a registry-parsing test (OBL-9).
  - **Scope.** The users' guide paragraph, the quickstart Step 3 rewrite,
    and the `foreach` fixture were dropped.
  - **Conventions.** The dylint exemptions were replaced by a planned `fs`
    split. The plan adds the `coverage-upload` tracer, the Python contracts,
    the documentation-example registration steps, a `Milestones and
    plateaus` table, and runnable counting commands. Budgets were re-derived.

  The Linux tracing and CI panel member's experiments produced further changes,
  also applied in this revision:
  - **Trace command.** The `strace` command was corrected (D-3). It now
    wraps `netsuke` by absolute path, uses `-s 65536 -y -e signal=none`, and
    drops `-v` to keep environments out of logs.
  - **Trace reading.** The plan now covers `unfinished`/`resumed` pairing
    and clone-based parent attribution, and ignores the `/proc` false
    positive.
  - **Shim argument order.** `-t commands` now goes before `--`.
  - **`fetch` control.** The control was moved into a rendered `command`,
    with a `403` reply and a non-zero exit expected.
  - **Tracer probe.** The availability probe now attaches to a real process.
  - **Quickstart Step 3.** The stale Step 3 output was recorded as a
    discovery.

  Effect on remaining work: larger code and file budgets, no production change,
  and approval still pending.
