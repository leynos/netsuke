# Import shared prover installers and variadic proof execution

Status: IN PROGRESS

This ExecPlan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current throughout execution. The user
approved this plan on 2026-11-01. Implementation follows its scope and
tolerances.

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
- Gate every commit and keep this living plan synchronized with evidence.
  Stop at the stated tolerance thresholds.

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
- Medium impact, high likelihood after a cache-generation change: the hosted
  warm-cache run cannot happen until a trusted push to `main` saves the new
  generation. Pull requests and manual dispatches cannot write that key. Verify
  cache behaviour locally with disposable paths, record a hosted cold run, and
  leave the hosted warm run pending until the repository's authorized main
  workflow populates the key.

## Progress

- [x] (2026-11-01) Inspect Netsuke's current Make targets, CI installer,
  workflow contracts, cache action and proof inventory.
- [x] (2026-11-01) Resolve and inspect upstream main at
  `592558f7183b554fc749696246984c5d7300d029` and draft this plan.
- [x] (2026-11-01) User approved implementation and set the ExecPlan status to
      APPROVED before implementation began.
- [x] (2026-11-01) Add EP-M1 expected-red contracts. The Rust targets reported
      2 expected failures (11 passed), workflow contracts reported 1 expected
      failure (1,665 passed, 3 skipped), and CI script tests reported 1 expected
      failure (98 passed). Logs are recorded in the implementation handoff.
- [x] (2026-11-01) Implement EP-M1's shared Kani install path, CI cache
      generation
      bump, obsolete downloader removal and maintainer-guide updates.
- [x] (2026-11-01) Run the full EP-M1 gate sequence. Formatting, typecheck,
      doc coverage (98.23%), Mermaid, nextest (4,942 passed, 7 skipped), doctests,
      workflow contracts (1,666 passed, 3 skipped, plus 28 supplementary tests),
      CI script tests (90 passed), Makefile validation and diff checks passed.
- [x] (2026-11-01) Replace R9109 substring assertions with one exact command
      snapshot and stage the intentional removals for spelling's tracked-file
      scan.
- [x] (2026-11-01) Rerun all 11 EP-M1 gates after the workflow line-wrap fix.
      All passed: format, lint, typecheck, doc coverage (98.23%), Markdown,
      Mermaid, Rust tests (4,942 passed, 7 skipped plus doctests), workflow
      contracts (1,666 passed, 3 skipped plus 28 helper tests), CI scripts
      (90 passed), Makefile validation and diff check. Logs share the
      `/tmp/*-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-r9109b.out`
      suffix.
- [x] (2026-11-01) Complete the committed EP-M1 CodeRabbit review against
      approval checkpoint `9a7bcb86f0b326f176fd637dc9a2aafa2080568b` and head
      `848d8ee866e10a22e92acda602984bfc19b1439e`. It identified a local macOS
      Kani installation regression; verify the platform evidence and restore
      the supported compile route before closing the milestone.
- [x] (2026-11-01) Add the Kani host-selection regression and observe its
      expected red result before changing Make: Linux x86_64 passed, while
      macOS arm64 and Linux aarch64 failed because they did not receive
      `--compile`. Log:
      `/tmp/test-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-m1-host-fallback-red.out`.
- [x] (2026-11-01) Default `make install-kani` to upstream's Cargo compile
      path outside Linux x86_64, retain explicit CI prebuilt flags, document
      the platform behaviour, and pass all three focused host cases. Green log:
      `/tmp/test-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-m1-host-fallback-green.out`.
- [x] (2026-11-01) Run all EP-M1 gates after the platform fallback. Ten passed;
      Markdown spelling failed on two US spellings, corrected in
      this revision. Test and review results are otherwise green. The log
      suffix is `epm1-macos-fallback`.
- [x] (2026-11-01) Rerun all 11 gates after the spelling correction. Formatting,
      lint, typecheck, doc coverage (98.23%), Markdown, Mermaid, Rust tests
      (4,945 passed, 7 skipped plus doctests), workflow contracts (1,666 passed,
      3 skipped plus 28 helper tests), CI script tests (90 passed), Makefile
      validation and diff checks all passed. Logs share the
      `/tmp/*-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-epm1-macos-fallback-final.out`
      suffix.
