# Mutation baseline reaches real build-tool preflight without the required pinned mold (issue #875)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

The scheduled mutation-testing run has failed its *baseline* build for at least
two weeks, so no mutant has been evaluated on this repository since. The
baseline is the unmutated `make test` run the worker performs before it mutates
anything; when it fails, cargo-mutants reports no mutant results at all.

Two distinct faults are tangled together in the reports, and only one of them
is a CI provisioning problem:

1. **A genuine CI gap.** The mutation caller
   (`.github/workflows/mutation-testing.yml`) selects the shared reusable
   workflow `leynos/shared-actions/.github/workflows/mutation-cargo.yml` pinned
   at `0606ca89…`, whose `install-mold` input defaults to `'false'`. The worker
   therefore has no `mold` on `PATH`. Every netsuke build target depends on the
   `check-build-tools` prerequisite, which fails with
   `build-tools: mold not found on PATH (pinned 2.41.0)`. Fixing the CI job is
   a two-line caller change.

2. **A test-boundary fault.** Two tests in
   `tests/makefile_test_target/rustdocflags.rs` are supposed to be *hermetic*:
   they drive the real `doctest` recipe with a fake `cargo` on the `PATH`, and
   assert that a caller-supplied `RUSTDOC_FLAGS` reaches Cargo's `RUSTDOCFLAGS`
   unchanged. They are not hermetic, because the `doctest` recipe has a
   `check-build-tools` prerequisite and the test inherits the ambient host
   `PATH`. On a developer machine with the pinned `mold` installed the tests
   pass; on the mutation worker they fail with the `mold not found` diagnostic
   above. That sentence is the *entire* actionable content of the CI failure —
   the two tests' own assertions never ran, so the failure tells a reader
   nothing about `RUSTDOCFLAGS`.

After this change a manifest reader can observe that:

1. `make test-workflow-contracts` pins the caller's mold provisioning: the
   contract test asserts the caller enables `install-mold` and runs
   `make check-build-tools` in `setup-commands`, so a later edit that drops
   either fails on the pull request rather than in a scheduled run.
2. The two rustdoc tests pass on a machine with **no host `mold` and no host
   `rustup`**, because they inject the build-tools boundary through the shared
   `Sandbox` helper exactly as `tests/build_tools_check_tests.rs` already does.
   A reader can reproduce this: move `$HOME/.local/bin/mold` aside, run
   `make test-nextest` restricted to the `makefile_test_target` binary, and see
   the rustdoc tests pass while `build_tools_check_tests` still exercises the
   real preflight.
3. Real-preflight coverage of the `doctest` target does not disappear: it
   moves to `tests/build_tools_check_tests.rs`, where `doctest` joins the
   existing parameterized "targets stop when the check fails" table.

The distinction matters because the two halves have different owners. (1) is
provisioning: a worker that has never had `mold` installed. (2) is a design
choice: whether a test of *argument propagation* should also be, incidentally,
a test of *host capability validation*. It should not be — the capability gate
has its own suites, and a test that crosses both boundaries reports whichever
one fires first, masking the other.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not a workaround.

- **`cmd`/`interfaces` scope.** This change touches a GitHub Actions caller,
  one Python contract suite under `tests/workflow_contracts/`, and two Rust
  test modules. It must not change `.cargo/config.toml`, the `Makefile`'s flag
  composition, `scripts/check-build-tools.sh`, or any production Rust source
  under `src/`.
- **The build standard must survive.** `RUSTFLAGS` must not be set or
  overridden by the caller workflow. The plan's whole reason for existing is
  that the worker needs the pinned linker; silently replacing the project's
  `rustflags` tables would "fix" the diagnostic while removing the thing the
  diagnostic protects. See
  <https://github.com/leynos/shared-actions/issues/544> for the flag-ownership
  question this leaves open.
- **No new dependencies.** The Rust side already depends on
  `test_support::build_tools`; the Python side already depends on
  `workflow_loading`. No crate or Python package is added.
- **No compatibility machinery.** The two rustdoc tests are test-only surfaces.
  They are rewritten, not dual-pathed; the old direct `Command::new("make")`
  helper is deleted outright.
- **Existing runner, cache, and concurrency limits stay untouched.**
  `concurrency`, `permissions`, and the triggers in the caller keep their
  current values. No `runs-on`, cache, or shard setting is added.
- **`mold` must not be added to the sandbox utility allowlist.** The allowlist
  in `test_support/src/build_tools/sandbox/utilities.rs` is deliberately
  explicit so a new dependency surfaces as a test failure. Adding `mold`,
  `rustup`, or `cargo` to it would defeat the injection the plan is introducing.
