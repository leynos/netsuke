# Import shared prover installers and variadic proof execution

Status: DRAFT

This ExecPlan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current throughout execution.
Implementation awaits approval; this pull request proposes the work only.

## Purpose / big picture

Netsuke maintainers should install the pinned Kani and Verus releases through
one shared tool, locally and in continuous integration (CI). CI should stop
carrying its own Kani downloader. `make verus` should verify Netsuke's actual
proof by default and accept several explicitly ordered proof files through
upstream's repeatable `--proof-file` option. A failing proof must stop the
batch and remain a failure at the Make boundary.

Success means a cold Kani CI run downloads verified prebuilt archives, a warm
run reuses verified archives, and both execute the existing proof and mutation
checks. Verus remains optional. No application behaviour changes.

## Constraints

- Pin the shared tool to a full commit, never a moving `main` reference.
  Retain Kani 0.67.0 and the existing Verus version and checksum files.
- Preserve the Kani scope decision, required-check reporting, metrics,
  mutation compilation, resource wrapper, job timeout and trunk-only cache
  writes. Keep the pinned nightly selection for mutation compilation.
- Do not compile Kani from source in CI or weaken archive verification. Do not
  add a second downloader, an installation fallback, or a compatibility shim.
- Keep `RUSTC_WRAPPER` and `RUSTDOC` admission intact. Use the shared Cargo
  cache locally; preserve the workflow's existing isolated homes on CI.
- Keep prover acquisition outside Netsuke's Rust domain modules. Preserve
  existing proof statements, harness bounds and production code.
- Retain shared CI archive helpers while actionlint consumes them. Do not
  migrate unrelated installers, add a mandatory Verus CI job, or change
  `formal-pr` to install tools implicitly.
- Gate every commit; keep the plan synchronized with evidence. This draft
  changes only documentation and does not authorize execution by itself.

## Tolerances (exception triggers)

Stop and record a proposed deviation if a milestone requires changes outside
build tooling, proof integration tests, workflow configuration, and associated
maintainer documentation; a new runtime dependency; an upstream package patch;
a prover version change; or weakened checksum or cache trust policy. Escalate
after three unsuccessful fixes of the same failing contract. There is no time
limit: healthy compiler admission queues are not failures.

The implementation budget is 20 non-ExecPlan files and 800 net added lines of
code and other documentation. ExecPlan files are excluded from both limits.
Stop before exceeding either limit. Record options and affected requirements in
Decision log, set `Status: BLOCKED`, and obtain approval for the deviation.

## Risks

- High impact, medium likelihood: installation layout and PATH differences
  make the next workflow step execute an old binary. Pass explicit install and
  archive paths, publish the matching directory, and test a separate subsequent
  process with a conflicting binary later on PATH.
- High impact, medium likelihood: a warm cache contains old layout or stale
  toolchain overrides. Bump the cache generation, retain explicit nightly
  setup, and exercise cold, warm, and incomplete installations.
- Medium impact, medium likelihood: old environment variable names are
  silently ignored by the new CLI. Replace the workflow binding contract rather
  than retaining obsolete checksum inputs that no longer enforce pins.
- Medium impact, medium likelihood: Make or shell quoting splits proof paths
  or hides child failure. Test actual argument boundaries, ordered execution,
  and first/middle/last failures against the pinned CLI.
- Medium impact, low likelihood: package resolution or upstream downloads are
  unavailable. Separate network setup evidence from offline integration tests;
  report infrastructure failures without claiming successful proof execution.

## Progress

- [x] (2026-11-01) Inspect Netsuke's current Make targets, CI installer,
  workflow contracts, cache action and proof inventory.
- [x] (2026-11-01) Resolve and inspect upstream main at
  `592558f7183b554fc749696246984c5d7300d029` and draft this plan.
- [ ] Obtain implementation approval.
- [ ] EP-M1: Adopt the pinned shared installer in CI and remove Kani-only code.
- [ ] EP-M2: Wire explicit Verus proof selection and batch integration tests.
- [ ] EP-M3: Complete real-tool acceptance and documentation reconciliation.

## Surprises & discoveries