- [x] (2026-11-01) Commit EP-M1 as
      `848d8ee866e10a22e92acda602984bfc19b1439e` plus the Kani fallback repair
      `0e0dcbb047b03960127319d919674d604a08a0a4`.
- [x] (2026-11-01) Repeat CodeRabbit against the committed EP-M1 delta from
      approval checkpoint `9a7bcb86f0b326f176fd637dc9a2aafa2080568b` through
      `0e0dcbb047b03960127319d919674d604a08a0a4`; review completed with zero
      findings.
- [x] (2026-11-01) Demonstrate the missing Verus default with the
      argv-preserving Make recorder, then add the canonical-value default,
      explicit repeated `--proof-file` and space-containing path cases. The
      focused suite passes 11/11; logs are
      `/tmp/test-netsuke-import-prover-tools-epm2-red.out` and
      `/tmp/test-netsuke-import-prover-tools-epm2-make-green.out`.
- [x] (2026-11-01) Add a pinned-CLI integration target using fake Verus
      processes. The seven integration cases pass, covering order,
      first/middle/last failure, status propagation, missing paths and
      environment/CLI selection. Log:
      `/tmp/test-prover-tools-integration-netsuke-import-prover-tools-epm2-space-paths.out`.
- [x] (2026-11-01) Document the default proof, repeated override syntax and
      dedicated fake-verifier test target in the developers' guide.
- [x] (2026-11-01) Address the first EP-M2 lint findings: mark the focused
      child-process boundary explicitly, set `shell=False`, and attach failure
      messages to every Python assertion. Both full gate attempts stopped at
      lint; formatting passed each time. The complete sequence must pass after
      these corrections.
- [x] (2026-11-01) Run all 12 EP-M2 gates after the lint fixes. Formatting,
      lint, typecheck, doc coverage (98.23%), Markdown, Mermaid, Rust tests
      (4,947 passed, 7 skipped plus doctests), workflow contracts (1,666 passed,
      3 skipped plus 28 chained helper tests), CI scripts (90 passed), pinned
      prover-tools integration (7 passed), Makefile validation and diff check
      all passed. Logs share the
      `/tmp/*-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-epm2-assert-fix-final.out`
      suffix.
- [x] (2026-11-01) Commit EP-M2 as
      `fc989b9d631a245e08bb59eb646d346316b1e1cb` and review the exact range
      from `0e0dcbb047b03960127319d919674d604a08a0a4`. CodeRabbit completed
      against all five EP-M2 paths with zero findings.
- [x] (2026-11-01) Install Kani 0.67.0 and Verus
      0.2026.10.04.426d8b0 through the pinned shared CLI. `make kani-check`
      passed. The bounded Kani suite first timed out twice at its eight-minute
      cap while compiling cold dependencies, then passed all 15 harnesses on a
      warm compile; all invocations used the documented systemd scope.
- [x] (2026-11-01) Exercise Kani installation with disposable install, archive,
      and `KANI_HOME` paths while leaving shared `CARGO_HOME` untouched. A cold
      installation succeeded; a warm installation preserved both verified
      archive hashes;
      deleting the disposable `cargo-kani` executable was repaired; corrupting
      the disposable verifier archive caused it to be replaced with its pinned
      SHA-256. Logs are under `/tmp/kani-disposable-*-netsuke-import-prover-tools.out`.
- [x] (2026-11-01) Verify the canonical Verus proof (16 verified), a two-file
      batch (16 verified per file), and a batch whose second temporary proof
      contains `assert(false)` (nonzero exit, 15 verified and one error, stopped
      at that file with the first path reported as proved). No tracked proof
      source changed.
- [x] (2026-11-01) Run the first full EP-M3 gate sequence. Eleven gates passed,
      including 4,947 Rust tests, 1,666 workflow contracts, 90 CI-script tests,
      7 pinned-CLI integration tests, and 98.23% doc coverage. Markdownlint
      found one US spelling in the new cache-risk note; the remaining gates
      completed and passed. Logs share the
      `/tmp/*-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-epm3-candidate.out`
      suffix.
- [x] (2026-11-01) Correct the spelling and remove only the task-created
      default Kani archive cache before the final gate run.
- [x] (2026-11-01) Rerun all 12 EP-M3 gates after the spelling fix and plan
      evidence update. Formatting, lint, typecheck, doc coverage (98.23%),
      Markdown, Mermaid, Rust tests (4,947 passed, 7 skipped plus doctests),
      workflow contracts (1,666 passed, 3 skipped plus 28 helper tests), CI
      scripts (90 passed), prover-tools integration (7 passed), Makefile
      validation, and diff checks all passed. Logs share the
      `/tmp/*-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-epm3-spelling-fix-final.out`
      suffix.
