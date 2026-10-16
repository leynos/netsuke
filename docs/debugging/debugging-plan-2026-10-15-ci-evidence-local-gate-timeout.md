# Debugging plan: local CI evidence commit-gate timeout

Generated: 2026-10-15. Issue: #894. Severity: commit-gate blocker.
Falsification sub-agent: `alchemist`. The planning agent prepared this
document; the named sub-agent executes the experiment.

## Problem statement

The unchanged `production_build_module_slice_has_expected_boundary` test timed
out at 300.009 seconds during local `make test`. Nextest passed 3,895 tests,
timed out one and left 16 unrun through fail-fast. Six tests were skipped by
the existing selection. Compilation succeeded; doctests did not run. No
workflow, Rust, dependency, timeout or worker configuration was edited.

| Context              | Value                                                        |
| -------------------- | ------------------------------------------------------------ |
| Local source base    | `c65ad9709d1d7cc46c094d88b2eecd1deea8f4c8`                   |
| Nextest              | `0.9.133`, matching the CI pin                               |
| First observation    | 15 October 2026; one failed full local attempt               |
| Tested merge SHA     | None: this was local validation, not a CI merge run          |
| Working-tree changes | Offline Python evidence tooling, contracts and documentation |

The failure log is `/tmp/test-netsuke-894.out`, lines 4827–4845. The harness
captures child Cargo output, so the timeout supplies no Cargo diagnostic.
Shared compiler admission was healthy and busy during preceding gates, with
sccache unavailable. That observation does not establish the timeout's cause.

## Information gaps

The original child's phase, freshness, compiler activity and lock waits were
not captured. Its partial compilation may have changed the local build tree. A
subsequent success cannot reconstruct those missing observations or prove a CI
speedup. Keep the failed gate in the validation record.

## H1: Cargo preparation still requires fresh compilation

Claim: the fixture's `cargo build --lib --all-features` preparation currently
needs non-fresh compiler artefacts under the exact Makefile gate flags.

Initial plausibility: medium. Building test executables does not by itself
prove that the helper's library-only Cargo invocation is fresh.

Prediction: a single equivalent Cargo invocation emits at least one native
`compiler-artifact` message with `fresh: false`. Record wall time and native
lock/admission messages separately; non-fresh artefacts alone do not prove
eight cold builds or explain the original 300 seconds.

Falsification: the command succeeds with zero non-fresh artefacts. This rules
out required recompilation in the current state only; it leaves original-run
lock waits and fixture-rustc behaviour unresolved. Failed or incomplete output
is inconclusive.

## Execution plan

1. Create a temporary Makefile fragment outside the repository. Include the
   repository Makefile and define one diagnostic target depending on
   `check-build-tools`. Its sole build recipe is:

   ```makefile
   $(GATE_RUSTFLAGS) $(CARGO) build --lib --manifest-path $(CURDIR)/Cargo.toml --message-format=json --all-features
   ```

2. Run that target once from the existing worktree using the clean client
   environment. Preserve Makefile PATH, compiler wrappers, the shared Cargo
   cache, target layout and concurrency. Capture stdout/stderr under `/tmp`.
3. Summarize exit status, elapsed time, fresh/non-fresh compiler-artefact
   counts, observed lock/admission markers and the verdict. Do not copy raw
   errors or secrets into documentation. Run no fixtures or full gates in this
   experiment.

Do not edit tracked files, clear caches, raise timeouts, bypass admission or
signal other processes. Stop after the one invocation. Report falsified,
not-falsified or inconclusive, explicitly separating current-state evidence
from the unobserved cause of the earlier timeout. The parent agent decides the
next diagnostic or gate action after reviewing that evidence.

## Outcome

The single invocation succeeded in 266.773 seconds. It emitted 335 native
compiler-artefact records: 327 fresh and eight non-fresh. Four package-cache
lock messages occurred; their individual durations were not measured. Cargo
reported completion in four minutes 24 seconds. H1 is not falsified for the
current state. These are artefact counts, not eight cold fixture builds or
counts inferred from repeated compiler-wrapper messages.

The diagnostic completed the outstanding library preparation. Rerun the full
local gate with unchanged selection, wrappers, workers and timeouts, retaining
the first failure and this preparation in the validation history. A passing
rerun does not establish the earlier timeout's cause. This is local validation
recovery, not a candidate benchmark, implementation change or optimization
adoption. Raw evidence remains under `/tmp/netsuke-894-h1-*`.