The Makefile already pins `rust-prover-tools`, at
`2ba17da6b7e24160c4041e8d41ba1ba6cca87b35`, for installation and Verus
execution. The remaining CI Kani path runs `scripts/ci/install_kani.py`. Its
two digests match upstream's `rust_prover_tools/binary_assets.py` exactly.

Upstream installs front ends to `$CARGO_HOME/bin` by default and stores
archives under `.cache/kani`. Netsuke currently uses a version-qualified
front-end directory and publishes it through `GITHUB_PATH`. Upstream changes
PATH only for its child processes, so workflow publication remains Netsuke's
responsibility. Its installer rechecks cached archive digests and still runs
extraction/setup; a warm run must not be described as skipping installation.

Upstream's default Verus proof is `verus/edge_harvest_proofs.rs`, which does
not exist here. Netsuke's actual entry point is
`verus/canonical_value/main.rs`. Repeated `--proof-file` arguments run separate
verifier processes in order; `VERUS_PROOF_FILE` and `INPUT_PROOF_FILE` each
still mean one path.

`scripts/ci/install_actionlint.py` also imports `ci_archive.py` and
`ci_support.py`. Removing those modules with the Kani script would break an
unrelated CI installer. There is no bespoke Verus CI downloader in this tree.

## Decision log

- 2026-11-01: Use the inspected upstream commit as the candidate pin. Record
  later upstream movement separately; do not silently change the review basis.
- 2026-11-01: Treat Make and the workflow as infrastructure adapters. The
  shared CLI owns download, checksum, extraction, installation and batch
  execution mechanics; Netsuke owns pins, proof selection and CI policy. No new
  Rust port or wrapper module is justified for this tooling-only change.
- 2026-11-01: Keep `kani-full` and `kani-ir` on their current execution path.
  Upstream's separate `kani exec` command changes compiler-routing semantics;
  adopting it is not necessary to replace downloads or enable Verus batches.
- 2026-11-01: Use explicit repeated CLI arguments for multiple Verus proofs,
  not a new manifest, whitespace-separated path variable, or repository loop.
  Preserve the existing `VERUS_FLAGS` override as the Make customization point.
- 2026-11-01: Do not add Z3 installation as an implicit dependency of every
  proof run. Upstream offers `z3 install`, but no bespoke Z3 downloader exists
  here. Check the existing Verus pin's solver requirements during acceptance; a
  required new solver pin or support policy is an exception to review.

## Outcomes & retrospective

The planning outcome is a bounded integration proposal with an immutable
upstream evidence basis. No installer, workflow, test, or proof implementation
has changed. All implementation milestones and their acceptance evidence remain
outstanding. The key lesson from inspection is that package reuse still leaves
repository-owned PATH, cache, and proof-selection contracts to verify.

## Context and orientation

Work from the Netsuke repository root. `Makefile` defines `PROVER_TOOLS_SOURCE`
and `PROVER_TOOLS`, plus `install-kani`, `kani-check`, `kani-full`, `kani-ir`,
`install-verus`, `verus`, and `formal-pr`. `tests/prover_tools_make_tests.rs`
currently replaces the CLI with a fake executable to test delegation,
diagnostic redaction and failure reporting; its flattened `$*` log cannot prove
that paths containing spaces survive as single arguments.

`.github/workflows/ci.yml` owns the `kani-smoke` lane.
`.github/actions/kani-cache/action.yml` restores and saves `.kani-cargo`,
`.kani-home`, and `.kani-rustup` under a versioned, platform-qualified key.
`tests/workflow_ci.rs` and `tests/workflow_contracts/kani_cache_test.py`
currently require the bespoke installer and its `INPUT_*` checksum bindings.
Scope and metrics contracts live beside the latter. The scope policy itself is
`tools/kani/proof-scope.toml`; update only tooling entries if the removed
installer is named, leaving the derived Rust source closure intact.

`scripts/tests/test_ci_install_kani.py` tests the old installer.
`scripts/tests/test_ci_scripts_metadata.py` inventories CI scripts, and
`make test-ci-scripts` names their tests explicitly. These references must be
updated atomically with deletion. Shared archive and containment tests remain
because actionlint still uses that code.

## Conformance basis

