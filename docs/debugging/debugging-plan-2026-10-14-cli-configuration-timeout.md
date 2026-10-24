# Debugging plan: CLI configuration compile timeout

**Generated:** 2026-10-14. **Issue:** The 11.2.1 ExecPlan's EP-M0 baseline
timeout. **Severity:** Blocks the implementation baseline. **Falsification
sub-agent:** `alchemist`. **Planning agent boundary:** The parent agent
prepared this document. The named sub-agent executes the falsification
experiment.

## Problem statement

The untouched-tree `make test` run terminated
`cli_configuration_fixture_compiles` after 300 seconds. Its public API fixture
should type-check within the normal test budget. The test copies a standalone
Cargo manifest and source into a temporary project, then runs `cargo check` with
`target/cli-configuration-ui` as its output directory. Other public API
fixtures use `compile_public_api_fixture`, which builds the workspace library
with the gate's features and reuses its artefacts.

## Context summary

- Baseline head: `6ee0ef09dd25882e06da4d0c2a186c0da7f02a39`.
- Current head: `7c8b48bdb4adacf8c44bc3b7aac022e0edbb8d3e`.
- No product source changes preceded the failed baseline.
- Nextest's default budget is five 60-second warning periods, with no retry.
- The failed run passed 3,348 tests before the fixture timeout.
- Shared build admission was healthy but contended after the failure.
- On 2026-10-14 the shared pool is still healthy and saturated.

### Error artefacts

```plaintext
TIMEOUT [300.011s] command_env_ui_tests::cli_configuration_fixture_compiles
make: *** [Makefile:232: test-nextest] Error 100
```

The baseline log is the `-4.out` test log under `/tmp` identified in the
shell-selection ExecPlan's discoveries section.

### Information gaps

The baseline captured neither the child Cargo diagnostics before termination
nor its dependency resolution. The exact share of compile time, admission
waits, package-cache waits, and registry access is therefore unknown.

## Hypotheses

### H1: The isolated project rebuilds a redundant dependency graph

**Claim:** The separate target directory and `cargo check` build mode cause the
fixture to compile Netsuke's dependency graph during its timed test, instead of
reusing the already-built workspace library.

**Plausibility:** High. The harness explicitly selects a separate target, and
`cargo check` generates metadata rather than the gate's build artefacts. The
[Cargo check documentation][cargo-check] describes this distinction.

**Prediction:** A single unmodified fixture run will invoke compiler processes
for dependencies, with admission waits under current shared load. Its scratch
project may also resolve versions independently of Cargo.lock.

**Falsification test:** Execute only the existing fixture test binary once,
with the gate's Rust flags and wrappers preserved. If its previous binary has
been removed, reproduce only its child command: copy the two fixture files into
`target/<temporary-root>/cli_configuration_pass` and run
`cargo check --manifest-path <copied-manifest>` with the harness's dedicated
`CARGO_TARGET_DIR`. Do not build the entire integration-test binary merely to
reproduce this command. Trace child executions and waits with `strace`; inspect
generated Cargo.lock while it exists; record elapsed time and build admission
before and after. Use the existing target directory without deleting or
replacing its contents. Do not start another build, retry, bypass admission, or
run a repository gate.

**Expected negative result:** Cargo reports every dependency fresh and invokes
no dependency compiler; the observed delay is elsewhere. This decisively
falsifies redundant compilation as the immediate cause.

### H2: Cargo stalls while resolving the unlocked scratch project

**Claim:** The standalone fixture's missing lockfile makes registry or
package-cache access consume the timed test budget before compilation.

**Plausibility:** Medium. The fixture copies no Cargo.lock and does not use
`--locked` or `--offline`.

**Prediction:** The trace shows a prolonged delay before any dependency
compiler starts, and child diagnostics identify registry or package-cache
waiting.

**Falsification test:** Only after H1's experiment has completed, inspect its
captured trace and lockfile evidence. No additional build is required.

**Expected negative result:** Resolution completes promptly and the time is
spent compiling or queued for compilation. This rules out registry/cache access
as the dominant cause of the observed run.

## Recommended execution order

Execute H1's single controlled run. Return `falsified`, `not-falsified`, or
`inconclusive` for H1 with precise evidence. The parent then commissions H2's
read-only assessment if the same evidence leaves resolution uncertain.

## Termination criteria