- **British-oxendict spelling throughout**, including in comments and new
  prose, per `AGENTS.md`.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- **Scope:** if implementation requires changes to more than 8 tracked files,
  or more than 250 net lines, stop and escalate.
- **Interface:** if `test_support::build_tools`'s public surface must gain a
  new public function or type, stop and escalate. (Reusing `Sandbox::run_make`,
  `Sandbox::write_mold`, `Sandbox::write_rustup`, `MakeInvocation`,
  `pinned_mold_version`, `pinned_toolchain`, and `combined` needs no new
  surface. Adding a sandbox utility to the allowlist is not an interface change
  but is governed by the constraint above.)
- **Dependencies:** any new external dependency requires escalation. The
  expected count is zero.
- **Iterations:** if the two rustdoc tests still fail under a mold-free `PATH`
  after 3 fix attempts, stop and escalate with the captured `PATH`, the
  `.stderr` text, and the sandbox layout.
- **Ambiguity:** if the caller cannot express "install mold, then verify the
  repository pin" with the pin's declared inputs, stop and present options
  rather than inventing an input.
- **Worker dispatch:** the manual dispatch in Milestone 3 costs a long build on
  a shared runner. If the dispatch cannot be started within the first three
  attempts, record the failure and continue with local evidence; do not retry a
  fourth time without escalation.

## Risks

Known uncertainties that might affect the plan. Each notes severity,
likelihood, and mitigation.

- **Risk:** the shared workflow's `install-mold` input is typed `string`, not
  `boolean`, in the pinned revision. Passing a YAML boolean would be a type
  error at workflow-call time. Severity: high (the job would not start at all).
  Likelihood: medium. Mitigation: pass the literal quoted string `'true'`,
  matching the documented usage; assert the *type* in the contract test by
  reading the raw YAML value and requiring it to be the string `"true"` rather
  than the boolean `True`. Verified during reconnaissance: the input is declared
  `type: string` / `default: 'false'` at the pin.

- **Risk:** `setup-commands` may run *after* the baseline rather than before it,
  in which case verifying the pin there would not protect the baseline.
  Severity: high. Likelihood: low. Mitigation: reconnaissance confirmed the job
  order is Setup Rust (which handles `install-mold`) → Install cargo-mutants →
  Run setup commands → baseline. The `make check-build-tools` in
  `setup-commands` therefore runs before the baseline. If a later observation
  contradicts this, escalate rather than reordering.

- **Risk:** a warm sandbox `PATH` in the new rustdoc tests could accidentally
  resolve a real `mold`, making the test pass on a machine that has one and
  giving a false sense of hermeticity. Severity: medium. Likelihood: low.
  Mitigation: `Sandbox` uses `env_clear` and sets `PATH` to the sandbox `bin`
  alone, so only files the test writes are visible. Verify by running the suite
  with `$HOME/.local/bin` removed from the ambient `PATH`, which is the
  mold-free reproduction below.

- **Risk:** removing the direct `Command::new("make")` path also removes
  coverage of "the recipe works when `BUILD_TOOLS_PREFIX` is the caller's real
  prefix". Severity: low. Likelihood: low. Mitigation: that coverage is not
  lost but relocated — `build_tools_check_tests.rs` already exercises the real
  recipes through `Sandbox::make`, and the `doctest` cases added there run
  against the same sandbox.

- **Risk:** the `Text file busy` (`ETXTBSY`) report from the 18 September run
  may be a real defect in a Ninja fixture, or may be a transient from the same
  window as the mold failure. Severity: low (it has not recurred). Likelihood:
  low. Mitigation: Milestone 3 performs a *read-only* audit of the fixture
  write paths (`test_support::exec::write_exec_with_content` →
  `test_support::fs::write` → `set_mode`) and changes a helper only if a write
  handle can outlive a spawn. No retries are added.

## Progress

Every stopping point is documented here. Timestamps are UTC.

- [x] (2026-10-12 16:45Z) Reconnaissance: confirmed the upstream diagnostic
  reproduces locally. With `PATH` and `HOME` both free of `mold`/`rustup`,
  `make … doctest` exits 2 at
  `make: *** [Makefile:476: check-build-tools] Error 1` with
  `build-tools: mold not found on PATH (pinned 2.41.0)`.