The baseline is Netsuke commit `5f0a4d123e8e2e69bb8f1f482794ed86dda123f4`.
Relevant repository guidance is `AGENTS.md`, `docs/contents.md`,
`docs/repository-layout.md`, `docs/developers-guide.md`, and
`docs/formal-verification-methods-in-netsuke.md` at that revision. ADR-0039
(`docs/adrs/adr-0039-run-the-kani-proofs-on-a-pull-request-only-when-their-inputs-change.md`)
governs scope gating. There is no task-specific Terms of Reference, technical
design, issue number, or assigned roadmap item; do not invent one or mark a
roadmap item complete for this plan.

The inspected upstream revision is
[592558f7183b554fc749696246984c5d7300d029][upstream]. Its README, users' guide,
`docs/v0-2-0-migration-guide.md`, `rust_prover_tools/cli.py`, `kani_binary.py`,
`binary_assets.py`, `downloads.py`, `verus_run_cli.py`, and `verus/batch.py`
define the external contract used below.

- R1: Shared pinned acquisition replaces bespoke Kani CI installation.
  Trace: EP-M1 to E1 installer wiring and E4 cold/warm real-tool evidence.
- R2: Explicit ordered variadic Verus execution preserves failure.
  Trace: EP-M2 to E2 argv tests and E3 pinned-CLI execution tests.
- R3: Existing CI policy, checksums, redaction and domain isolation survive.
  Trace: EP-M1 and EP-M2 to E1/E2, then EP-M3 to E4 and full gates.
- R4: Maintainers can reproduce and understand the supported commands.
  Trace: EP-M3 to developer/formal-verification guide updates and E4.

[upstream]: https://github.com/leynos/rust-prover-tools/tree/592558f7183b554fc749696246984c5d7300d029

## Interfaces and dependencies

Change `PROVER_TOOLS_SOURCE` to the inspected full SHA. Resolve it through
`uv tool run --python $(PYTHON_BASELINE) --from ... prover-tools`, retaining the
`PROVER_TOOLS` override and using the repository's public-Git environment
convention. Python 3.14 is already the repository baseline and the upstream
minimum. Do not import upstream Python internals into Netsuke production code.

CI should call `make install-kani` with `KANI_INSTALL_FLAGS` explicitly naming
`--install-dir "$CARGO_HOME/bin" --cache-dir "$KANI_HOME/archives"`. The tool
reads `tools/kani/VERSION`; do not pass an overriding version that could
disagree with that file. Remove obsolete `INPUT_KANI_VERSION`,
`INPUT_FRONTEND_SHA256`, and `INPUT_BUNDLE_SHA256` bindings: the immutable tool
revision now owns those exact asset digests. Publish `$CARGO_HOME/bin` to
`GITHUB_PATH` after successful installation. Keep this short orchestration in
the workflow rather than adding another installer script.

Bump `netsuke-kani-v1` to `netsuke-kani-v2` so old front-end layout does not
masquerade as a complete new installation. Archives under `KANI_HOME` remain
inside the existing three-directory cache ownership boundary. Retain platform,
runner image, version and trusted-writer qualification. Use `make kani-check`
for the subsequent check against the version file, preserving step ordering.

Set `VERUS_FLAGS ?= --proof-file verus/canonical_value/main.rs`. Explicit
`VERUS_FLAGS` overrides replace this default and may repeat `--proof-file`.
Each path must be quoted independently when it contains spaces. Do not append
the default proof to an explicit list or split the single-file environment
bindings. Keep `VERUS_INSTALL_FLAGS`, redacted diagnostics, and upstream's
nonzero status in the failure message; Make itself may return status 2.

## Plan of work and milestones

### EP-M1: Replace the Kani CI installer atomically

Begin with failing tests for the desired Make invocation, pin source, PATH
publication, archive location and new cache generation. Update
`tests/workflow_ci.rs`, `tests/workflow_contracts/kani_cache_test.py`, and
`tests/prover_tools_make_tests.rs`; use their existing parameterized test
patterns and exercise the real Makefile. A deleted-script assertion is useful
only alongside positive assertions for the replacement invocation.