Stop after the one fixture completes or an admission error prevents it from
running. A passing warm run alone does not establish cold-build reliability.
Report missing evidence explicitly; do not adjust timeout policy.

## Notes for the executing agent

Use the shared Cargo cache. Preserve `RUSTC_WRAPPER` and `RUSTDOC`. Run with
`RUSTFLAGS=-D warnings -Zthreads=8 -Clink-arg=-fuse-ld=mold` on this Linux
host, matching `make test`. Use the clean Git environment, including
`GIT_EXEC_PATH=/usr/libexec/git-core`, for child Cargo dependency fetches.
Capture logs and trace files under `/tmp`. Never interfere with unrelated
processes. Exchange the relevant source through the supplied context pack.

## Investigation progress

- H1 was not falsified: the controlled child command succeeded after
  1,418.27 seconds (23 minutes 37 seconds), emitting 263 dependency
  compile/check entries through the managed wrapper. Its redundant graph
  compilation extended far beyond the original 300-second budget.
- At approximately 540 seconds, 84 dependency build entries had been
  emitted and compile-slot waits were directly observed, with 48 to 57 requests
  ahead of several dependency compilers. The supervisor remained healthy and
  compiled directly while managed sccache was unavailable.
- The standalone Cargo.lock resolved 331 packages. Excluding the fixture
  package itself, 42 name/version pairs were absent from the workspace
  lockfile, including `serde-saphyr` 1.3.0 and `syn` 3.0.9. The independent
  resolution and separate metadata target both prevent gate artefact reuse.
- H2's dominant registry/cache explanation was falsified for this
  reproduction. The first dependency compiler request occurred after 265.37
  seconds: eight compiler metadata probes occupied 227.94 seconds, and five
  blocking package-cache lock waits occupied 31.75 seconds. The trace does not
  expose the internal cause of slow metadata probes, and it does not establish
  the timing of the historical failure.
- Logs and trace: `/tmp/netsuke-cli-timeout-h1.out` and
  `/tmp/netsuke-cli-timeout-h1.trace`. The lockfile difference is recorded in
  `/tmp/netsuke-cli-timeout-h1.lock-diff.json`.
- Repair: use the existing `NetsukeRlib` public API harness,
  keeping compile-pass coverage plus a missing-library compile-fail control.
  Forward Cargo's configured compiler wrapper for the fixture compiler and test
  that command construction with injected toolchain values. Retain the default
  300-second budget and no-retry policy.
- The isolated-target regression was observed red before the repair:
  `make test-workflow-contracts` with the regression filter reported one
  failure for `command_env_ui_tests.rs` and two passing ambient harnesses. The
  exact result is in that gate's `-8.out` log under `/tmp`.
- The first post-repair run passed workflow contracts (663 passed, two
  skipped), formatting, and typechecking. Clippy then rejected an assertion in
  the fallible fixture test and a shadowed wrapper binding. The test now
  returns a descriptive error for a mismatched compile result, and the binding
  has a distinct name. The remaining gates will resume on that corrected
  candidate.
- Post-repair full validation is pending. No feature implementation or
  green EP-M0 baseline is claimed from the diagnostic run alone.

## Follow-up: ambient build-module fixture timeout

The corrected candidate passed workflow contracts, formatting, typechecking,
lint, and documentation coverage (98.81%). The full test gate then timed out
`production_build_module_slice_has_expected_boundary` at 300.012 seconds: 3,352
tests passed, five were skipped, and 17 were not run. The repaired CLI
fixture's two cases were among those not run. The evidence is the
`-20261014-2.out` test log under `/tmp`, lines 4534 to 4546.

### H3: the ambient preparation command still incurs a timed build

**Claim:** `BuildSliceDependencies::build` incurs compiler preparation or
dependency compilation inside the timed test even though it reuses the ambient
target. Its `cargo build --lib --all-features` selects a narrower target graph
than the preceding all-target test build; that may change dependency feature
unification and artefact fingerprints.

**Prediction:** One execution of that exact Cargo child command shows non-fresh
compiler artefacts or substantial compiler metadata-probe/admission waits.
Fingerprint diagnostics distinguish target-graph changes from ordinary cache
freshness. A prewarmed successful run alone cannot establish the historical
timing because the timed-out run may already have created some artefacts.