During the unchanged full-gate retry, the boundary fixture passed in 102.301
seconds. Its original timeout remains unexplained; the complete gate result
then failed on the real Cargo-consumer fixture at 300.008 seconds: 3,896
passed, one timed out, six skipped and 15 unrun. Further full-gate retries are
paused for diagnosis. No fixture, assertion or timeout was changed.

## H2: The active real-consumer fixture is waiting on a cache lock

The retry has reached `cli_configuration_fixture_compiles`. Claim: at the
snapshot instant, its child Cargo waits on a shared package-cache file lock.
Initial plausibility is medium because H1 observed that mechanism.

Prediction: the test's Cargo descendant has a queued `/proc/locks` entry
matching a default Cargo package-cache lock inode. Falsification: a snapshot
instead shows active compiler descendants and no matching queued lock. A
finished fixture, missing permissions or an uncorrelated wait is inconclusive.

The `alchemist` may take one read-only snapshot of the active test and its
descendants, their process roles/wait states and corresponding file-lock
entries. Restrict output to this worktree's test tree and relevant Cargo cache
lock inodes. Do not output unrelated command lines or environments. Run no
additional build or fixture, attach no debugger, change no file and signal no
process. This observes an existing local gate without duplicating execution. It
cannot establish wait duration or retrospectively explain the first timeout.

Outcome: inconclusive. At 11:35:21 UTC, the consumer Cargo held an unqueued
read lock on `.package-cache-mutate`; queued package-cache locks belonged
outside its process tree. Build-limit wrapper descendants were waiting, but no
compiler descendant was observed. Neither the predicted queued lock nor the
strict falsification condition was observed. The snapshot is
`/tmp/netsuke-894-h2-snapshot.json`; unrelated selector matches were excluded.

## H3: The real Cargo consumer currently has a compilation defect

Claim: the unchanged consumer's Cargo check fails because its source or
configuration is incompatible with this implementation base. Initial
plausibility is low: the test never produced a compiler error, but its output
was captured until Nextest terminated it.

Prediction: the exact real-consumer check completes with a non-zero compiler
result and diagnostic. Falsification: it succeeds with the original manifest,
source, features, toolchain and build layout. Infrastructure refusal or a
command that cannot complete is inconclusive. Success does not explain the
historical timeout or establish a performance improvement.

After the gate runner finishes typecheck and pauses, `alchemist` may execute
one consumer preparation:

1. Create a temporary source root under the repository's `target`, then the
   `cli_configuration_pass/src` child layout. Copy the two fixture files from
   `tests/ui/cli_configuration_pass` verbatim and verify their digests.
   Preserve the three-level relative Netsuke dependency in the manifest.
2. Use a temporary Makefile fragment including the repository Makefile.
   Depend on `check-build-tools`, use `$(GATE_RUSTFLAGS)` and `$(CARGO)`, and
   set `CARGO_TARGET_DIR` to the existing `target/cli-configuration-ui` exactly
   as the unchanged helper does. Run `cargo check --manifest-path` on the
   copied project once, adding only `--message-format=json`. Preserve its
   default features; do not add `--all-features`.
3. Capture exit status, elapsed time, native freshness and lock/admission
   messages under `/tmp`. Remove only the source root created by this probe
   after completion. Keep the shared Cargo cache and existing build output.

No full gate or test runs in this experiment. Preserve wrappers and resource
limits, wait naturally for admission, and change no tracked file, timeout or
assertion. This diagnostic may exceed Nextest's test duration because it is not
a gate result; the final gate must still run with its original limits.

Outcome: falsified with high confidence. The faithful consumer check succeeded
in 691.538 seconds, with 403 compiler artefacts: 186 fresh and 217 non-fresh.
Its copied files matched the originals, and the relative dependency resolved to
this worktree. Native output recorded three package-cache lock messages and 43
compile-slot wait messages; their durations were not measured. No compiler
error or admission refusal occurred.

The probe retained the existing consumer build output and removed only its own
temporary source root. Evidence is in `/tmp/netsuke-894-h3-summary.json` and
the corresponding provenance, stdout and stderr files. This rules out the
current compatibility-defect claim; it neither explains the earlier timeout nor
passes the gate. Resume the failed gate with its original limits and retain
both failed attempts and both preparation costs in local validation history.
These local diagnostics are outside the CI baseline dataset.

## H4: Packaging still times out with a smaller external admission queue

The third unchanged full gate passed 3,911 tests and timed out the final
packaging verification at 300.028 seconds. The consumer passed at 198.270
seconds, the split-layout fixture at 255.989 seconds and the module boundary at
99.072 seconds. Nextest reported 1,046.954 seconds; doctests did not run. The
failure log is `/tmp/test-netsuke-894-final-third.out`. Its captured Cargo
output does not identify the interrupted child phase.