Then update the Makefile pin/launcher and the workflow together. Delete
`scripts/ci/install_kani.py` and `scripts/tests/test_ci_install_kani.py`,
remove its explicit Make test-list entry, and revise the metadata inventory.
Sweep active tooling and documentation for references; retain historical
completed plans as history. Keep shared archive modules and their tests. Update
the relevant developer-guide installation paragraphs in the same commit.

The plateau discharges R1's wiring and R3's CI policy requirements through E1.
Run focused tests followed by all applicable gates before committing. Confirm
that no extra dependency, trust boundary, public interface, or Rust domain
change occurred. Record conformance and evidence; real downloads remain E4.
Recovery is to revert this atomic commit, not restore a parallel installer.

### EP-M2: Select Netsuke proofs and exercise upstream batches

First make an argv-preserving recorder in the existing Make integration tests
and demonstrate failures for the missing default proof and multi-file argument
boundaries. Cover default, explicit single path, repeated paths, spaces, and a
nonzero child status without exposing flag values in diagnostics. Change the
Make default only after observing those expected failures.

Add a focused Python integration test under
`scripts/tests/test_prover_tools_integration.py`. Run the pinned CLI as a
subprocess with a fake Verus executable and temporary proof files. Pass
`--no-install-missing --no-ensure-toolchain` so this test cannot install real
tools. Cover three successes and failures at the first, middle and last file;
assert exact order, no invocation after failure, preserved CLI status, and the
partial-success diagnostic. Include spaces and environment-versus-CLI
selection. Wire the test into a dedicated Make target that provisions the
pinned package through uv; include that target in implementation validation.
The tests exercise Netsuke's invocation contract, not upstream internals.

The plateau discharges R2 through E2/E3 and preserves R3. Document the override
and default in `docs/developers-guide.md`. Re-run the focused and full gates,
check conformance, and commit. No compatibility layer or new Rust API is
needed. Revert this milestone atomically if required; EP-M1 remains coherent.

### EP-M3: Prove integration and reconcile documentation

Exercise installation and proof execution with real tools as described in E4.
Update `docs/formal-verification-methods-in-netsuke.md` and the developer guide
to explain ownership of pins, cache paths, default proof selection, batch
failure, and reproducible commands. Update `docs/repository-layout.md` only
where the retired script changes its description. A substantive new boundary
would require an ADR and approval; this proposal retains the existing CLI
boundary and needs no new architectural abstraction.

This plateau discharges R4 and the real-tool part of R1/R3. Record actual logs,
proof counts and failure diagnostics, then reconcile every conformance link.
Keep incomplete or unavailable acceptance visible; do not mark COMPLETE while
E4 or a required gate is pending. Recovery is a focused documentation/evidence
correction or an atomic revert of the affected implementation milestone.

## Verification plan

No mathematical lemma or contractual business logic is introduced. The
obligations concern configuration and process boundaries; adding a new Kani
harness or Verus theorem for a Make recipe would not strengthen them. Existing
proofs remain the real-tool acceptance workload.

E1 verifies R1/R3: the installer is reached only after the scope decision,
never compiles from source, publishes the intended PATH, and uses only the
three owned cache directories. Workflow tests exercise run/skip and
restore/save conditions, explicit nightly selection, and the exact pin source.
Run `make test-workflow-contracts`, `make test-ci-scripts`, and the
`workflow_ci` Rust integration target via the normal test suite. Negative
controls restore the old script invocation, remove PATH publication, or permit
a pull-request cache write; each must fail the corresponding test. Keep full
checksums in expected boundary data or evidence so a truncated pin cannot be
mistaken for equivalent policy.

E2 verifies R2/R3 at Make's boundary. The recorder must record argument counts
and individual tokens, not flattened text. Named cases cover no override, one
path, several paths, whitespace and child status 42. A dropped second flag,
split path, or leaked secret is a failing negative control. This finite
partition is adequate because Make delegates ordering and iteration upstream.

E3 verifies R2 against the real pinned CLI with fake verifier processes. Three
existing temporary files witness successful execution; failures at each
position demonstrate reachable failure paths and prevent vacuous success. Also
test a missing proof file and require failure without a claimed complete batch.
The dedicated integration target must pass after EP-M2; removing a repeated
argument at the Make boundary must be caught by E2. Upstream ordering, status
propagation and diagnostic contracts are external assumptions exercised here,
not reimplemented or formally proved by Netsuke.