**Falsification test:** The `alchemist` sub-agent executes only that
preparation command once, with the existing ambient target, flags, wrapper, and
shared cache. Capture Cargo JSON, fingerprint diagnostics, elapsed time, and a
process/flock trace. Inspect whether emitted artefacts are fresh and which
compiler requests consume time. Do not run a repository gate, retry, delete
artefacts, create a new cache, bypass admission, or compile the fixtures
directly. Verify there is no remaining same-worktree child build before
starting; if one remains, wait for its completion without signalling it.

**Expected negative result:** All artefacts are fresh, metadata probes are
prompt, and no substantial compiler/admission interval occurs. This falsifies
the claim for the current reproduction but leaves historical warming effects
explicitly inconclusive. If dependency variants differ, compare their recorded
features with the preceding test-build variants before proposing a repair.

The parent planned and recorded this experiment; `alchemist` executed it.
Timeout and retry policies remain unchanged.

### H3 result and repair

H3 was not falsified: the exact command completed in 178.15 seconds and emitted
335 compiler artefacts, of which 26 were non-fresh. The trace recorded 26
actual compiler-wrapper requests and no compiler metadata probes. It used
`mockable` without `mock` and `monotony` without `test-util`; matching-profile
and matching-Rustflags fingerprints also existed for those test-enabled
variants. The precise historical contribution remains unknown because the
failed run may already have warmed part of the narrower graph.

The result and JSON are in `/tmp/netsuke-ambient-ui-h3.json` and
`/tmp/netsuke-ambient-ui-h3.cargo-json`; the trace is in the corresponding
`.trace` file. Cargo replayed some compiler diagnostics for fresh artefacts:
count actual traced launches, rather than replayed wait messages, as evidence
of current compiler work.

The extended workflow regression was observed red: its single selected test
failed because the existing shared arguments covered only `--all-features`. The
repair now shares `--workspace --all-targets --all-features` and the root
manifest across the three existing ambient harnesses. This preserves the
test-enabled dependency feature graph; it changes neither package verification
nor the timeout policy. The full corrected candidate still needs validation.

## H4: Cargo build mode still differs from Nextest's preparation

The next corrected run passed all pre-test gates, but the module-slice test
timed out again at 300.037 seconds. It also observed an empty Bash completion
file in `generated_completion_exposes_the_clap_command_tree::case_1`. The
2.871-second passing case from the same UI binary was a different test; it is
not evidence that the timed-out boundary check passed. The final summary
reports 2,147 passed, one failed, one timed out, five skipped, and 1,221 not
run.

**Claim:** Matching workspace, targets, and features still leaves an important
command-mode mismatch: the harness uses `cargo build`, whereas Nextest prepares
test artefacts through `cargo test --no-run`.

**Prediction:** A traced, build-only Nextest query shows a
`cargo test --no-run` child, rather than the `cargo build` used in the ambient
harnesses.

**Falsification test:** `alchemist` traces one `cargo nextest list` query for
only `netsuke-build`'s `build_module_slice_ui_tests` binary, with all features
and binary-only output. This query must not execute tests or select all
targets. Preserve the existing cache, target, wrappers, flags, and clean Git
environment. Capture process executions and Cargo output; inspect the actual
build command. Do not retry, run a full gate, or change source. If a
same-worktree build remains from the failed gate, wait without signalling it
before starting.

**Expected negative result:** Nextest actually uses the same `cargo build`
command mode. This falsifies the proposed mismatch. A differing mode
establishes the mismatch, but does not by itself prove its share of the
historical timeout.

The parent plans this probe; `alchemist` executes it. Also inspect the existing
completion-generator and test scheduling evidence read-only: their files share
the target/profile staging directory, and the completion reader is outside the
nested-build group. Any generation race remains a separate hypothesis until the
producer's write semantics and scheduling overlap are established.

## H5: completion readers race with nested generation

**Claim:** The generated-completion contract can read a truncated file while an
ambient UI preparation reruns the build script. Both use the same package's
target/profile staging directory, and the reader is not serialized with nested
builds.

**Prediction:** The production generator publishes its shared staging path, the
selected `clap_complete` implementation truncates final files before writing,
and Nextest assigns the reader outside the nested-build group.

**Falsification test:** A separate `alchemist` performs a read-only assessment
of `build.rs`, `tests/completion_contract_tests.rs`, `.config/nextest.toml`,
and the resolved local `clap_complete` source. Check the producer's write
semantics, the reader's path, and exact group-filter membership. Use the
existing failure log as symptom evidence; run no command that builds, formats,
lints, or executes tests. No source changes, instrumentation, or retries are
needed.