- [x] (2026-10-12 16:50Z) Reconnaissance: confirmed the byte-identical
  assertion travels through `run_doctest_with_override` →
  `assert_doctest_record` (`rustdocflags.rs:158-192`) and that the helper uses
  `env!("CARGO_MANIFEST_DIR")` + direct `Command::new("make")`, which under
  `cargo nextest` is the *worktree* root. Noted that whether `doctest` inherits
  `CARGO_TARGET_DIR` from the agent shell is unproven and is *not* load-bearing
  for this plan.
- [x] (2026-10-12 16:55Z) Reconnaissance: read the pinned shared workflow's
  `workflow_call.inputs`. `install-mold` is `type: string`, `default: 'false'`;
  `setup-commands` is `type: string`, `default: ""`. The shared revision
  already declares both — upstream `9a279509` ("Forward install-mold from
  mutation-cargo to setup-rust (#545)") is an ancestor of the pin. **No pin
  bump is needed.**
- [ ] Milestone 1: caller enabled and verified; contract test extended; suite
  green.
- [ ] Milestone 2: rustdoc tests sandboxed; `doctest` added to the
  real-preflight table; suite green under a mold-free `PATH`.
- [ ] Milestone 3: local gates, worker dispatch, linker-selection observation,
  ETXTBSY audit.
- [ ] Milestone 4: ExecPlan finalized, commit, push, draft PR.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.

- **Observation:** the repository already documents the exact bug this plan
  fixes, in the shared workflow's own guide. Evidence:
  `https://github.com/leynos/shared-actions/blob/main/docs/mutation-cargo-workflow.md`
  states "Cargo takes `RUSTFLAGS` over every `rustflags` entry in
  `.cargo/config.toml`", "so a mold flag set there does not apply in these
  runs", and "mold is installed, and used only where the configuration selects
  it through `linker`". Impact: installing mold is necessary but is **not**
  proven sufficient. The workflow does not change `setup-rust`'s `rustflags`
  default of `RUSTFLAGS="-D warnings"`, which displaces `.cargo/config.toml`'s
  `cfg(target_os = "linux")` table — the very table that carries
  `-Clink-arg=-fuse-ld=mold`. Milestone 3 therefore *observes* linker selection
  rather than asserting it, and records the finding against
  <https://github.com/leynos/shared-actions/issues/544> instead of adding a
  local workaround. The plan's constraint forbidding a caller-side `RUSTFLAGS`
  follows directly from this.

- **Observation:** the pin in the caller is *not* the revision named in the
  issue text. Evidence: the issue says the caller is "pinned at
  `ff1dd759dfffc0db3459e30e833f52437ee62b57`"; the file at `main` (and at this
  branch's merge base `c65ad970`) pins
  `0606ca899ce2995a0c4bc78e660e80832e6e8855`. `ff1dd759` is a *different*
  shared-actions revision that this repository uses elsewhere, for
  `ensure-cargo-version` in `release.yml`. Impact: the issue's SHA is stale or
  was transcribed from another workflow. The conclusion is unchanged — the pin
  already exposes `install-mold`, it is merely not enabled. No pin bump is
  required, contrary to the issue's conditional "with a workflow revision that
  exposes it".

- **Observation:** the failure surfaces at the `check-build-tools`
  *prerequisite*, before the `doctest` recipe's own command line is composed.
  Evidence: `make: *** [Makefile:476: check-build-tools] Error 1`, with the
  fake Cargo never invoked. Impact: this is why the diagnostic is so unhelpful,
  and why the fix must be split — the CI half makes the worker capable, the
  test half stops the test depending on worker capability at all.

## Decision log

Record every significant decision made while working on the plan.

- **Decision:** do not bump the `uses:` SHA. Add only `install-mold` and
  `setup-commands` to the existing `with:` block. Rationale: the pinned
  revision already declares both inputs (verified by reading the workflow at
  that exact SHA and by confirming upstream commit `9a279509` is an ancestor).
  Bumping would widen the diff, re-pin an unrelated set of actions, and
  contradict the caller's own contract test, which deliberately asserts the
  *shape* of the pin — a 40-character lowercase hex SHA — rather than its
  value, because Dependabot owns the value. Date/Author: 2026-10-12,
  implementer.

- **Decision:** pass `install-mold: 'true'` as a quoted YAML string.
  Rationale: the input is declared `type: string`. A YAML boolean would fail
  the workflow-call type check. The contract test asserts the *string* form so
  a later `true` → `'true'` edit cannot silently reintroduce the fault.
  Date/Author: 2026-10-12, implementer.

- **Decision:** verify the pin with `make check-build-tools` in
  `setup-commands`, printing `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` for the
  log, and set no `RUSTFLAGS` in the caller. Rationale:
  `make check-build-tools` is the repository's own definition of "the pinned
  mold is present and usable", and its diagnostics are exactly the ones the
  issue asks to retain. Printing the two flag variables turns the open
  `shared-actions#544` question into recorded evidence rather than speculation.
  Setting `RUSTFLAGS` is forbidden by the constraints. Date/Author: 2026-10-12,
  implementer.

- **Decision:** the two rustdoc tests inject the build-tools boundary rather
  than reaching the real preflight; the real preflight gains `doctest` in the
  `tests/build_tools_check_tests.rs` parameterized table. Rationale: the tests'
  subject is argument propagation across the Make-to-Cargo boundary, not host
  capability. Coupling the two made the tests fail for a reason unrelated to
  their assertions. The capability gate already owns real-preflight validation
  in the two dedicated suites named in its own module docstring; adding
  `doctest` there keeps coverage of the exact prerequisite relation that broke
  CI. Date/Author: 2026-10-12, implementer.

- **Decision:** take versions from the committed-pin readers
  (`pinned_mold_version()`, `pinned_toolchain()`) rather than hard-coding
  `2.41.0`; where a literal must appear in a contract test, read
  `tools/mold/VERSION`. Rationale: `sandbox/pins.rs` exists specifically so
  tests "agree with the repository rather than hard-coding a value that drifts
  on the next bump". Date/Author: 2026-10-12, implementer.

- **Decision:** use the `Sandbox` Make runner, which sets
  `BUILD_TOOLS_PREFIX=<sandbox prefix>`, so `doctest`'s prerequisite runs
  against the sandbox's fake mold instead of the caller's real toolchain.
  Rationale: this is the same arrangement
  `tests/build_tools_check_tests.rs::healthy_sandbox` already uses, and it is
  the only arrangement under which the rustdoc tests are hermetic. The
  `BENCH_ROOT`/`doctest` bookkeeping risk is therefore not applicable.
  Date/Author: 2026-10-12, implementer.

## Outcomes & retrospective

To be completed at Milestone 4. The comparison is against
`Purpose / big picture`, and no discovery may remain unreconciled with
`Conformance basis`.

## Context and orientation

The reader needs five things to follow this plan.

**The build standard.** Netsuke compiles with the `mold` linker and the parallel
`rustc` frontend by default. `.cargo/config.toml` declares them in a
`[target.'cfg(target_os = "linux")']` table, so a bare `cargo build` gets them.
Cargo selects *one* `rustflags` source, so an externally set `RUSTFLAGS`
displaces that table entirely — the reason the `Makefile` restates the flags in
`GATE_RUSTFLAGS` (`Makefile:78`).

**The capability gate.** Every build, test, and lint target depends on
`check-build-tools`, a phony target whose recipe is
`@scripts/check-build-tools.sh`. The script reports `build-tools: <message>` on
stderr and exits non-zero if `mold` is missing, unrunnable, or at a version
other than the one in `tools/mold/VERSION` (currently `2.41.0`), or if the
pinned toolchain is absent.

**The mutation caller.** `.github/workflows/mutation-testing.yml` is a thin
caller of `leynos/shared-actions/.github/workflows/mutation-cargo.yml`. It has
one job, `mutation`, whose `with:` block currently carries `exclude-globs` and
`extra-args: "--all-features"`. The default `setup-rust` on the shared worker
exports `RUSTFLAGS="-D warnings"`; the caller must not fight that.

**The shared sandbox.** `test_support/src/build_tools/sandbox/mod.rs` builds a
temporary `PATH` and `HOME` "from nothing": `env_clear`, then only the sandbox
`bin` and `home`. `Sandbox::run_make(&MakeInvocation)` runs
`make --no-print-directory -f Makefile BUILD_TOOLS_PREFIX=<prefix> <target>`
inside that environment, where `<prefix>` is empty and the test populates
`<prefix>/bin` with fakes. `Sandbox::write_mold(dir, version)` writes a fake
`mold` that echoes `mold <version> (compatible with GNU ld)`, which is the
format `installed_mold_version` parses. Pins are read from the committed files
by `sandbox/pins.rs`.

**The two failing tests.** `tests/makefile_test_target/rustdocflags.rs` holds
`behavioural_doctest_passes_shell_sensitive_overrides_only_as_rustdocflags` and
`property_doctest_preserves_rustdoc_override_values`. Both call
`run_doctest_with_override`, which today runs `make` directly with the ambient
environment plus `CARGO` and `RUSTDOC_ENVIRONMENT_LOG`. `CARGO` is a
`#!/bin/sh` script that records `RUSTDOCFLAGS`, whether `RUSTDOC_FLAGS` is
present, and `"$*"`, tab-separated, into `$RUSTDOC_ENVIRONMENT_LOG`.
`assert_doctest_record` then asserts on those three fields.

Terms used in this plan:

- **baseline** — the unmutated build-and-test run cargo-mutants performs before
  mutating, which establishes the expected green result.
- **mutant** — a source-level mutation cargo-mutants applies to test whether the
  suite notices; a mutated tree that still passes is a *survivor*.
- **preflight** — `check-build-tools`, which runs before a target's recipe.
- **hermetic** — a test that depends only on what it created, never on the host
  machine's installed tools.

## Conformance basis

This repository has no Terms of Reference or technical-design revision for
developer-tooling CI repairs; the governing artefacts are the repository's own
checked-in contracts and the upstream shared workflow.

- **Upstream requirement, provisioning:** `leynos/shared-actions` issue #591
  (shared prevention and diagnostic contract) and #590 (result completeness).
  These are cross-repository and are *not* discharged by this change; the
  caller change is this repository's contribution to #591.
- **Upstream open question:** `leynos/shared-actions` issue #544 (linker-flag
  ownership when `setup-rust` exports `RUSTFLAGS`). Traced to EP-M3's
  linker-selection observation. Deliberately left open.
- **Repository contract, `cmd`:** `AGENTS.md` "The build standard", which
  states that changing one `rustflags` source without the other fails a
  contract test, and that the release and coverage shapes must stay excluded.
- **Repository contract, `interfaces`:**
  `tests/workflow_contracts/mutation_testing_test.py`, whose `EXPECTED_WITH` is
  documented as "assert the whole block so an added or dropped input fails
  loudly". This plan changes that block deliberately and updates the test in
  the same change.
- **Repository contract, `interfaces`:** `tests/build_tools_check_tests.rs`
  module docstring, which states that the check's purpose is "to turn a missing
  tool into an actionable message instead of an opaque codegen-backend or
  linker failure", and that "none [of the cases] needs mold, rustup, or a
  network". The new `doctest` cases extend the parameterized table that
  discharges this.
- **Trace chain:**
  `shared-actions#591 → EP-M1 → tests/workflow_contracts/mutation_testing_test.py::test_with_block_carries_the_caller_configuration`
  and `shared-actions#544 → EP-M3 → the dispatch log's printed RUSTFLAGS`.

## Milestones

Each milestone ends in a coherent, validated repository state.

### EP-M1 — Provision mold on the worker and pin the provisioning

**Assigned requirements:** `shared-actions#591` (this repository's half).

**End state.** `.github/workflows/mutation-testing.yml` passes
`install-mold: 'true'` and a `setup-commands` value that runs
`make check-build-tools` and prints the two flag variables. Its header comment
states that mold is provisioned by the shared installer and checked by the
repository preflight. `tests/workflow_contracts/mutation_testing_test.py`
asserts both, and its `EXPECTED_WITH` matches the caller exactly.

**Red.** Add the `install-mold` and `setup-commands` entries to `EXPECTED_WITH`
and add the typed tests *first*, then run `make test-workflow-contracts`.
Expect failure: the caller does not yet pass either input. The failure must
name the missing input, not merely differ in `repr`.

**Green.** Edit the caller. Re-run the suite.

**Exact assertions for the contract test:**

- `test_with_block_carries_the_caller_configuration` gains the two keys in
  `EXPECTED_WITH`; its docstring and failure message mention mold provisioning.
- A new standalone test asserts the *type* of `install-mold` is the string
  `"true"` (not a YAML boolean) and that `setup-commands` invokes
  `make check-build-tools`. Each assertion carries a message.
- If the caller passes a mold version, a further test reads
  `tools/mold/VERSION` and asserts equality. **Decision:** the caller does
  **not** pass one, because the pinned shared revision declares no
  `mold-version` input. The test is therefore not added; the plan says so
  explicitly rather than leaving a gap. The repository pin is still verified, by
  `make check-build-tools` at run time.
- The SHA-format, permission, concurrency, and trigger tests are unchanged.

**Conformance check.** Does the caller still pin the `.github/actions/...`
shape, keep `permissions: {}` at the top level and exactly `contents: read` +
`id-token: write` on the job, and keep
`concurrency.group: mutation-testing-${{ github.ref }}` and
`cancel-in-progress: false`? All four tests must still pass.

**Acceptance evidence:** the full output of `make test-workflow-contracts`,
green, with the new tests named in the run.

**Recovery:** revert the two files; nothing depends on them yet.

**Remaining gaps:** the worker has not actually been observed running this.

### EP-M2 — Make the rustdoc tests hermetic; keep real-preflight coverage

**Assigned requirements:** the `interfaces` contract in
`tests/build_tools_check_tests.rs`; the issue's "Decide explicitly whether
rustdoc-override tests intend to exercise real host-tool validation or should
inject that boundary alongside fake Cargo".

**End state.** Both rustdoc tests pass with no `mold` and no `rustup` anywhere
on the host. `doctest` is one of the parameterized cases in
`tests/build_tools_check_tests.rs`, asserting that a missing or drifting mold
stops `make doctest` before the fake Cargo runs.

**Red.** Under a mold-free `PATH`, the two rustdoc tests fail at
`check-build-tools`. Reproduce with:

```sh
mkdir -p /tmp/empty-tools
PATH="$HOME/.cargo/bin:/usr/bin:/bin" make --no-print-directory -f Makefile \
  CARGO=/bin/false RUSTDOCFLAGS='--cfg docsrs -D warnings' doctest
```

After the change, run the focused suite with `mold` unreachable:

```sh
PATH="/usr/bin:/bin" RUSTFLAGS="-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold" \
  cargo nextest run --test makefile_test_target --all-features -E 'test(rustdocflags)'
```

Note the apparent contradiction: `RUSTFLAGS` here carries
`-Clink-arg=-fuse-ld=mold` while `mold` is unreachable. That is deliberate and
is precisely the CI worker's situation — it proves the tests no longer depend on
`mold` being resolvable, because they never invoke a real compiler.

Also set `NETSUKE_REQUIRE_NINJA` is *not* needed here; these tests do not use
Ninja.

**Green.** Rewrite the helper to use the sandbox.

**Shape of the change in `tests/makefile_test_target/rustdocflags.rs`:**

- Delete the direct `Command::new("make")` construction from
  `run_doctest_with_override`; take `&Sandbox` instead of a `make` command.
- The `Sandbox` is constructed once per test with `Sandbox::new()`, then
  `write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)` and
  `write_rustup(&pinned_toolchain()?)?`, following
  `build_tools_check_tests.rs::healthy_sandbox`.
- `write_fake_cargo` moves the fake Cargo into the sandbox `bin` (so `doctest`
  can find it) **and** the `CARGO` Make variable still names it explicitly, so
  the recipe's `${CARGO}` use is unchanged.
- `RUSTDOC_ENVIRONMENT_LOG` is passed through `MakeInvocation::environment`
  (the fake Cargo script reads it directly from the environment; it is not a
  Make variable).
- The `log` path stays inside the sandbox home, so repeated runs cannot collide.
- The command-line-override case uses
  `MakeInvocation::variable("RUSTDOC_FLAGS", value)`; the environment case uses
  `MakeInvocation::environment(…)`. Both routes must remain, because the
  distinction between them is the point of the test.
- On failure the message includes the full combined stderr/stdout from
  `combined(&output)`, so a future failure is diagnosable without re-running.
- `assert_doctest_record` is unchanged. Its assertions — exact
  `RUSTDOCFLAGS`, `RUSTDOC_FLAGS` absent, and the doctest argument list — are
  the test's subject and must not be weakened.
- The module docstring gains a short paragraph stating that these tests inject
  the build-tools boundary, and that `build_tools_check_tests.rs` and
  `build_tools_pin_tests.rs` own real-preflight validation.

For `property_doctest_preserves_rustdoc_override_values`, one sandbox is built
per proptest case (the test body already creates its temporary directory per
case, so the shape is unchanged) and the environment log is truncated by the
fake Cargo's `>` redirection on every invocation, so the 16 cases · 2 sources =
32 assertions each read the record just written.

**Shape of the change in `tests/build_tools_check_tests.rs`:**

- `doctest` joins the `#[case(...)]` list on
  `build_targets_stop_when_the_check_fails`, whose body already arranges "no
  mold anywhere, everything else present and recording", runs the target, and
  asserts the check's message appears while the recording Cargo saw nothing.
- No new helper is required, because the existing body is target-agnostic.
- `tests/build_tools_pin_tests.rs` is not changed.

**Verify the allowlist is not touched.**
`test_support/src/build_tools/sandbox/utilities.rs`'s `SANDBOX_UTILITIES` must
not gain `mold`, `rustup`, or `cargo`. The fake Cargo is a shell script using
only `printf` (a shell builtin) and `:`, so no utility is added.

**Conformance check.** Does `make test-nextest` still pass with `mold` present
*and* with it absent? The first proves nothing regressed on a capable machine;
the second proves the injection works. Both are required.

**Acceptance evidence:** the focused nextest output under both `PATH`s, and the
`make test-workflow-contracts` output unchanged from EP-M1.

**Recovery:** `git revert` the M2 commit; `build_tools_check_tests.rs` and the
sandbox module are untouched by it except for the added case.

**Remaining gaps:** the worker has still not been observed.

### EP-M3 — Validate on the real worker, observe linker selection, audit ETXTBSY

**Assigned requirements:** the issue's Acceptance section, and
`shared-actions#544`.

**End state.** A manual dispatch of `mutation-testing.yml` on this branch
shows, in order: `make check-build-tools` reporting the pinned mold, both
rustdoc tests passing, a green baseline, and a non-empty mutant set. The
`RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` printed by `setup-commands` are
recorded in the pull request against `shared-actions#544`.

**Steps.**

1. Run the full local gate set: `make check-fmt`, `make lint`,
   `make doc-coverage`, `make test`, `make markdownlint`,
   `make test-workflow-contracts`, and `make actionlint` (via
   `make github-actions-lint`). Delegate the full set to `scrutineer`.
2. Dispatch the workflow: `gh workflow run mutation-testing.yml --ref <branch>`
   and watch the run.
3. Read the `setup-commands` step output. Record the literal `RUSTFLAGS` and
   `CARGO_ENCODED_RUSTFLAGS` lines.
4. Confirm a non-empty mutant set executed, not merely a green exit.
5. Confirm `concurrency`, `permissions`, and the runner/cache settings are
   byte-identical to `main` except for the two added `with:` keys.

**Linker selection.** Two outcomes are possible and both are recorded, neither
is "fixed locally":

- If `RUSTFLAGS` is empty or already contains `-Clink-arg=-fuse-ld=mold`, the
  project configuration selects mold and the linker requirement is intact.
  Record that as the observation.
- If the shared workflow's `setup-rust` export displaces the project table, the
  worker will link with the platform linker. Add **no** local workaround; link
  the finding to `shared-actions#544` and pin a fixing shared revision only if
  one exists.

**ETXTBSY audit (read-only unless it reproduces).** The 18 September job
reported `Text file busy` when launching a temporary Ninja fixture. Audit the
write path: `test_support/src/exec.rs::write_exec_with_content` calls
`test_support::fs::write` (which returns after the handle is dropped) and then
`set_mode`. No handle can outlive the spawn on that path. Change a helper only
if the audit finds a writer that keeps a `File` alive across a spawn; add no
retries.

**Acceptance evidence:** the run URL, the `check-build-tools` line, the two
test names in the baseline output, the mutant count, and the printed flag
values.

**Recovery:** the workflow edit is reverted by a commit; the dispatch is
informational and changes no state.

### EP-M4 — Finalize, commit, push, open the draft pull request

Update `Progress`, `Outcomes & retrospective`, and the `Status:` header; re-run
`make check-fmt` and `make markdownlint` because the plan itself is Markdown.
Commit in small units: (1) the ExecPlan, (2) the caller plus its contract test,
(3) the rustdoc sandboxing plus the `doctest` cases, (4) any documentation.
Push over SSH, then open a draft pull request titled
`Fix mutation baseline mold provisioning and rustdoc test boundary (#875)` with
`Closes #875` and a `## References` section carrying
`https://lody.ai/leynos/sessions/4b6172f4-962c-41b6-a91d-7caa693b2a36`.

## Verification plan

### Invariants

- **I1 — The caller never sets `RUSTFLAGS`.** No `env:` block, no `RUSTFLAGS`
  key, anywhere in `mutation-testing.yml`. Verified by a contract assertion
  over the parsed job mapping (`"env" not in job`), plus the existing
  whole-block `with:` equality. *Non-vacuity:* a seeded edit adding
  `env: {RUSTFLAGS: ""}` to the job must fail that assertion. Run it once as a
  negative control.
- **I2 — The rustdoc tests reach the fake Cargo on a mold-free host.** Verified
  by running the focused suite with `PATH` stripped of any real `mold`, and by
  the positive control of the same suite on a `PATH` that has one.
  *Non-vacuity:* the mold-free run must **fail before** the change and **pass
  after**. The pre-change failure was observed (see `Progress`); the
  post-change pass is the discharge.
- **I3 — The `doctest` prerequisite relation is still tested.** Verified by the
  `doctest` case in `build_targets_stop_when_the_check_fails`, which asserts
  both the diagnostic and that the recording Cargo saw zero invocations.
  *Non-vacuity:* remove `check-build-tools` from `doctest`'s prerequisites in a
  scratch copy and confirm the case fails; restore.
- **I4 — The sandbox allowlist is unchanged.** Verified by the existing
  `check-fmt`/`lint` gates plus an explicit diff inspection of `utilities.rs`.
- **I5 — The two rustdoc assertions still carry their full strength.** The
  exact-`RUSTDOCFLAGS`, absent-`RUSTDOC_FLAGS`, and argument-list assertions in
  `assert_doctest_record` are byte-for-byte unchanged.

### Lemmas and axioms

- **Lemma L1:** `Sandbox::write_mold` produces output that
  `installed_mold_version` parses to the requested version. *Method:* already
  covered by `build_tools_check_tests.rs`, which relies on the same property
  through the real script. Treated as an axiom here, with the contract-level
  evidence cited rather than re-derived.
- **Axiom A1:** GitHub's `workflow_call` rejects a boolean for a `string`
  input. *Method:* not directly testable from this repository; mitigated by
  passing the documented string form and by the contract test pinning the
  *type*.
- **Axiom A2:** `setup-commands` runs before the baseline in each mutants job.
  *Method:* read from the pinned workflow's job steps during reconnaissance;
  re-checked at EP-M3 from the run log itself, where the `setup-commands`
  output appears before the baseline compile.
- **Axiom A3:** `shared-actions#544` (whether `setup-rust`'s `RUSTFLAGS` export
  displaces the project's linker flag) is an upstream question this repository
  cannot settle. *Method:* the printed flag values at EP-M3 are recorded as
  evidence for that issue rather than resolved locally.

### No new formal obligations

This change introduces no invariant over generated inputs and no contractual
business logic. The correct levels on the `rust-router` testing hierarchy are
the ones already chosen: a parameterized table for the finite set of failing
targets (EP-M2), and behavioural assertions over a parsed configuration for the
caller (EP-M1). No `proptest`, `kani`, or `verus` obligation is warranted, and
saying so explicitly is the honest discharge. The existing `proptest` in
`property_doctest_preserves_rustdoc_override_values` remains, with a 16-case
bound, because its subject (order-independence and byte preservation of a
generated flag string) genuinely spans inputs.

### Evidence to capture

- `make test-workflow-contracts` output (EP-M1), showing the new tests.
- Focused nextest output for `makefile_test_target` under a mold-free `PATH`
  (EP-M2), showing the 2 rustdoc tests passing and no `check-build-tools`
  diagnostic.
- Focused nextest output for `build_tools_check_tests` with the new `doctest`
  case (EP-M2).
- The `scrutineer` gate bundle (EP-M3).
- The run URL, `check-build-tools` line, baseline result, mutant count, and
  printed flag values (EP-M3).

## Idempotence and recovery

Every step is re-runnable. The workflow edit is declarative. The Rust test
edits are pure test code with no side effects outside their temporary
sandboxes. The local evidence runs create only `target/` and `~/.cache` state.
The `PATH`-restricted runs use a sub-shell `PATH` assignment, so they cannot
leak into the caller's environment. If a focused run is interrupted, re-running
it is safe. The only destructive-sounding step, "move `mold` aside", is
implemented as a `PATH` restriction and never moves a file.

## Interfaces and dependencies

No public interface changes. No new dependencies. The Rust change reuses the
already-public `test_support::build_tools` surface (`Sandbox`, `MakeInvocation`,
`combined`, `pinned_mold_version`, `pinned_toolchain`); the Python change
reuses `workflow_loading`'s `load_workflow`, `require_mapping`, and the
module's existing `_mutation_job`. `RecordingCargo` is deliberately **not**
used in the rustdoc tests: the existing bespoke fake Cargo records exactly the
three fields the assertions need, whereas `RecordingCargo` records a different,
larger schema.

## Revision note

Initial draft, 2026-10-12. Established by reconnaissance that (a) the failure
reproduces locally under a mold-free `PATH` and (b) the existing pin already
exposes `install-mold`, so no pin bump is required — contrary to the issue's
conditional. Added `Axiom A3` and the linker-selection observation after
reading the shared workflow's own guide, which states that a mold flag in
`.cargo/config.toml` does not apply when `setup-rust` exports `RUSTFLAGS`.