- [x] (2026-11-01) Complete the local CodeRabbit CLI 0.9.2 review of the
      committed EP-M3 documentation delta from
      `1239b3bc2cb1b4bc70ba3318871858b8ed594e89` to
      `26409c772cc24d2b48a0265392bc52999bcc1fdd`. It returned four minor
      grammar findings in the three changed guides; all four are corrected in
      this revision. Reassessment and gates for the corrections remain pending.
- [x] (2026-11-01) Complete CodeRabbit CLI 0.9.2 reassessment of commit
      `dba20f45ab5a8534e5bed53140a9a4b3a028f1a1`. It returned one minor
      grammar finding in the formal-verification guide about the
      `CARGO_TARGET_DIR` sentence. The correction is in commit
      `5b1f4b22a58adaf61795b2ae271e1a25096e89e0`; all 12 local gates passed
      on that candidate, and focused reassessment of the correction returned
      zero findings.
- [ ] EP-M3: Complete the hosted Kani cache run, milestone review, and PR
      assessment/delivery. A pull request cannot save the new cache generation;
      the hosted warm run requires a trusted `main` push.

## Surprises & discoveries

At the start of this work the Makefile pinned `rust-prover-tools` to
`2ba17da6b7e24160c4041e8d41ba1ba6cca87b35` for installation and Verus
execution. The Kani workflow separately ran `scripts/ci/install_kani.py`; its
two digests matched upstream's `rust_prover_tools/binary_assets.py` exactly.
EP-M1 now pins the inspected current upstream commit so the same package owns
both acquisition paths and Verus batch support.