E4 verifies R1/R3/R4 in the hosted Linux x86_64 environment and a supported
local environment. On CI, record a cold run and a subsequent warm run with
archive reuse, exact Kani version, the proof harness count, mutation gate
success, scope decision and metrics. Verify an incomplete restored payload is
repaired by the installer and a bad cached archive is never accepted as valid;
upstream may replace it with a verified download. Use disposable directories
for fault injection, never alter shared caches. For Verus, run the existing
canonical-value proof and a batch of two temporary copies through the pinned
CLI, then introduce a false assertion in one temporary copy and require a
nonzero result. Record any required solver setup rather than assuming it.

Axioms are the inspected CLI semantics, Kani and Verus soundness for the
existing harnesses, GitHub cache trust policy, normal process exit behaviour,
and availability of the pinned assets and Python package dependencies. Offline
fake-process tests cannot establish download availability, actual solver
compatibility or proof validity; E4 supplies that evidence. No claim extends to
unsupported Kani platforms or releases. No property generator is needed because
this migration adds no repository-owned iteration algorithm.

## Concrete steps and validation

Run commands from the repository root. Before implementation, record branch,
status and upstream commit, then obtain approval. Use a separate clean checkout
of upstream for inspection; do not build under `/tmp`. The source reference to
inspect is the full SHA in Conformance basis.

For each milestone, run its new focused tests before and after the edit and
save the expected red assertion and subsequent green result. `make test`
includes `tests/prover_tools_make_tests.rs` and `tests/workflow_ci.rs`.
`make test-ci-scripts` covers the remaining installer helpers;
`make test-workflow-contracts` covers YAML and cache contracts. The new
`make test-prover-tools-integration` target introduced in EP-M2 must execute
the pinned-CLI subprocess suite and report all cases passing.

Run formatting and gates sequentially, with `set -o pipefail` and a separate
`tee` log per command under `/tmp`, using names such as
`/tmp/check-fmt-netsuke-import-prover-tools.out`:

```sh
make fmt
make check-fmt
make lint
make doc-coverage
make markdownlint
make nixie
make test
make test-workflow-contracts
make test-ci-scripts
make test-prover-tools-integration
```

Restart the sequence after any candidate edit. Delegate full gates to the
`scrutineer` agent; never overlap gates for the same candidate. If executable
examples are added to the users' guide, README, or standard-library guide, also
satisfy their tested-example registry and `make test-documentation-contracts`;
this plan only needs maintainer docs.

After installing the pinned tools, these repository-root commands must succeed:

```sh
make install-kani
make kani-check
make kani-ir
make install-verus
make verus
make verus VERUS_FLAGS='--proof-file verus/canonical_value/main.rs --proof-file verus/canonical_value/main.rs'
```

The last command deliberately proves the same real file twice to demonstrate
multiple invocations without inventing a second production theorem. Use the
existing documented systemd resource scope for local Kani proofs. Keep
admission wrappers and the pinned toolchains intact. The expected evidence is
version 0.67.0, successful existing harnesses, and two Verus invocations for
the batch; capture actual verifier totals rather than predicting them here.

For this documentation-only planning PR, run `make fmt`, `make check-fmt`,
`make markdownlint`, `make nixie`, and `git diff --check`. Code gates and real
proof runs belong to the future implementation, not claims about this draft.

## Idempotence and recovery

Re-run gates against the same candidate and reuse the shared Cargo cache.
Installer retries must go through the shared CLI, which verifies cached
archives; never bypass a digest failure. Use a new cache generation for the
layout migration rather than deleting remote caches. Clean only disposable
fixtures created for this task. If disk space fills or admission becomes
unhealthy, report the failure and preserve logs instead of killing other work.

## Artefacts and revision notes

The upstream API and source checkout both resolved main to
`592558f7183b554fc749696246984c5d7300d029` during planning. The local tree
began clean on the Netsuke baseline recorded above. CodeGraph located the
installer and its workflow/Make tests; direct source inspection confirmed the
external CLI bindings and archive ownership.

Revision 1 (2026-11-01): Draft the installer and variadic-proof migration from
current repository and upstream evidence. Implementation remains pending
approval and no acceptance result is claimed.