**Expected negative result:** Writes are already atomically published, paths
are isolated between producers and consumers, or both tests share an exclusive
group. Any of those falsifies the proposed concurrent-read mechanism. If the
structural hazard exists, state that the assessment cannot prove the exact
historical scheduling interleaving without a captured file-operation trace.

The parent plans H5; the separately assigned `alchemist` executes this bounded
source assessment. H4 alone owns the build-only compiler experiment.

## H4 and H5 results and next repair

H4 was not falsified: the binary-only probe completed successfully after 213.03
seconds. Its trace shows a separate metadata query, followed by
`cargo test --no-run` with the selected package, test binary, and features.
There was no test-binary execution. The exact command appears at line 162 of
`/tmp/netsuke-nextest-mode-h4.trace`; its result is in the corresponding
`.json` file. This establishes the remaining command mismatch, without
assigning a historical duration to it.

H5 was not falsified as a structural hazard. The resolved `clap_complete`
4.6.10 implementation opens each final output with `File::create` before
writing in `src/aot/generator/mod.rs`, lines 245 to 248. Production publishes
the shared target/profile directory, while the completion tests read it outside
the exclusive nested-build group. The empty Bash content fits that hazard, but
the exact historical interleaving was not traced.

Three additional regressions were observed red: mandatory `--no-run`
preparation, group membership for completion readers, and discovery of Cargo
test preparation as build-capable. The exact result is the workflow-contract
`-20261014-4-red.out` log under `/tmp` (three failures).

The existing ambient harnesses now use `cargo test --no-run` with the gate's
workspace, targets, and features. The existing group also includes the five
parameterized completion readers, keeping them mutually exclusive with nested
generation. The scanner recognizes `cargo test` so a future preparation cannot
evade that policy. No build-script or product behaviour is changed. Full
validation remains pending.

The first workflow validation after this repair rejected Rustfmt's line break
between the shared constant's assignment and array. The contract parser now
accepts formatting whitespace while retaining the exact argument checks.

## H6: packaging verification performs an intentional cold build

The next full run passed 3,369 tests, including both CLI fixture cases (17.352
and 31.720 seconds), the module-slice boundary (277.435 seconds), and all five
completion readers. Its sole remaining failure was packaging smoke at 300.017
seconds. Doctests did not start. This is a distinct boundary: unlike public API
fixtures, packaging must verify the extracted published crate with its shipped
default features.

**Claim:** The packaging smoke test's newly created target directory forces a
cold verification build. Its five-minute budget assumes the cached Linux build
described in the test's comment and is insufficient with uncached, admitted
compilation under shared load.

**Prediction:** An equivalent publish dry run with a fresh workspace-local
target emits actual dependency compiler requests and takes more than 300
seconds while admission remains healthy.

**Falsification test:** `alchemist` runs the packaging child once, using the
same platform arguments and flags, but places its fresh target under the
workspace's `target/` directory. This location adjustment is mandatory because
the agent instructions forbid using `/tmp` as a build target. Keep the shared
Cargo cache, wrappers, and clean Git environment; capture process executions,
elapsed time, and admission state. Run only the dry-run publish command, not
the package-list assertion or any test suite. No real publish, retry, cache
cleanup, service management, or source change is authorized. Wait for any
remaining same-worktree child before starting without signalling it.

**Expected negative result:** The fresh verification graph invokes no
dependency compiler or completes promptly; the proposed cold-build bottleneck
is then falsified for this reproduction. Report any unrelated verification
error separately instead of treating it as evidence for the duration claim.

The parent plans H6; `alchemist` executes it. Preserve the isolated shipped
package verification. If this intentional cold build exceeds the allowance,
record its measured duration before choosing a packaging-only budget, following
the compile-admission instruction to allow longer compilation timeouts. Keep
the default timeout for ordinary tests and the no-retry policy unchanged.

## H6 result and scoped repair

H6 was not falsified with high confidence. The verification dry run succeeded
in 507.99 seconds, with 264 Cargo compilation entries and 336 actual wrapper
requests. Admission remained healthy; sccache was unavailable. There were no
explicit slot-wait diagnostics, so this does not quantify queue time. The
upload was explicitly aborted by dry-run mode. Evidence is retained in
`/tmp/netsuke-package-cold-h6.json`, `.trace`, and `.err` (lines 712 to 714).