The EP-M1 review caught a local platform regression: the prior pinned
[`kani.py`](https://github.com/leynos/rust-prover-tools/blob/2ba17da6b7e24160c4041e8d41ba1ba6cca87b35/rust_prover_tools/kani.py)
installed through Cargo, while the new
[`kani_binary.py`](https://github.com/leynos/rust-prover-tools/blob/592558f7183b554fc749696246984c5d7300d029/rust_prover_tools/kani_binary.py)
uses prebuilt archives only on Linux x86_64 and offers `--compile` elsewhere.
Keep the fast verified-archive route on its supported host and retain the
Cargo-backed path for other local platforms.

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

Firecrawl retrieval of the upstream users guide at the pinned revision confirms
the Kani `--version-file`, `--install-dir` and `--cache-dir` options, plus
repeatable Verus `--proof-file` arguments, ordered execution, first-failure
behaviour and the unrelated upstream default proof path. Source:
<https://github.com/leynos/rust-prover-tools/blob/592558f7183b554fc749696246984c5d7300d029/docs/users-guide.md>,
retrieved 2026-11-01.

A bounded repository reconnaissance also found an active script inventory in
`docs/developers-guide.md` under “CI helper scripts”. EP-M1 must update that
inventory and the formal-verification installation description, while retaining
the actionlint downloader and its shared support modules.

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
- 2026-11-01: Preserve the previous local Kani installation route on hosts
  without upstream prebuilt binaries. Default `make install-kani` to
  `--compile` outside Linux x86_64; CI's explicit install flags keep using the
  verified prebuilt archives and cache paths.
- 2026-11-01: Keep hosted cache publication under the existing trusted-writer
  policy. PRs and manual dispatches cannot seed the new generation, so local
  disposable-cache runs establish warm reuse and repair while a hosted warm run
  remains contingent on an authorized push to `main`.

## Outcomes & retrospective

The approval checkpoint is recorded in commit `9a7bcb86f`. EP-M1's expected-red
contracts established the old pin, workflow command and CI script inventory
failures. The Makefile now uses the inspected upstream SHA, the Kani workflow
delegates through `make install-kani`, publishes its cached front-end, and
checks it through `make kani-check`; the old downloader and its test are
removed. The first full gate sweep passed all functional and structural checks
except R9109's substring assertion policy and spelling's inability to scan a
deleted file that remains in the Git index. The workflow command now has one
exact reviewed snapshot, and the removals are staged. A second sweep cleared
those findings and found one workflow line longer than 120 columns. The command
is now split into short shell assignments. The final EP-M1 sweep passed every
gate, including formatting, lint, typecheck, documentation coverage, Markdown,
Mermaid, all Rust tests and doctests, workflow contracts, CI script tests,
Makefile validation and diff checks. The exact candidate must retain those
results through the milestone review and commit. Package reuse still leaves
repository-owned PATH, cache, and proof-selection contracts to verify. The
committed milestone review then verified that the new installer would regress
local macOS installs; the pinned upstream supports prebuilt Kani only on Linux
x86_64 and offers `--compile` for other hosts. A red regression test now
specifies the host-selection policy. `KANI_HOST_TRIPLE` now selects the
verified archive only on Linux x86_64; other local hosts receive `--compile`,
while the workflow's explicit flags preserve CI's cached archive route. All
three focused Make cases and the final 11 gate sequence pass. The repair is
committed and its repeat CodeRabbit review found no issues. EP-M2 first
recorded the missing Verus default as an expected failure, then added an
argv-preserving recorder, the canonical-value default, and tests for repeated
proof paths with spaces. A pinned-CLI integration suite now exercises batches
through fake Verus processes, including failure position, status propagation,
missing inputs, and CLI/environment precedence. All 11 Make boundary cases and
seven upstream integration cases pass. The full 12-gate EP-M2 sequence passes,
including 4,947 Rust tests, 1,666 workflow contracts, 90 CI script tests and
98.23% documentation coverage. EP-M2 is committed as
`fc989b9d631a245e08bb59eb646d346316b1e1cb`; CodeRabbit reviewed its five-file
delta from the M1 head and reported zero findings. Real-tool acceptance and
documentation reconciliation remain for EP-M3. Local real-tool acceptance has
now passed: Kani 0.67.0 installed and version-checked, and all 15 existing
harnesses passed in the documented bounded systemd scope after two cold-build
attempts reached the eight-minute cap. Disposable cold and warm installations
passed; both pinned archive hashes remained stable on the warm run, an absent
`cargo-kani` executable was restored, and a corrupted verifier archive was
replaced with its expected SHA-256. The shared Cargo home was not redirected.
Verus 0.2026.10.04.426d8b0 verified the canonical proof (16 verified), both
files in a real two-proof batch (16 verified each), and stopped at a false
assertion in the second file with a nonzero status (15 verified, one error).
The formal-verification guide now describes the current delegated commands and
cache ownership. No Kani or Verus proof source changed, so this migration adds
no theorem or harness. The first full EP-M3 gate sweep found one spelling error
in the plan; after correcting it, all 12 gates passed, including 4,947 Rust
tests, 1,666 workflow contracts, 90 CI-script tests, 7 pinned-CLI integration
tests, and 98.23% doc coverage. Milestone review and the hosted cold-cache run
remain outstanding. A hosted warm cache remains unavailable until a trusted
`main` run saves the new generation; a PR cannot perform that write.

## Context and orientation

Work from the Netsuke repository root. `Makefile` defines `PROVER_TOOLS_SOURCE`
and `PROVER_TOOLS`, plus `install-kani`, `kani-check`, `kani-full`, `kani-ir`,
`install-verus`, `verus`, and `formal-pr`. `tests/prover_tools_make_tests.rs`
replaces the CLI with a fake executable to test delegation, diagnostic
redaction, failure reporting and argv boundaries, including paths with spaces.

`.github/workflows/ci.yml` owns the `kani-smoke` lane.
`.github/actions/kani-cache/action.yml` restores and saves `.kani-cargo`,
`.kani-home`, and `.kani-rustup` under a versioned, platform-qualified key;
only a cache miss on a push to `main` can save it. `tests/workflow_ci.rs` and
`tests/workflow_contracts/kani_cache_test.py` enforce the shared installer,
cache paths, and write policy. `scripts/tests/test_ci_scripts_metadata.py`
ensures the obsolete Kani downloader and test are absent. The scope policy in
`tools/kani/proof-scope.toml` retains the proof-source closure while excluding
the removed script from tooling metrics. Shared archive and containment helpers
remain because actionlint still uses them.

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
  Trace: EP-M1 to E1 installer wiring and EP-M3 to E4 local cold/warm,
  corruption-repair, real Kani, and hosted-cache evidence.
- R2: Explicit ordered variadic Verus execution preserves failure.
  Trace: EP-M2 to E2 argv tests and E3 pinned-CLI execution tests; EP-M3 adds
  the canonical proof, real ordered batch, and real first-failure result.
- R3: Existing CI policy, checksums, redaction and domain isolation survive.
  Trace: EP-M1 and EP-M2 to E1/E2, then EP-M3 to E4 local archive fault
  injection, the current cache-policy documentation, and full gates.
- R4: Maintainers can reproduce and understand the supported commands.
  Trace: EP-M1 and EP-M3 to developer/formal-verification guide updates, actual
  Kani and Verus invocation, and E4.

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

Run commands from the repository root. Before each milestone, record branch,
status and upstream commit, then confirm the work remains within the approved
scope. Use a separate clean checkout of upstream for inspection; do not build
under `/tmp`. The source reference to inspect is the full SHA in Conformance
basis.

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

Before implementation, a plan-only candidate uses `make fmt`, `make check-fmt`,
`make markdownlint`, `make nixie`, and `git diff --check`. Once implementation
begins, run the milestone and acceptance gates listed above; documentation-only
evidence cannot discharge the code, workflow, or real-prover obligations.

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
current repository and upstream evidence. At that revision no acceptance result
was claimed.

Revision 2 (2026-11-01): Mark the plan approved at the user’s explicit request
before starting implementation.

Revision 3 (2026-11-01): Begin EP-M1 after committing the approval checkpoint;
replace obsolete installer contracts with tests for the shared-tool workflow.

Revision 4 (2026-11-01): Observe the expected-red contracts and implement the
shared Kani install path, cache-generation bump, old-script removal, and guide
updates. Focused green validation remains pending.

Revision 5 (2026-11-01): Record the first full EP-M1 gate results. All
functional gates passed; replace the flagged substring assertions with a
reviewed command snapshot and stage the deletions before the next spelling scan.

Revision 6 (2026-11-01): Record the second full gate sequence: all gates passed
except the workflow line-length limit. Split the Kani install flags into short
assignments and update the exact command snapshot before the final EP-M1 sweep.

Revision 7 (2026-11-01): Record the final EP-M1 gate sequence. All 11 gates
passed on the workflow-wrap candidate; detailed results and log suffix are in
Progress. CodeRabbit review and commit remain outstanding.

Revision 8 (2026-11-01): Record the committed EP-M1 CodeRabbit review and its
verified macOS compatibility finding. Add a host-platform regression test
before selecting the upstream compile fallback outside Linux x86_64.

Revision 9 (2026-11-01): Confirm the macOS compatibility regression with an
expected-red Make integration test: Linux x86_64 passed, macOS arm64 and Linux
aarch64 failed to select `--compile`.

Revision 10 (2026-11-01): Restore Cargo-based Kani installation on hosts
without the Linux x86_64 prebuilt archive, document the platform behaviour, and
pass the focused Linux x86_64, macOS arm64 and Linux aarch64 Make cases. Full
gates and repeat CodeRabbit review remain pending.

Revision 11 (2026-11-01): Record the platform-fallback full gate attempt. Ten
gates passed; Markdown spelling caught two US spellings in this plan. Correct
them before the final gate sequence and repeated CLI review.

Revision 12 (2026-11-01): Record the final platform-fallback gate sequence. All
11 gates passed after correcting the two spellings. Commit and repeated
milestone review remain outstanding.

Revision 13 (2026-11-01): Record Kani and Verus real-tool acceptance,
disposable Kani cache reuse and repair checks, correct the current
formal-verification state and cache write policy, and note the hosted
warm-cache dependency on a trusted `main` run. The first attempt at the
documentation gate found one spelling error.

Revision 14 (2026-11-01): Correct the spelling, record the successful final
EP-M3 local gate sequence, and leave hosted cache acceptance and
review/delivery work pending.

Revision 15 (2026-11-01): Record the completed local CodeRabbit review and fix
its four grammar findings. Run the final gates and focused reassessment before
publishing the rebased candidate.

Revision 16 (2026-11-01): Record the focused reassessment finding about the
`CARGO_TARGET_DIR` sentence and correct its wording. Final gates and
reassessment remain pending.

Revision 17 (2026-11-01): Record all 12 local gates passing on the corrected
candidate and the zero-finding focused reassessment. Hosted Kani cache
acceptance and PR delivery remain outstanding.