During that gate, a read-only supervisor status showed a healthy, occupied
four-slot admission pool, queued requests and unreachable sccache. This is a
concurrent observation, not a measurement of the packaging child's total wait.
The crate's explicit package inclusion list excludes the new evidence files.

Claim: the unchanged packaging test still reaches its 300-second timeout when
started with no more queued requests than the admission pool's capacity.
Initial plausibility is medium; it creates an isolated verification target, so
the earlier consumer preparation cannot warm that target. Prediction: its
original Nextest limit terminates it again. Falsification: the selected test
passes within that original limit. A compiler error, supervision failure or
renewed queue growth makes the duration hypothesis inconclusive. A pass does
not establish the cause of the earlier timeout or a CI improvement.

After documentation gates finish and the parent explicitly releases the test
slot, `alchemist` may perform one minimal experiment:

1. Observe supervisor status without changing it. Wait for two observations
   at least 30 seconds apart with a healthy pool and queued-request count no
   greater than capacity. Preserve the status in temporary logs. Never signal
   competing builds or change cache, admission or service configuration.
2. Use a temporary Makefile fragment including the repository Makefile and
   depending on `check-build-tools`. Run exactly one Nextest invocation with
   the existing workspace/all-target/all-feature selections, Makefile flags,
   wrappers and worker limits. Add only the anchored test filter
   `test(/^packaged_manifest_retains_build_script_sources$/)`. Keep the default
   profile and its 300-second limit; assert that one test is selected.
3. Capture output and elapsed time under `/tmp`, then take a final supervisor
   observation. Report the test result, queue observations and verdict. Make no
   tracked edits and run no full gate or second test invocation.

This is a local diagnostic with a reduced selection, not a passing full gate, a
controlled CI observation or authorization to relax packaging fidelity. The
parent reviews the outcome before authorizing further validation.

As of 15:10 UTC on 15 October, 299 read-only observations had not produced a
qualifying pair. One healthy reading at 15:03:12 showed zero queued requests;
31 seconds later the queue was 321. No diagnostic test had launched. This
records an unavailable observation condition, not a verdict about packaging
behaviour or the earlier timeout's cause. These local observations remain
outside the CI baseline dataset.

## H4 outcome and later environment observation

The monitor collected 424 initial observations and 76 continuation
observations, ending at 16:57:23 UTC on 15 October. No two qualifying readings
were obtained and no diagnostic test launched. H4 is therefore inconclusive;
the original prediction was not tested.

A separate read-only status on 16 October showed a healthy four-slot admission
pool with three slots held and sccache unreachable. The managed
`buildlimits-sccache.service` is enabled but inactive/dead. Systemd reports it
inactive since 12 October, with `Result=success`, `ExecMainStatus=15` and zero
restarts. This does not establish that the service state caused the local test
timeouts. No service, configuration or admission-limit changes have been made.
Explicit user approval is pending before the proposed recovery command:
`systemctl --user start buildlimits-sccache.service`. Treat these as local
environment observations, separate from the CI baseline.

On 15 October, ordinary required hosted checks passed on tested merge
`0ced8474e03ba6ec22d45abf8d81145e90d41a35`, which included newer `main` at
`486b876c7244c81b4fa9787a990448beebafea26`. That merge had 78 changed paths
relative to the documentation PR head, including Rust, tests and Nextest
configuration. Hosted Nextest reported 3,928 passes and six skips; doctests
reported 129 passes and 32 ignored. Its packaging test passed in 161.125
seconds with a 600-second allowance. The local full gate selected 3,912 tests
and timed out packaging at 300.028 seconds. These runs do not have equivalent
source, configuration, selection or timeout conditions, so hosted success does
not waive the local gate or identify the earlier timeout's cause.

A retained run provides a narrower comparison: for run `38276341866`, job
`114902625996`, the tested merge `1d4bbde6d01713062621c7b58aa5441aaa97197e` has
no diff from the PR head across Cargo inputs, Rust source, build scripts,
locales or Cargo-included documentation/assets. Its full diff still differs in
CI/Kani tooling and `tests/workflow_ci.rs`; the native record reports packaging
at 169.864 seconds. This narrows package-input differences in that comparison,
but does not establish matching compiler, cache or runner conditions or explain
the local timeout. The native journal window from 11–13 October contains no
cache-service entries, so the service stop reason remains unknown.