Two regression guards were observed red in the workflow-contract
`-20261014-7-red.out` log: missing packaging allowance and system-temporary
build placement. Packaging now creates its isolated target beneath the workspace
`target/` directory, retaining extracted-package verification and shipped
defaults. Only that test receives eleven 60-second periods (660 seconds),
leaving 152 seconds above the measured build and remaining below CI's
780-second whole-run budget. Ordinary tests retain 300 seconds, and no retries
or outer CI budgets change. This is an evidence-based exception for an
intentional cold build, rather than restoring redundant public API builds. Full
Rust validation subsequently passed: 3,370 Nextest tests, five skipped, 39
doctests, and six ignored doctests. Packaging completed in 385.821 seconds; the
CLI cases took 15.715 and 17.049 seconds. The module boundary passed in 285.271
seconds, leaving limited margin under heavy shared-host load; this run
establishes a pass, not a guaranteed future duration. Workflow guards passed
667 tests, with two skipped; lint and documentation coverage passed. Logs use
`-netsuke-11-2-1-20261014-9.out` under `/tmp`. Markdown validation initially
rejected the removed fixture manifest because it remained in Git's index.
Staging the intended deletion resolved that scan; formatting, spelling,
Markdown lint, and Mermaid validation then passed in the `-20261014-10-doc.out`
logs. Local review and publication evidence belong to the repair delivery
ledger and PR.

## Standalone prerequisite integration

On 2026-10-14 the user requested this repair as a separate prerequisite PR under
[PR #778](https://github.com/leynos/netsuke/pull/778). The prerequisite targets
`main`; #778 targets the prerequisite. The original combined candidate is
preserved at `refs/recovery/timeout-stack-20261014/original-head`.

The repair is integrated onto main commit `1a5fc57f`, retaining its mutation
compile allowance (600 seconds), module-boundary checks, and module-qualified
filter guards. Packaging alone changes from main's 600-second allowance to the
measured 660-second allowance. The ordinary 300-second budget, no-retry policy,
and 780-second CI run budget remain. Existing timeout documentation and the
regression guard now account for both targeted exceptions. The obsolete
standalone manifest is removed despite its intervening package-version update.

Historical gate evidence above belongs to the original combined candidate;
fresh standalone validation was run after this integration. Its first full
suite exposed current main's Dependabot directory contract: deleting the
obsolete fixture manifest also requires removing its Cargo update directory from
`.github/dependabot.yml`. The contract was observed red before that entry was
removed; the root and `test_support` update directories remain. The feature's
scope is evaluated in its own PR diff, excluding the prerequisite repair.

## Fresh current-main validation

The standalone candidate passed formatting, typechecking, lint, documentation
coverage (98.87%, 4,988/5,045), and workflow contracts (1,271 passed, three
skipped; 27 additional script tests passed). The final full Rust gate passed
3,934 Nextest tests, with six skipped, and 129 doctests across three targets,
with 32 ignored. The CLI cases completed in 9.849 and 5.959 seconds, the module
boundary in 151.141 seconds, and packaging in 167.123 seconds.
Markdown/spelling and Mermaid validation also passed. The full-run evidence is
in `/tmp/*-netsuke-timeout-stack-20261014-3.out`; workflow contracts and
unchanged-Rust typechecking use the corresponding `-1.out` logs, and
unchanged-Rust doc coverage uses `-2.out`.

[cargo-check]: https://doc.rust-lang.org/cargo/commands/cargo-check.html

## Current-main integration note (2026-10-16; historical candidate)

That integration preserved the CLI fixture's artefact-reuse repair: it prepares
against the workspace lockfile, target, and gate-matching
`cargo test --no-run --workspace --all-targets --all-features` selection, then
checks the fixture against the gate-built library. It remains serialized with
other child-Cargo work, and at this integration step its allowance was the
ordinary 300 seconds on the reasoning that it no longer resolves and compiles a
separate cold graph.

That reasoning was later falsified. Reusing the gate's *selection* is not the
same as reusing a warm build tree: `NetsukeRlib::build()` runs a check-mode
`cargo check` whose graph neither `cargo nextest run`'s `cargo test --no-run`
preparation nor `make typecheck`'s workspace-less `cargo check` satisfies, so
the child build is cold whenever a gate starts. Measured on 2026-10-17, the
child command took 455 s against an ambient target with no `.fingerprint` or
`deps/`, 1 s when immediately repeated, and the fixture was stopped at
`TIMEOUT [300.024s]`. The ten ambient fixtures share that cost and the
`nested-cargo-builds` group serializes them, so the allowance now sits on their
shared override at 900 s rather than on this one test. See "Test timeouts: the
tiers this repository sets" in `docs/developers-guide.md`.

The earlier 660-second packaging allowance and 780-second whole-run budget
described above were measured for the original standalone candidate. Current
main has since adopted a 900-second packaging allowance and a 960-second CI
whole-run cap; this integration retains those current-main values. The
current-main timeout policy is 300 seconds ordinary, 600 seconds for mutation
compilation, 720 seconds for the production-module check, and 900 seconds for
the ambient public-API fixture family, the stub-environment, split-build, and
packaging checks. The CI watchdog remains 1,980 seconds and the job limit
remains 100 minutes. At this integration step, fresh full gates were pending;
the historical measurements and validation above apply to their recorded
candidates only.

## Windows diagnostic follow-up (2026-10-16)

The original Windows run failed nested compilation after 45.959 seconds; it was
not a timeout. The harness discarded the compiler diagnostic because Cargo
emitted it on JSON stdout, while the failure included only stderr.
JSON-producing fixture commands now select `json-render-diagnostics`, retaining
artefact JSON while sending diagnostics to stderr. A deliberate compiler-error
case guards that boundary alongside the existing successful split-build case.

The nested command selects the running integration-test target, but the log
does not establish an executable replacement collision or the reason Cargo
invalidated the artefact. The exact Windows cause remains unconfirmed until a
new hosted run provides its diagnostic. Moving preparation before test
execution is a proposed systemic follow-up, not an implemented change or a
demonstrated explanation of that historical failure.

An isolated no-dependency probe under the pinned nightly confirmed the stream
contract: both commands failed as intended, with the sentinel on JSON stdout for
`json` and on stderr for `json-render-diagnostics`. Subsequent full Rust
validation was blocked by compiler admission: the supervisor refused launches
while requests to create compiler scopes timed out. Those results do not
establish a source failure or a passing Rust suite.

## Production-module timeout follow-up (2026-10-17; previous candidate)

On the resumed repaired candidate, `make check-fmt`, `make lint`, and
`make typecheck` passed. `make doc-coverage` reported 98.89% (5,074/5,131), and
its tool tests passed 65 cases. `NETSUKE_REQUIRE_NINJA=1 make test` failed when
`production_build_module_slice_has_expected_boundary` reached its 720.034
seconds limit: 3,971 passed, one timed out, six were skipped, and 23 did not
run. See `/tmp/test-netsuke-913-repaired.out`, lines 4531–4545. Compiler
admission was healthy. Its nested
`cargo test --no-run --workspace --all-targets --all-features` used the gate's
target and flags but still compiled many workspace binaries.

Fingerprint inspection found that the outer gate's
`target/debug/build/netsuke-build/*/fingerprint/run-build-script-build-script-build.json`
recorded `CARGO_PKG_VERSION`, `CARGO_PKG_DESCRIPTION`, and `CARGO_PKG_AUTHORS`
as absent, while the latest nested fingerprint `a05ef5cfbe344bd2` recorded
their injected values. The failing candidate emitted `rerun-if-env-changed` for
those values, `TARGET`, and `PROFILE`. Cargo documents that these directives
compare the environment of Cargo invocations; they do not watch values Cargo
supplies only to the build script
([build-script reference](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-env-changed)).

The four-build canary ran under the pinned nightly and every invocation exited
successfully. With the tracked watcher, the transition from an unset
`CARGO_PKG_VERSION` to `0.1.0` marked the build script dirty (log line 4) and
ran it (line 6). Without the watcher, the matching-value build stayed fresh
(line 3) and did not run the build script; its warning text was cached output
rather than another build-script execution. The logs are
`/tmp/netsuke-cargo-metadata-canary-tracked-unset.log`,
`/tmp/netsuke-cargo-metadata-canary-tracked-matching.log`,
`/tmp/netsuke-cargo-metadata-canary-control-unset.log`, and
`/tmp/netsuke-cargo-metadata-canary-control-matching.log`. This supports the
invalidation mechanism with high confidence. It does not identify the exact
first invalidation in the historical Windows run; the local fingerprint is
compatible evidence, not a trace of that first cause.

The repair removes the five watchers for Cargo-owned package metadata, target,
and profile values. It relies on Cargo's manifest and build-configuration
tracking for those inputs, retains the explicit `SOURCE_DATE_EPOCH` watcher,
and keeps manifest-file tracking. The new regression module,
`tests/support/cargo_metadata_reuse.rs`, adds a parameterized `cargo_metadata`
split-fixture case using the existing workspace build helper. It checks that
inherited Cargo metadata leaves the build fresh and that changing
`SOURCE_DATE_EPOCH` triggers a rebuild. At this pre-validation checkpoint, the
regression was implemented and the canary supported the mechanism. The
candidate still required sequential gates; the canary did not prove the exact
first invalidation in the historical Windows run.

## Windows link failure and metadata-only preparation (2026-10-17)

Published head `e212ec5d` had passed its full local gates. A subsequent Windows
job failed after 102.015 seconds in
[CI run 39004145794](https://github.com/leynos/netsuke/actions/runs/39004145794/job/117313036522).
The nested `cargo test --no-run` compiled `ring`, `rustls`, `ureq`, and the
workspace integration target before `link.exe` reported LNK1104: it could not
open `build_module_slice_ui_tests-98286508330a53df.exe`. A separate Nextest
execution of `release_help_documentation_metadata_is_stable` reported OS error
32, “file in use”. The log does not identify the handle owner or current
running path, so the precise collision and its first cause remain unknown. This
failure does not establish that build-script invalidation caused the link error.

CodeRabbit's
[investigation response](https://github.com/leynos/netsuke/pull/913#issuecomment-6229977574)
confirms metadata-only preparation as the smallest systemic repair. It also
identifies `ring 0.17.14`'s own Cargo environment watchers as a
source-supported additional invalidation mechanism. The job contains no
first-dirty trace, so this does not establish that `ring` was the first
invalidated unit or explain the executable link failure.

The source audit found that these ambient helpers need Cargo metadata and
dependency artefact paths for their direct `rustc --emit=metadata` checks; they
do not need Cargo to link workspace test executables. The repair changes their
preparation to
[`cargo check`](https://doc.rust-lang.org/cargo/commands/cargo-check.html) with
`--workspace --all-targets --all-features` and the same target selection, while
removing `--no-run`. Cargo check omits final code generation, but can still
compile metadata and run build scripts, so this change does not promise that
Cargo performs no work or eliminate the need for the nested-build group. The
isolated split-build fixture and packaging verification remain unchanged; the
timeout budgets and no-retry policy remain unchanged as well. At this
pre-validation checkpoint, the repair still required its sequential gates.

Focused CodeRabbit confirmation withdrew the mandatory-manifest concern absent
a demonstrated failure
([comment](https://github.com/leynos/netsuke/pull/913#issuecomment-6229870394)).
The subsequent investigation request received the response linked above; its
mechanism assessment does not resolve the first-dirty uncertainty.

## Focused metadata-selector ambiguity (2026-10-17)

For the first unpublished check-mode candidate, `make check-fmt` and 52
selector tests passed. The focused production boundary test then failed after
140.199 seconds because the artefact selector found two matching Camino 1.2.6
metadata paths from the same package, source, and manifest identity. Read-only
inspection found that `cfc87cce273ebe8c/out` contained both `.rmeta` and
`.rlib` outputs for a code-generation/build-dependency unit, while
`fdf014ee1ef30f75/out` contained only `.rmeta` for the check unit. Both used
the same features and flags. The lockfile also has direct build dependency
`thiserror` 1.0.69 and transitive `thiserror` 2.0.20; display versions
therefore cannot safely select artefacts.

The in-progress selector refinement prefers metadata-only check messages
(`.rmeta` without `.rlib`) over code-generation messages, scans the full stream
before considering the fallback, and rejects ambiguity within the selected
class. It continues to reject mismatches in package or source identity. The
production-module fixture also needs exact build-dependency package IDs, so its
narrow `cargo_build_dependency_ids` helper queries the locked, all-features
Cargo metadata graph and resolves root build dependencies by `kind = build`.
Those opaque IDs match Cargo compiler messages exactly; the helper does not
parse displayed versions or hard-code dependency versions. It is used only by
`BuildSliceDependencies`; the other ambient consumers use the shared artefact
selector directly. The isolated split fixture retains its single-message helper.

At this refinement checkpoint, the candidate still required its sequential
gates.
