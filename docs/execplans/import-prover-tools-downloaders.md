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
- [x] (2026-11-01) Complete hosted cold Kani acceptance on published candidate
      `9a2c0658d62b9c3b59a533733df9ba601e95ad9e` via CI dispatch run
      [41322090500](https://github.com/leynos/netsuke/actions/runs/41322090500).
      The run missed
      `netsuke-kani-v2-Linux-X64-self-hosted-ubuntu2404-0.67.0`, installed
      Kani 0.67.0, verified all 15 harnesses, and passed mutation patch
      compilation. Dispatches cannot save the cache generation.
- [x] (2026-11-01) Receive the ExecPlan completeness and correctness assessment
      for base `e5d57c58352b51473de4170f86e34fdc9db3ece6` and head
      `9a2c0658d62b9c3b59a533733df9ba601e95ad9e` in
      [comment 6560374229](https://github.com/leynos/netsuke/pull/1063#issuecomment-6560374229).
      It found no confirmed implementation defect, but identified missing
      Make-boundary Verus failure and subsequent-process Kani PATH regression
      tests. It also found the hosted cold-run evidence stale in this plan.
- [x] (2026-11-01) Add both requested regressions: `make verus` child failure
      status and flag redaction, plus a separate Cargo process that selects the
      Kani binary published through `GITHUB_PATH` ahead of a conflicting later
      PATH entry. Both focused tests and the final full gate sequence pass;
      candidate-bound assessment follow-up remains pending.
- [x] (2026-11-01) Run the complete local gate sequence after adapting the
      PATH harness to the repository's subprocess and typing conventions.
      Eleven gates passed, including 4,948 Rust tests, 1,667 workflow
      contracts, 90 CI-script tests, 7 prover-tools integration tests, and
      98.23% doc coverage. Lint found one missing `Returns` section in the new
      helper docstring; correct it before the final gate sequence. Logs share
      the `/tmp/*-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-epm3-assessment-regressions-refactor.out`
      suffix.
- [x] (2026-11-01) Rerun all gates after adding the helper's return
      documentation. Eleven passed, with the Python docstring checker
      clarifying that `Returns` uses a dashed underline and a return type.
      `make test` again passed 4,948 tests with 7 skipped; workflow contracts
      passed 1,667 with 3 skipped plus 28 helpers; CI scripts passed 90 tests;
      prover-tools integration passed 7 tests; doc coverage was 98.23%. Apply
      the exact documented heading format and rerun the full sequence. Logs
      share the
      `/tmp/*-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-epm3-assessment-regressions-final.out`
      suffix.
- [x] (2026-11-01) Run the final full local gate sequence after correcting
      the helper's NumPy-style `Returns` documentation. All 12 gates passed:
      formatting, lint, typecheck, 98.23% doc coverage, Markdown and Mermaid,
      4,948 Rust tests (7 skipped) plus doctests, 1,667 workflow contracts
      (3 skipped) plus 28 helper tests, 90 CI-script tests, 7 prover-tools
      integration tests, Makefile validation, and diff checks. Logs share
      `/tmp/*-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-epm3-assessment-regressions-final2.out`.
- [x] (2026-11-01) Publish candidate `98e024e1edd053fe549012c6ee6ae7e4d93adc54`
      and receive the completeness/correctness follow-up in
      [comment 6562877623](https://github.com/leynos/netsuke/pull/1063#issuecomment-6562877623),
      correlated with request 6562864047 and the original assessment. The
      response confirms both regression gaps are satisfied and reports no
      remaining in-scope implementation omission or confirmed correctness
      defect. It requires hosted warm-cache acceptance before the plan can be
      marked complete.
- [x] (2026-11-03) Receive the queued CodeRabbit review request `f31cfc30` on
      published head `15f413bac8fcda6a96c344261125aeddbfd2f774`. Review
      [#5743914948](https://github.com/leynos/netsuke/pull/1063#pullrequestreview-5743914948)
      completed in check run
      [125570930194](https://github.com/leynos/netsuke/runs/125570930194) and
      reported two valid Makefile edge cases: the `--compile` Kani installer
      could select a conflicting `cargo-kani`, and an apostrophe in
      `CARGO_HOME` broke the `kani-check` shell argument. Focused regressions
      reproduced both failures before the repair and initially passed in the
      existing 14-test Makefile target. The two regressions have since been
      moved to tests/prover_tools_make_edge_tests.rs to keep the original
      test file within the repository's 400-line limit. The new target and all
      12 repository gates passed on the candidate described in Revision 41.
      That run is historical: it predates the review rounds and rebase, and
      describes neither the current head nor the current gate set. Revisions
      49 to 53 record the candidates that followed and their own evidence.
- [x] (2026-11-06) Make `make kani-check` a local read-only version query.
      Commit `475d15409` removes `KANI_CHECK_FLAGS` and the delegated
      `check-version` invocation; the target reads `tools/kani/VERSION`, runs
      the active-home `cargo-kani --version`, and compares the first semantic
      version, so it needs neither `uv` nor network access and writes no uv
      cache. New tests/prover_tools_kani_check_tests.rs poisons `PROVER_TOOLS`
      and `UV` to prove no remote resolution occurs, and covers mismatch,
      verifier exit status, unparsable output, and apostrophes in the active
      Cargo home. The obsolete delegated-target cases leave
      tests/prover_tools_make_tests.rs, and the developers' guide and formal
      verification methods document the local contract.
- [x] (2026-11-06) Run the full 12-gate sequence on the local-query candidate.
      All gates passed: `make check-fmt`, `make lint`, `make typecheck`,
      `make doc-coverage` (98.23%), `make markdownlint` (212 files),
      `make nixie`, `make test` (4,953 passed, 7 skipped, plus doctests),
      `make test-workflow-contracts` (1,669 passed, 3 skipped, plus 28 helper
      tests), `make test-ci-scripts` (90 passed),
      `make test-prover-tools-integration` (7 passed),
      `mbake validate Makefile`, and `git diff --check`. The first
      `make markdownlint` attempt failed on `unparseable` in
      docs/developers-guide.md; correcting it to `unparsable` and rerunning
      the Markdown gate resolved the finding.
- [ ] Reconcile the current-head CodeRabbit formal review. The candidate-bound
      implementation assessment [#6573587184](https://github.com/leynos/netsuke/pull/1063#issuecomment-6573587184)
      (Assessment-ID `import-prover-tools-1e9a9927-completeness-20261102`,
      base `e5d57c58352b51473de4170f86e34fdc9db3ece6`, head
      `1e9a992734550d51ef4cc21aee78da590459c6db`) accepts implementation
      requirements and earlier regression and constructor-documentation
      findings, with no confirmed functional defect. It confirms that the
      current hosted Kani run verified all 15 harnesses and reported successful
      mutation metrics. The two local scoped `make kani-ir` attempts remain
      inconclusive after their eight-minute caps. Review #5743914948 covered
      head `15f413bac8fcda6a96c344261125aeddbfd2f774`, and its two findings are
      repaired and published in the candidate. The follow-up triage
      [#6684528489](https://github.com/leynos/netsuke/pull/1063#issuecomment-6684528489)
      asked for an isolation boundary test for `kani-check` and two
      documentation corrections; this revision supplies all three.
      Reconciling the summary and pre-merge rows for the resulting head remains
      open.
- [ ] EP-M3: Complete trusted-main warm-cache acceptance and PR
      assessment/delivery. The candidate-bound assessment confirms the
      functional requirements; hosted warm-cache evidence remains open because
      a pull request cannot save the new generation. After the authorized merge,
      observe a trusted `main` run that writes and reuses the new generation.
- [x] Repair CodeRabbit's Kani PATH finding. Review `4438989893` identified
      that `BUILD_TOOLS_PREFIX/bin` could shadow the active Cargo home's
      `cargo-kani`. The published fix invokes the active-home executable
      directly, preserves build-tools-first PATH precedence for `mold`, and
      covers conflicting binaries, default and custom Cargo homes, and paths
      containing spaces. CodeRabbit confirmed the repair in reply
      [#4440506630](https://github.com/leynos/netsuke/pull/1063#discussion_r4440506630).
      The full local gate sequence passed; hosted CI verified Kani 0.67.0 and
      all 15 harnesses on the candidate. The two local scoped attempts timed
      out before producing proof results; see Revisions 29 and 30.
- [ ] Reconcile the current-head diff-scoped Docstring Coverage row. The
      latest available summary reports 56.25% across 16 functions in 6 files,
      with 4 unsupported declarations, while its review-completed line names
      head `15f413ba`. CodeRabbit's
      response in comment 6569361800 records an access limitation: the
      available report and check output expose no declaration-level diagnostics,
      and that conversation could not access internal analyser records. Its
      source inventory lists only `#[test]` and
      `#[rstest]` functions without Rustdoc comments and finds documentation
      on touched helpers and Python declarations. The completed formal review
      reports findings but supplies no refreshed declaration diagnostics.
      Commit `475d15409` adds `///` documentation to the touched
      integration-test and helper functions; the `#[cfg(test)]`/`#[test]`
      carve-out in AGENTS.md covers Rustdoc-invisible items, which these
      compiled integration-test targets are not, and no aggregate Rustdoc
      figure is substituted for the diff-scoped row. Reconcile the row against
      the latest summary and pre-merge output after the repair review,
      requesting declaration diagnostics if a warning remains.
- [x] (2026-11-06) Replace the isolation sandbox with a read-only chroot after
      CodeRabbit kept the boundary finding open. The previous version bound
      only the repository, so `temp=read-only` re-probed the repository and
      nothing outside that mount was restricted. The chroot now binds `/usr`,
      `/etc`, the repository, and the fixture read-only over a read-only root,
      mounts a fresh procfs, and classifies each probe by the kernel's errno
      with a writable `control` path as a non-vacuity guard. The test requires
      `control=writable`, `repo=read-only`, `fixture=read-only`,
      `rootdir=read-only`, `usr=read-only`, `etc=read-only`, `tmp=read-only`,
      `home=read-only`, `ifaces=lo,`, `dns=blocked`, and the matching version,
      and rejects any other `=writable` line. The sandbox machinery moved to
      `tests/prover_tools_kani_check_tests/sandbox.rs` to stay under the
      400-line module cap. All 11 gates passed sequentially on tree
      `fe679d954f064187ab642572bfc09640649e5dec`; the code commit is
      `b45b19da5d00f6c2bfeb63681384743a24f2a853`. Revision 45 records the
      detail, and Revision 44's superseded claim is corrected in place.
- [x] (2026-11-06) Make the `kani-check` write boundary a required CI check.
      Commit `231ec7344` creates `/tmp` and the caller's home directory inside
      the chroot before the read-only remount, so both are proven unwritable
      instead of absent, and guards an unset `HOME` under `set -u`; adds a
      capability probe that performs the bind, read-only remount, procfs
      mount, and `chroot` the sandbox needs and names the operation a host
      refused; gates the sandbox module with `#[cfg(target_os = "linux")]`;
      and runs the case as `make test-kani-check-isolation` with
      `NETSUKE_KANI_CHECK_SANDBOX_STRICT=1` from its own `kani-smoke` step.
      All 16 gate targets pass on tree `bc8771006`. See Revision 46.
- [x] (2026-11-06) Close the remaining pre-merge rows from review
      `5803947003`. Two new suites cover the recipe and the real verifier:
      `tests/prover_tools_kani_check_command_tests.rs` drives `make kani-check`
      with a fake verifier for a matching pin, a mismatched pin, a missing pin,
      a pin that is not a file, and a failing verifier; and
      `tests/prover_tools_verus_e2e_tests.rs` gains two property cases that run
      the pinned CLI against a fake verifier over generated batch lengths,
      failure positions, and exit statuses. `make kani-check` names a stable
      `category=` on every failure, prints the pin and the configured command
      before invoking the verifier, and never echoes the verifier's own
      output: it captures that stream, extracts one semantic version, and
      quotes only that token, so an unparsable report is categorized rather
      than repeated. `make test-verus-e2e` is the strict lane for the
      real-verifier cases. See Revision 48.
- [x] (2026-11-09) Refuse an absent Kani payload instead of installing one,
      and widen what the trace can see. `cargo-kani` treats a missing
      `<KANI_HOME>/kani-<version>` as an incomplete installation and starts a
      first-time setup that downloads a release, so `make kani-check` —
      documented as a read-only query — could become an installer. The recipe
      now checks that directory before invoking the verifier and refuses with
      `category=payload-missing`, naming `make install-kani`. The isolation
      case runs a further traced query against a home with no payload and
      requires both the refusal and an attempt-free trace. The refusal is what
      fails if the guard is removed, since the run uses the stub verifier and a
      stub answers without installing anything; the trace only shows that the
      refusal attempted nothing itself. The trace filter moved from named calls
      to the `%file,%network` classes, because `strace` refuses to start when a
      qualifier names a call the host architecture lacks and `mkdir` has no
      syscall of its own on aarch64; the reader's mutating-call list grew to
      cover the calls those classes now deliver. `WHITAKER_SUITE_SELECTION` is
      `:=` rather than `?=`, so an exported value cannot replace the pinned
      selection. See Revision 58.
- [x] (2026-11-09) Refuse an empty `KANI_HOME` rather than resolve it two
      ways. `cargo-kani` reads a set-but-empty home as the current directory,
      while the recipe's shell default reads it as `$HOME/.kani`, so the
      payload guard could approve one directory while the launcher installed
      into another. `make kani-check` now refuses that setting with
      `category=kani-home-empty`, the category list names it, and
      `kani_check_refuses_an_empty_kani_home` drives it end to end. The same
      review pass bounded the workflow's apt fetches, split the longest
      sentences in the guide's boundary section, corrected its writable-path
      count, and tightened three contract finders. See Revision 59.
- [x] (2026-11-09) Split the `kani-check` command suite, correct the traced-run
      order, and stop claiming the trace detects a removed guard. The
      walkthrough's remaining rows named three things Revision 59 had not
      fixed. `tests/prover_tools_kani_check_command_tests.rs` was 414 lines and
      the empty-home case pushed it to 452; its fixture now lives in
      `tests/prover_tools_kani_check_command_tests/fixture.rs`, leaving the
      case list at 277 lines and the fixture at 192, both inside the cap. The
      guide called the missing-payload query the third traced run when the
      dirty-recipe control runs third, so the ordinals are gone rather than
      corrected. And five places — the guide, `ensure_missing_payload_refused`,
      the isolation case's doc comment, and the two ExecPlan passages — said
      the payload-free run's clean trace is what fails if the guard is removed.
      It cannot be: that run uses the stub verifier, which answers `--version`
      without installing anything, so a removed guard would let the run
      succeed. The refusal status is what catches it, and the clean trace only
      adds that the refusal attempted nothing. See Revision 60.
- [x] (2026-11-10) Answer the review's trace, ordering and signature
      findings. The trace reader now names the re-time, ownership and
      extended-attribute calls `%file` delivers, and `sendmmsg`, with a case
      per family; a suppressed attempt through any of them was previously a
      clean trace. Every isolation assertion takes the whole `SandboxRun` and
      reads the stream or trace it is about, which fixes CodeScene's
      string-heavy signature report — nine of twelve arguments were strings —
      and the two-`&str` inconsistency the review named. The step test records
      the `sudo` calls visible when `make` starts, so the lift-before-suite
      order is asserted rather than assumed, and the two no-lift cases are one
      parametrized case. The tracer install uses `sudo --non-interactive`, the
      Whitaker operator has cases for a recipe-line reference and `+=`, and
      the Verus property cases report fixture errors as case failures. See
      Revision 61.
- [x] (2026-11-10) Close the review at `cfdd1563f`. `kani-check` now refuses
      an unset `KANI_HOME` with an empty `HOME` as `home-unresolvable`, because
      the `$HOME/.kani` fallback would otherwise inspect `/.kani`, a directory
      no launcher derives; the category has a regression case and a documented
      entry. The guide's `whitaker-ls` sentence and the Outcomes summary now
      name what they mean, the summary extending through Revision 61. Hosted
      CodeScene then caught the two home refusals as duplicates, so they are
      one `#[rstest]` case over a fixture enum. See Revision 62.

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

The PR's formal CodeRabbit review found that the workflow's `GITHUB_PATH`
publication alone does not control Kani command selection: each Make recipe
rebuilds PATH and had placed `$HOME/.cargo/bin` ahead of a job-local
`CARGO_HOME/bin`. The new workflow-contract regression invokes `make kani-ir`
through a Cargo dispatcher and proves that the active Kani binary wins with
both default and custom homes. The same review caught an overbroad guide
sentence that implied Kani proof execution also delegates to the pinned CLI.

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
tests, and 98.23% doc coverage. Hosted cold acceptance then passed at the
published head: the dispatch run missed the new cache key, verified all 15 Kani
harnesses, and passed mutation compilation. The completeness assessment
confirmed no implementation defect but identified missing regression coverage
for `make verus` failure propagation and Kani path precedence in a subsequent
process. Both assessment regressions and the final 12-gate local sequence now
pass. The completeness/correctness follow-up confirms the two coverage findings
are satisfied on published head `98e024e1edd053fe549012c6ee6ae7e4d93adc54`; it
found no remaining in-scope implementation omission or confirmed correctness
defect. Hosted warm-cache acceptance remains contingent on a trusted `main`
cache writer. The subsequent formal PR review independently found that Make's
PATH recuration could select the wrong Kani binary and that the guide
overstated CLI delegation. The first PATH correction prioritized the active job
Cargo home, but a later review found that `BUILD_TOOLS_PREFIX/bin` could still
shadow `cargo-kani`. Commit `1e9a992734550d51ef4cc21aee78da590459c6db` now
invokes the active Cargo home's executable directly while preserving
build-tools-first PATH precedence for `mold`; the regression covers conflicting
binaries, default and custom Cargo homes, and paths with spaces. CodeRabbit
confirmed the repair in reply
[#4440506630](https://github.com/leynos/netsuke/pull/1063#discussion_r4440506630).
The guide distinguishes the pinned CLI's installation role from proof
execution by Make targets, and from Kani version checking, which is the local
read-only query rather than a delegation; CodeRabbit confirmed that correction
in reply
[#4437685594](https://github.com/leynos/netsuke/pull/1063#discussion_r4437685594).
Both corrections are published. Current-head hosted Kani smoke verified
version 0.67.0 and all 15 harnesses. The next completed formal review on
`15f413bac8fcda6a96c344261125aeddbfd2f774` found two additional Make boundary
cases: the `--compile` installer could resolve a conflicting
`BUILD_TOOLS_PREFIX/bin/cargo-kani`, and shell quoting failed when the active
Cargo home contained an apostrophe. Both defects reproduced with focused tests.
The install recipe now prepends the exported active Cargo home only for the
installer invocation, leaving Make's global build-tools-first order intact;
`kani-check` uses a shell-escaped command value. Both regressions pass in the
focused Rust suite (2/2). All 12 sequential repository gates passed on the
candidate recorded as Revision 41 — 4,950 nextest tests, all doctests, 1,669
workflow contracts, 90 CI-script tests, seven prover-tools integration tests
and 98.23% doc coverage — and each later revision records its own sweep at its
own entry rather than renewing that one. The exact-base CodeScene delta reports
no new findings, the new edge-test file scores 10.0 with no review findings,
and the hosted CodeScene check reports six quality gates passed. The repair is
published in the candidate. The isolation boundary test now enforces the write
boundary with a read-only chroot rather than a single repository bind, so the
query is verified to run with every mounted path read-only: `/usr`, `/etc`, the
repository, the fixture directory, the chroot root, `/tmp` and the caller's
home. Three paths are deliberately writable, and none is storage the query can
use. The `control` probe at `/proc/self/comm` exists to prove the probe can
report a plain failure rather than classifying every path as blocked because
redirection itself is broken. `/var/trace` is a writable bind of a host
directory where `strace` records what each run attempted; the recipe is never
told about it, and the case reads it back afterwards as the only record of a
suppressed attempt. `/dev/null` is bound because shells and the recipe's own
diagnostics write there unconditionally, and without it a shell aborts a
redirection instead of reporting a refused write. On a host that forbids
unprivileged user namespaces the case reports the reason and returns success,
which `NETSUKE_KANI_CHECK_SANDBOX_STRICT=1` converts into a failure. The review
at `5803947003` recorded six failed pre-merge checks, and the working candidate
now addresses each while leaving every published repair above unchanged. Two
new suites carry the coverage. `tests/prover_tools_verus_e2e_tests.rs` drives
`make verus` through the pinned CLI and the real verifier, checking the default
proof, an ordered batch and failure propagation, and adds property cases that
generate batch length, failure position and exit status — and, in a third, a
batch order that is a permutation rather than creation order — against a fake
verifier while the CLI stays real.
`tests/prover_tools_kani_check_command_tests.rs` drives the `kani-check` recipe
through a matching pin, a mismatch, a missing pin, an unreadable pin and a
failing verifier, and asserts that the verifier receives `--version` in every
reachable case. The recipe now reports a stable `category=` and takes only a
version token from the verifier's output — the `found` value of a mismatch and
the version named by the success line; the `kani-version=` field is the
expected pin read from the file — because the surrounding text comes from an
external binary and no amount of bounding or escaping makes it safe to print.
The read-only chroot boundary remains the evidence for the blocked-network and
blocked-write requirement, and it runs strictly in the required `kani-smoke`
lane rather than skipping. Version checking is described as the local read-only
`--version` query, not a delegation to the pinned CLI.

The gate evidence above is historical: it is the Revision 41 sweep, and it was
collected before the review at `5808594893` and the CodeScene delta on
`adeb43dd6` raised their findings. Fourteen further candidates have since been
gated, each recording its own evidence at its own revision entry: Revision 49
for those findings, Revision 50 after the rebase onto `main` at `6ff836603`,
Revision 51 for the review at `5816957501`, Revision 52 for the Whitaker
duplicate-registration repair, Revision 53 for the `string-continuation- style`
repair and the contract-test split, Revision 54 for the review at `5819530205`,
Revision 55 for the runtime trace reader, Revision 56 for the
installed-verifier query inside the boundary, Revision 57 for the control
recipe that discards a write and a directory creation, Revision 58 for the
payload refusal and the syscall classes, Revision 59 for the review at
`715970069`, Revision 60 for its eight inline findings and the removed-guard
check, Revision 61 for the review at `1f38b9e91` and the CodeScene
string-argument repair, and Revision 62 for the review at `cfdd1563f`. The
Revision 41 run quoted at the start of this section is the earliest of them and
describes neither the current head nor the current gate set. Review at the
current head and trusted-main warm-cache acceptance remain open.

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
cache paths, write policy, and Kani precedence in a subsequent process.
`scripts/tests/test_ci_scripts_metadata.py` ensures the obsolete Kani
downloader and test are absent. The scope policy in
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
A test runs the install-step shell command and invokes `cargo kani --version`
from a separate process with a conflicting binary later on PATH; the published
installer path must win. The harness is local to this workflow contract and
models only the `GITHUB_PATH` handoff; it is not a shared subprocess fixture.
The source-compile fallback also runs a fake `cargo kani setup` with a
conflicting `cargo-kani` under `BUILD_TOOLS_PREFIX/bin`; the installer
invocation must select the active Cargo home's executable while preserving the
global build-tools-first order used to resolve `mold`. Run
`make test-workflow-contracts`, `make test-ci-scripts`, and the `workflow_ci`
Rust integration target via the normal test suite. Negative controls restore
the old script invocation, remove PATH publication, or permit a pull-request
cache write; each must fail the corresponding test. Keep full checksums in
expected boundary data or evidence so a truncated pin cannot be mistaken for
equivalent policy.

E2 verifies R2/R3 at Make's boundary. The recorder must record argument counts
and individual tokens, not flattened text. Named cases cover no override, one
path, several paths, whitespace, apostrophes in the active Cargo home, and
child status 42 for both the installer and the `verus` proof target, including
failure-diagnostic redaction. A dropped second flag, split path, malformed
shell command, leaked secret, or swallowed proof failure is a failing negative
control. This finite partition is adequate because Make delegates ordering and
iteration upstream.

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
success, scope decision and metrics. The cold run passed on the published
candidate in dispatch run 41322090500: the new generation missed, Kani 0.67.0
installed, all 15 harnesses verified, and mutation patch compilation passed. A
subsequent warm run remains pending until a trusted `main` cache writer
populates the generation. Verify an incomplete restored payload is repaired by
the installer and a bad cached archive is never accepted as valid; upstream may
replace it with a verified download. Use disposable directories for fault
injection, never alter shared caches. For Verus, run the existing
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

Revision 18 (2026-11-01): Record the successful hosted cold Kani run and the
ExecPlan completeness assessment, add both requested regression tests, and
record their focused passes. The full gate sequence and assessment follow-up
remain pending; hosted warm-cache acceptance remains pending.

Revision 19 (2026-11-01): The first full gate sequence for the assessment
regressions passed 11 gates; `make lint` identified five repository-specific
subprocess, import, and local-count findings in the Python contract test. Shape
the harness to match existing subprocess tests before rerunning the sequence.

Revision 20 (2026-11-01): Record the full gate rerun after adapting the
workflow harness. Eleven gates passed, including the complete Rust, workflow,
CI-script, and prover-tools suites; lint found one missing `Returns` section in
the helper docstring. Add the section and rerun all gates.

Revision 21 (2026-11-01): Record the full gate rerun after adding the Python
helper's return section. Eleven gates passed; lint requires the repository's
NumPy-style `Returns` heading with a dashed underline and return type. Apply
that format and repeat all gates.

Revision 22 (2026-11-01): Record all 12 local gates passing on the final
assessment-regression candidate, including the complete Rust and Python test
suites. The two requested regression cases are closed; publish this candidate
and request assessment follow-up. Hosted warm-cache acceptance still requires a
trusted `main` cache writer.

Revision 23 (2026-11-01): Record the candidate-bound assessment follow-up
confirming both requested coverage findings are satisfied and no in-scope
implementation omission or confirmed correctness defect remains. Keep the plan
in progress until hosted warm-cache acceptance is observed from a trusted
`main` run.

Revision 24 (2026-11-02): Record the candidate-bound assessment response
identifying four undocumented non-test proof-case constructors. The four
constructors now have usage documentation; preserve the distinction between
functional assessment and the still-unconfirmed current-head Docstring Coverage
result. The first full gate sequence passed 11 of 12 gates; only the ExecPlan's
Markdown reflow failed `make check-fmt`. After applying the formatter, all 12
gates passed in the full rerun (see Revision 25). Current-head review and
trusted-main warm-cache acceptance remain outstanding.

Revision 25 (2026-11-02): Record the final local gate sequence after formatting
the assessment follow-up. All 12 gates passed: `make check-fmt`, `make lint`,
`make typecheck`, 98.23% doc coverage, Markdown lint (0 issues), Mermaid
validation, 4,948 Rust tests plus doctests, 1,669 workflow contracts (3
skipped) plus 28 helpers, 90 CI-script tests, 7 prover-tools integration tests,
Makefile validation, and `git diff --check`. Logs share the
`/tmp/*-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-5.out` suffix.
After this final plan update, `make check-fmt`, `make markdownlint`,
`make nixie`, and `git diff --check` passed (logs use the `final-docs` suffix).
The latest repeat after the wording adjustment uses the `final-docs2` suffix.
This plan update was committed and published in candidate
`c3c35c76773ab888ca16c44244ed343f82f7ba93`; current-head review and trusted-
main warm-cache acceptance remained outstanding at that point (see Revision 26).

Revision 26 (2026-11-02): Record that candidate `c3c35c7` passed hosted CI,
Release Dry Run, Netsukefile Build Test, CodeScene, and Gecko review. The
candidate-bound completeness response in comment 6566774367 accepts the four
constructor docs and reports no remaining in-scope omission or correctness
defect. Full plan acceptance remains open for trusted-main warm-cache evidence
and a current-head diff-scoped Docstring Coverage result. The formal CodeRabbit
request 7962aeba is still queued against the PR; its summary remains stale at
`c7438721`. Keep `Status: IN PROGRESS` until both acceptance items are recorded.

Revision 27 (2026-11-02): The formal CodeRabbit review completed on
`55cebb79ac3eb7e1e8fb474ae397e965c59a0ecc`. It found that the active Cargo
home's `cargo-kani` can lose to a conflicting binary in
`BUILD_TOOLS_PREFIX/bin` (finding 4438989893). The existing contract did not
cover that directory. The repair invokes the active-home executable directly
and keeps the global PATH order that prioritizes the pinned `mold`; a
regression now covers the conflicting prefix, default and custom Cargo homes,
and spaces in the home path. The same review reports current-head diff-scoped
Docstring Coverage at 56.25% (16 functions in 6 files, 3 unsupported). A
focused request for the exact counted declarations is recorded in comment
6569348528; await that diagnostic before changing docs. The workflow on `main`
confirms that only a push event on `main` can write the cache generation;
manual dispatch cannot provide the pending warm-cache acceptance. Full gates,
repair publication, current-head review reconciliation, and trusted-main
acceptance remain outstanding.

Revision 28 (2026-11-02): The focused active-home Kani contract passed for
default and custom Cargo homes, including a path containing spaces and a
conflicting `cargo-kani` under `BUILD_TOOLS_PREFIX/bin`. `make kani-check`
verified Kani 0.67.0 through the active-home executable. CodeRabbit's
diagnostic response in comment 6569361800 confirms the pre-merge analyser
exposes no declaration membership. Its independent source inventory found docs
on touched helpers and Python declarations; the only touched undocumented Rust
declarations it listed are test functions. Keep the row open for evidence-based
reconciliation after publishing the repair; do not add Rustdoc comments to
tests against repository guidance. Full gates and repair publication remain
pending.

Revision 29 (2026-11-02): The first scoped Kani run reached its documented
eight-minute runtime cap while compiling dependencies. Systemd recorded
`Result=timeout`; no proof result was produced. The shared compiler supervisor
was healthy, with all four slots held and a long queue. Retry once under the
same scope and limits to reuse the partial build; do not relax the cap or
bypass compiler admission.

Revision 30 (2026-11-02): The one permitted retry also reached the documented
eight-minute cap while Cargo waited on the package-cache lock and compiled
dependencies. Systemd again recorded `Result=timeout`; no Kani proof result was
produced, and the compiler supervisor remained healthy. Stop further retries.
The earlier 15-harness result remains valid for the unchanged proof sources;
the changed active-home command is covered on this candidate by the focused
path contract and the passing real `make kani-check` against Kani 0.67.0.

Revision 31 (2026-11-02): The first final gate pass on the Kani path repair
passed formatting and typecheck, but Ruff found one excess local in the new
path regression. The test now derives the build-tools marker path at each use
instead of storing it in a separate local. Restart the full gate sequence from
the beginning against this corrected candidate.

Revision 32 (2026-11-02): After that local-count fix, the full gate sequence
passed `make check-fmt`, `make lint`, `make typecheck`, and `make doc-coverage`
(98.23%). Mermaid validation, the 4,948-test nextest suite and doctests,
workflow contracts (1,669 tests plus 28 supplementary checks), CI scripts (90
tests), prover-tools integration (7 tests), Makefile validation, and diff
checks passed. Markdown lint found two American spellings of the analysis
tool's name; both are corrected to the project spelling. Restart the full
sequence on the corrected candidate.

Revision 33 (2026-11-02): The corrected candidate passed the full sequential
gate set: formatting, lint, typecheck, 98.23% doc coverage, Markdown lint,
Mermaid validation, 4,948 nextest tests (7 skipped), all doctests, workflow
contracts (1,669 passed, 3 skipped, plus 28 supplementary checks), CI scripts
(90 passed), prover-tools integration (7 passed), Makefile validation, and diff
checks. Logs use the `import-prover-tools-r3` suffix under `/tmp`. PR review
and hosted cache acceptance remain open; keep this plan in progress.

Revision 34 (2026-11-02): Record the exact-head assessment on candidate
`1e9a992734550d51ef4cc21aee78da590459c6db`. CodeRabbit confirms that the
implementation requirements and previous regression and
constructor-documentation findings are satisfied, with no confirmed functional
defect. The hosted Kani workflow verified version 0.67.0, all 15 harnesses, and
successful mutation metrics; the two local scoped runs remain inconclusive.
Hosted CI and Netsukefile Build Test passed, hosted CodeScene passed six
quality gates with zero annotations, and local `cs delta` found no issues in
the checked changes. Keep hosted warm-cache acceptance pending until a trusted
`main` workflow populates the new cache generation. The 56.25% diff-scoped
Docstring Coverage row still belongs to head `55cebb79`; request a current
result through formal review after this plan update. Full ExecPlan acceptance
remains open.

Revision 35 (2026-11-02): Record the focused Progress-assessment response
[#6574621268](https://github.com/leynos/netsuke/pull/1063#issuecomment-6574621268)
on head `4f1861e98d2b15d53f2026ccfeef4cac3df32142`. CodeRabbit accepts
Revision 34's reconciliation, confirms the Kani PATH repair and the two
outstanding acceptance items, and retains `Status: IN PROGRESS`. Correct its
two wording findings: describe analyser output as unavailable at declaration
level without claiming no authorized backend exists, and update Outcomes with
the published, separately confirmed initial PATH and final active-home
executable repairs. Keep hosted warm-cache and current-head diff-scoped
Docstring Coverage open; the walkthrough still reports the stale `55cebb79`
review and current formal review remains pending.

Revision 36 (2026-11-03): The queued current-head review completed on
`15f413bac8fcda6a96c344261125aeddbfd2f774`. CodeRabbit's check reports “Review
completed” and review
[#5743914948](https://github.com/leynos/netsuke/pull/1063#pullrequestreview-5743914948)
supplies a Makefile finding about `cargo-kani` shadowing in the `--compile`
installer path;
[comment 4448218361](https://github.com/leynos/netsuke/pull/1063#discussion_r4448218361)
identifies apostrophe quoting in `kani-check`. Read-only source verification
and focused pre-fix tests confirmed both defects. The Makefile repairs and
regressions initially passed the existing 14-test Rust target. The two new edge
regressions are now in tests/prover_tools_make_edge_tests.rs so the existing
test file remains below the repository's 400-line limit; final validation is
pending. The full sequential gates, review replies and confirmation,
current-head review/pre-merge reconciliation, and trusted-main warm-cache
acceptance remain open; keep the plan `IN PROGRESS`.

Revision 37 (2026-11-03): The extracted edge-test target passed on the current
worktree: 2 tests passed, 0 skipped, using cargo nextest run --test
prover_tools_make_edge_tests. The scrutineer waited in the healthy shared
compiler queue and completed normally; the full log is
/tmp/test-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-edge-regressions.out.
The test-file split keeps tests/prover_tools_make_tests.rs at 379 lines. Run
all 12 repository gates sequentially from the beginning on the final candidate;
then commit, publish, reconcile current-head review findings and pre-merge
checks, and observe trusted-main warm-cache acceptance. Keep Status: IN
PROGRESS.

Revision 38 (2026-11-03): The first post-review full gate sequence passed 11 of
12 gates, including typecheck, 98.23% doc coverage, Markdown/Mermaid, 4,950
tests (7 skipped) plus doctests, workflow contracts, CI scripts, prover-tools
integration, Makefile validation and diff checks. Clippy rejected the new
regression's direct PATH read. Remove that redundant override and rely on
Makefile's exported PATH, where the injected BUILD_TOOLS_PREFIX/bin precedes
CARGO_HOME/bin; set the apostrophe test's CARGO_HOME to its custom path
explicitly. The corrected focused target then passed 2 tests with none skipped,
using cargo nextest run --test prover_tools_make_edge_tests; the log is
/tmp/test-a97788bc-4b29-45ee-b2bb-7ed1bab5f9fc-import-prover-tools-policy-fix.out.
Restart all 12 gates on this corrected candidate before commit or publication.
Keep the plan IN PROGRESS.

Revision 39 (2026-11-03): The next sequential gate pass again passed 11 of 12
gates, including 4,950 tests (7 skipped), all doctests, workflow contracts, and
98.23% doc coverage. Whitaker rejected ten direct std::fs operations in
tests/prover_tools_make_edge_tests.rs under the capability-based filesystem
policy. The edge tests now open their temporary roots as cap_std::fs::Dir and
create, read, and inspect files through that capability. Restart the full
12-gate sequence on this candidate; do not commit or publish until it passes.
Keep Status: IN PROGRESS.

Revision 40 (2026-11-03): The capability-based filesystem repair passed
documentation, workflow, integration, and Makefile gates, but lint, typecheck,
and the Rust test build rejected an unused invocation_log binding. The fake
prover-tools helper now returns only its executable path; the second test reads
its invocation log through the temporary-root Dir. Restart all 12 gates from
the beginning on this candidate before commit or publication. Keep Status: IN
PROGRESS.

Revision 41 (2026-11-03): All 12 repository gates passed sequentially on the
final worktree based on published head
`15f413bac8fcda6a96c344261125aeddbfd2f774`. The run passed `make check-fmt`,
`make lint`, `make typecheck`, `make doc-coverage` (98.23%),
`make markdownlint` (212 files, zero issues), `make nixie`, `make test` (4,950
passed, 7 skipped, plus all doctests), `make test-workflow-contracts` (1,669
passed, 3 skipped, plus 28 helper tests), `make test-ci-scripts` (90 passed),
`make test-prover-tools-integration` (7 passed), `mbake validate Makefile`, and
`git diff --check`. Logs share the suffix
`import-prover-tools-unused-binding-fixed-final`; final build admission was
healthy. CodeScene CLI 1.0.52 reported no new findings for the exact PR base
`e5d57c58352b51473de4170f86e34fdc9db3ece6` through published head
`15f413bac8fcda6a96c344261125aeddbfd2f774`; focused review of the new edge
tests returned score 10.0 with no findings. The hosted CodeScene check on that
head passed all six quality gates with no annotations. Publish the validated
repair, reconcile the current-head formal review and Docstring Coverage row,
and observe trusted-main warm-cache acceptance before completing this plan.
Keep Status: IN PROGRESS.

Revision 42 (2026-11-06): `make kani-check` is now a local read-only version
query, committed as `475d15409` on top of published head
`9a562f33f4b4756758cad322def4d30f8c886dac` and bound to candidate tree hash
`1ca3782f483c9685cdf0cdb40be9cfe18dcd1dd2f37018085495c458a9816de3`. The target
reads `tools/kani/VERSION`, runs the active Cargo home's
`cargo-kani --version`, and compares the first semantic version, so resolution
no longer needs `uv` or the network and writes no uv cache. The new
tests/prover_tools_kani_check_tests.rs poisons `PROVER_TOOLS` and `UV` to prove
no remote runner is consulted and covers a version mismatch, a failing
verifier's exit status, unparsable output, and apostrophes in the active Cargo
home. All 12 gates passed sequentially: `make check-fmt`, `make lint`,
`make typecheck`, `make doc-coverage` (98.23%), `make markdownlint` (212 files,
zero issues), `make nixie`, `make test` (4,953 passed, 7 skipped, plus
doctests), `make test-workflow-contracts` (1,669 passed, 3 skipped, plus 28
helper tests), `make test-ci-scripts` (90 passed),
`make test-prover-tools-integration` (7 passed), `mbake validate Makefile`, and
`git diff --check`. The first Markdown attempt rejected `unparseable` at
docs/developers-guide.md:5200; correcting it to `unparsable` and rerunning the
Markdown gate cleared the finding, and the documentation gates were rerun after
the one-word change. This plan-only revision follows the code commit so the
recorded candidate SHA and gate evidence stay together. Next, publish over SSH,
reconcile the current-head review and pre-merge rows, request the final
approval, and merge once equilibrium holds. Keep Status: IN PROGRESS.

Revision 43 (2026-11-06): The hosted CodeScene check failed on the Revision 42
head with "Code Duplication" (9.38) in tests/prover_tools_kani_check_tests.rs,
where each case repeated the same fake-verifier, version-file, and query setup.
Commit `3a684b0689dc66feecb69add11f4efe0c9d2625e` extracts `run_fake_kani` and
`run_failing_check` helpers and introduces a `PINNED_VERSION` constant, so the
version file takes no argument and each test states only the verifier script it
exercises. The first extraction scored 9.68 on "String Heavy Function
Arguments" because the version string was still threaded through as a
parameter; removing that parameter restored 10.00 with no advisories. The
committed tree hash is `5600d58467a5eba7bb6301a8168ba2610cb1eba4`, identical to
the candidate the gates ran against, and all 12 gates passed sequentially on it:
`make check-fmt`, `make lint`, `make typecheck`, `make doc-coverage` (98.23%),
`make markdownlint` (212 files, zero issues), `make nixie`, `make test` (4,953
passed, 7 skipped, plus doctests), `make test-workflow-contracts` (1,669
passed, 3 skipped, plus 28 helper tests), `make test-ci-scripts` (90 passed),
`make test-prover-tools-integration` (7 passed), `mbake validate Makefile`, and
`git diff --check`. This plan-only revision follows the code commit. Next,
publish over SSH, post the pre-merge reconciliation comment, await the queued
CodeRabbit review, request the final approval, and merge once equilibrium
holds. Keep Status: IN PROGRESS.

Revision 44 (2026-11-06): The follow-up triage on the pre-merge comment
([#6684528489](https://github.com/leynos/netsuke/pull/1063#issuecomment-6684528489))
asked for an isolation boundary test for `kani-check`, a correction to the
formal-verification guide, and reconciliation of this plan's Progress and
Outcomes statements. Commit `2b63540d0c65a28f8a9fd6f46a6abf869ea1bbfb` supplies
all three. The new `kani_check_runs_without_network_or_writable_storage` case
runs the real recipe inside `unshare --user --map-root-user --net --mount` with
the repository root self-bound and remounted read-only; the sandbox asserts
that restriction before running the query, and the test asserts the reported
`repo=read-only`, `temp=read-only`, `ifaces=lo,`, and `dns=blocked` lines, so a
namespace that silently failed to apply fails the case rather than passing it
vacuously. That revision bound only the repository, so its `temp` probe
re-probed the repository rather than the fixture, and the inherited home
directory, cache, and temporary paths stayed writable outside the namespace's
write barrier; Revision 45 replaces it. A host without user namespaces reports
a skip, and `NETSUKE_KANI_CHECK_SANDBOX_STRICT=1` turns that skip into a
failure. The fixture and sandbox machinery moved to
`tests/prover_tools_kani_check_tests/support.rs`, keeping both files under the
400-line cap; both score 10.00 on the CodeScene command-line interface. The
guide now separates installation and Verus proof execution, which delegate to
the pinned CLI through `uv tool run`, from `make kani-check`, which is a local
read-only query over `tools/kani/VERSION` and the installed `cargo-kani`. The
committed tree hash is `a06f75b4dede95743ac060d186d8de38b645ea4e`, identical to
the candidate the gates ran against, and all 11 gates passed sequentially on it:
`make check-fmt`, `make lint`, `make typecheck`, `make doc-coverage` (98.23%),
`make markdownlint` (212 files, zero issues), `make nixie`, `make test` (4,954
passed, 7 skipped, plus doctests), `make test-workflow-contracts` (1,669
passed, 3 skipped, plus 28 helper tests), `make test-ci-scripts` (90 passed),
`make test-prover-tools-integration` (7 passed), and `make test-package-verify`
(publish dry run). This plan-only revision follows the code commit. Next, post
the follow-up comment, await the CodeRabbit review, request the final approval,
and merge once equilibrium holds. Keep Status: IN PROGRESS.

Revision 45 (2026-11-06): CodeRabbit's reply on the pre-merge thread
([#6688359015](https://github.com/leynos/netsuke/pull/1063#issuecomment-6688359015))
accepted the guide correction and the Progress/Outcomes reconciliation but
kept the isolation-boundary finding open with four repairs: probe the fixture
directory rather than the repository twice, block writes outside the two
mounts, enforce the filesystem-write boundary for the query process so the
inherited `HOME`, the uv cache, and temporary paths are not writable, and
verify that attempted writes outside the repository and fixture fail. Revision
44 satisfied none of the last three: it bound only the repository, so
`temp=read-only` re-probed the repository, and every path outside the namespace
stayed writable in principle. That revision record has been corrected in place,
and this revision supersedes it.

The sandbox is now a read-only chroot built inside
`unshare --user --map-root-user --net --mount --pid --fork`. The chroot root is
a fresh temporary directory, self-bound and remounted `ro,bind`; `/usr`,
`/etc`, the repository, and the fixture are bound on top of that read-only root
and remounted read-only; `/proc` is a fresh procfs mounted over it. `/bin`,
`/sbin`, `/lib`, and `/lib64` are symlinks into `/usr`, so the query sees only
read-only mounts, and every other path — the inherited home directory, the uv
cache, `/tmp`, and the chroot root itself — is a read-only directory or absent.
The process namespace is what makes the procfs mount possible; the network
namespace leaves `lo` as the only interface, matching the existing `ifaces=lo,`
assertion.

Each restriction is verified rather than assumed. The inner script reports
`control=writable` for `/proc/self/comm`, a path that really is writable, which
proves the probe distinguishes a protected path from a broken redirection; it
then classifies the attempt to create a file in the repository, the fixture,
the chroot root, `/usr`, `/etc`, `/tmp`, and `$HOME` by the errno the kernel
returns, reporting `read-only`, `absent`, or `other`. The test requires
`control=writable`, `repo=read-only`, `fixture=read-only`, `rootdir=read-only`,
`usr=read-only`, `etc=read-only`, `tmp=read-only`, `home=read-only`,
`ifaces=lo,`, `dns=blocked`, and `Kani 0.67.0 matches`, and rejects any line
other than `control=writable` that ends in `=writable`. The observed output on
this host is exactly that list. The `temp=read-only` line is gone because it
was the repository probed twice; `fixture=read-only` replaces it.

A host that forbids unprivileged user namespaces cannot demonstrate the
boundary. `sandbox::unavailable` probes the same namespace set the sandbox
requests, and the case then calls `sandbox::skip`, which prints
`skipped: kani-check isolation — <reason>` to standard error and returns
`Ok(())`. That is a reported prerequisite skip, not a native test skip: nextest
records the case as passed with zero skipped, so the reason is visible only in
the captured output. `NETSUKE_KANI_CHECK_SANDBOX_STRICT=1` converts it into a
failure, following the precedent of `NETSUKE_REQUIRE_NINJA`, and remains
available to a lane that must exercise the boundary. This revision left it
unset in CI; Revision 46 corrects that with a dedicated `kani-smoke` step, so
the required lane enforces the boundary rather than reporting it.

The sandbox machinery moved to `tests/prover_tools_kani_check_tests/sandbox.rs`
because the combined file reached 413 lines and the `module-max-lines` lint
caps a module at 400. The three files are 172, 254, and 172 lines, and each
scores 10.00 on the CodeScene command-line interface. The outer and inner
scripts are raw string constants so the shell braces survive formatting, and
the inner script is passed to `sh -c` as a single-quoted literal, which is why
it uses no apostrophes.

The committed tree hash is `fe679d954f064187ab642572bfc09640649e5dec`,
identical to the candidate the gates ran against, and all 11 gates passed
sequentially on it: `make check-fmt`, `make lint`, `make typecheck`,
`make doc-coverage` (98.23%), `make markdownlint` (212 files, zero issues),
`make nixie`, `make test` (4,954 passed, 7 skipped, plus doctests),
`make test-workflow-contracts` (1,669 passed, 3 skipped, plus 28 helper tests),
`make test-ci-scripts` (90 passed), `make test-prover-tools-integration` (7
passed), and `make test-package-verify` (publish dry run). The code commit is
`b45b19da5d00f6c2bfeb63681384743a24f2a853`. This plan-only revision follows it.
Next, post the corrective follow-up comment, await the CodeRabbit review,
request the final approval, and merge once equilibrium holds. Keep Status: IN
PROGRESS.

Revision 46 (2026-11-06): The review at head `8c80761db` raised four inline
findings and an earlier trivial one. Commit `231ec7344` supplies the repairs,
and CodeRabbit marked the four inline findings addressed in
`8c80761`..`231ec73`.

The chroot now creates `/tmp` and the caller's home directory before the root
is remounted read-only. Revision 45 left both paths outside the mounts, so the
probe reported `absent` where the assertion required `read-only`, and the home
probe dereferenced `$HOME` under `set -u`. Both directories are now proven
unwritable rather than merely missing, an unset `HOME` is guarded with
`${HOME:-}` and reported as `home=absent`, and the test accepts either reading
for `home` while still refusing any `=writable` line other than `control`.
Findings
[#4495296252](https://github.com/leynos/netsuke/pull/1063#discussion_r4495296252)
and
[#4495296271](https://github.com/leynos/netsuke/pull/1063#discussion_r4495296271).

Creating the namespaces does not prove the sandbox can build its chroot: a host
may admit `unshare` and still refuse a bind mount, a read-only remount, a
procfs mount, or `chroot`. `unavailable()` now runs exactly that sequence on a
throwaway directory inside the namespace, with a distinct exit status for each
step, and `probe_stage` names the refused operation in the reported skip. A
host that can do only part of the sequence is therefore reported, not failed
from inside the real script. Finding
[#4495296289](https://github.com/leynos/netsuke/pull/1063#discussion_r4495296289).

The sandbox module, its `SandboxPaths` import, and `run_sandboxed_check` are
gated with `#[cfg(target_os = "linux")]`, so a non-Linux Unix build compiles
them out instead of carrying dead code that `-D warnings` rejects. Finding
[#4495296304](https://github.com/leynos/netsuke/pull/1063#discussion_r4495296304).

The trivial finding
[#4493213472](https://github.com/leynos/netsuke/pull/1063#discussion_r4493213472)
asked for a boundary test that blocks and records network attempts and
inspects an isolated uv cache for writes. Revision 45 already supplies the
first half: the sandbox runs in a network namespace whose only interface is
`lo`, the case asserts `ifaces=lo,` and `dns=blocked`, and a DNS lookup is
attempted inside the sandbox so a namespace that failed to apply cannot pass.
The cache half is subsumed rather than omitted. `.uv-cache` and `.uv-tools`
live inside the repository, which is bound read-only as a whole, and
`$HOME/.cache/uv` lies under the home directory, which the `home` probe covers
by the same rule. A read-only mount is enforced by the kernel for every
descendant, so a probe of a cache subdirectory cannot fail while
`repo=read-only` passes; the existing catch-all assertion rejects any
`=writable` line. The recipe also invokes neither `uv` nor the network, and both
`PROVER_TOOLS` and `UV` are poisoned, so an attempt would fail loudly rather
than silently succeed.

`kani-smoke` now runs `make test-kani-check-isolation` as its own step. That
target exports `NETSUKE_KANI_CHECK_SANDBOX_STRICT=1`, so the required lane
fails when the boundary cannot be demonstrated instead of recording a skip that
nextest counts as a pass. `NEXTEST_TARGETS` gains the target, which the nextest
contract test requires of every target that invokes `cargo nextest run`.

`make test-prover-tools-integration` now passes `--with hypothesis`, which its
property test already imported. The Python suite also resolves `make` through
`shutil.which` rather than a bare name, and both multi-line docstrings carry
the numpydoc `Returns` section the house lints require.

The committed tree hash is `bc87710068100639776c815581e65421b94f1b0b`,
identical to the candidate the gates ran against. All 16 gate targets passed
sequentially on it: `make check-fmt`, `make lint-clippy`, `make lint-whitaker`,
`make lint-python`, `make github-actions-lint`, `make typecheck`,
`make doc-coverage` (98.23%), `make markdownlint` (212 files, zero issues),
`make nixie`, `make test-nextest` (4,957 passed, 7 skipped), `make doctest`,
`make test-workflow-contracts` (1,669 passed, 3 skipped, plus 28 helper tests),
`make test-ci-scripts` (90 passed), `make test-prover-tools-integration` (11
passed), `make test-documentation-contracts` (15 passed, 48 skipped), and
`make test-package-verify` (711 files packaged). `make lint` is decomposed into
its four documented prerequisites only to fit the harness's per-command limit;
the set is the one the target runs. The code commit is `231ec7344`. This
plan-only revision follows it. Next, post the corrective follow-up comment,
await the CodeRabbit review, request the final approval, and merge once
equilibrium holds. Keep Status: IN PROGRESS.

Revision 47 (2026-11-06): The first hosted run of the new boundary step failed
before the sandbox ran. Run `42275955402`, job `128092206020`, step 14, "Verify
the kani-check isolation boundary", at head `231ec7344`:

```text
Error: the kani-check isolation sandbox could not be exercised: the isolation
sandbox cannot run the capability probe: unshare: write failed
/proc/self/uid_map: Operation not permitted (strict mode requires the boundary
test to run for real)
```

The lane runs on `ubicloud-standard-2-ubuntu-2404`, and Ubuntu 24.04 ships
`kernel.apparmor_restrict_unprivileged_userns=1`, which confines unprivileged
user namespaces with AppArmor. `unshare --user` therefore fails at the
`uid_map` write, before the capability probe can report a stage. The strict
variable did its job: the lane failed rather than recording a skip that nextest
would count as a pass.

The step now lifts that restriction for its own duration and restores it
afterwards, preserving `make`'s exit status across the restore so a genuine
sandbox failure is still reported as itself. A host that will not let the
restriction be lifted fails with an `::error::` naming the reason, rather than
reporting a sandbox failure that never reached the sandbox. The local host is
Rocky 10, which has no AppArmor and admits unprivileged user namespaces, so the
repair is exercised only by the hosted lane; locally
`make test-kani-check-isolation` ran 9 tests with 0 skipped, so the boundary
itself was exercised for real.

The hosted lane then confirmed the repair at head `9430d0f93`: run
`42283994996`, job `128119970127`, step 14. The log records
`kernel.apparmor_restrict_unprivileged_userns = 0` after the lift, then
`9 tests run: 9 passed, 0 skipped`, then `= 1` after the restore. The required
lane therefore enforces the boundary for real, rather than reporting a skip on
the one host whose policy would otherwise forbid it.

The committed tree hash is `d4627899165a7826d471f17de458df6997aad26d`, and all
13 gate targets passed sequentially on the candidate that carried this revision:
`make check-fmt`, `make lint-clippy`, `make lint-whitaker`, `make lint-python`,
`make github-actions-lint`, `make typecheck`, `make doc-coverage` (98.23%),
`make markdownlint` (212 files, zero issues), `make nixie`,
`make test-kani-check-isolation` (9 passed, 0 skipped), `make test-nextest`,
`make doctest`, and `make test-workflow-contracts`. The code commit is
`9430d0f93`. This plan-only revision follows it. Next, post the corrective
follow-up comment, await the CodeRabbit review, request the final approval, and
merge once equilibrium holds. Keep Status: IN PROGRESS.

Revision 48 (2026-11-06): The review at `5803947003` recorded six failed
pre-merge checks. Each names a concrete gap, and all six are now addressed by
the working candidate; the published repairs recorded above are unchanged.

*Testing (Overall)* asked for different and unreadable version files, a
`--version` requirement on the fake verifier, and detection of network and
filesystem or cache write attempts, including attempts whose errors are ignored.
`tests/prover_tools_kani_check_command_tests.rs` now drives the recipe through
a matching pin, a mismatch, a missing pin and a pin replaced by a directory,
and asserts `invocation_args() == ["--version"]` wherever the verifier is
reachable. The write-attempt half stays with the read-only chroot boundary in
`tests/prover_tools_kani_check_tests.rs`, which the required `kani-smoke` lane
runs strictly, so a host that cannot enforce the boundary fails instead of
recording a skip.

*Testing (Unit And Behavioural)* asked for an end-to-end run of `make verus`
with the pinned CLI and a real verifier, covering the default proof, an ordered
batch and failure propagation. `tests/prover_tools_verus_e2e_tests.rs` adds
those three cases. They skip on a host without the pinned install, which is the
documented Verus policy; the new `test-verus-e2e` target sets
`NETSUKE_VERUS_E2E_STRICT=1` so a maintainer who means to run them gets a
failure rather than a skip.

*Testing (Property / Proof)* asked for a property test over batch lengths,
proof orders, failure positions and exit statuses. The check is implemented with
`proptest` rather than Hypothesis: the suite is Rust, the repository's
property-testing convention is `proptest`, and the pinned CLI is driven from
Rust here. `verus_runs_every_proof_of_a_successful_batch_in_order` asserts the
full invocation order, `--version` then every proof, and
`verus_stops_at_the_first_failing_proof_and_propagates_its_status` asserts the
exact executed prefix plus the reported status. Only the verifier is fake; the
batching loop under test is the pinned CLI's own, and the real-verifier cases
cover the same behaviour end to end. Both property cases sit behind the same
installed-verifier gate as the rest of the module, so a host without the pinned
Verus skips them rather than fetching the pinned CLI from the network during
`make test`.

*Developer Documentation* asked for Progress and Outcomes to separate published
repairs from the newest candidate, for the chroot isolation change and its
validation to be recorded, for the version-checking description to be
corrected, and for the pending review and trusted-main warm-cache items to stay
visible. The Outcomes section now makes those distinctions, and Revision 47
records the AppArmor lift together with the hosted run that confirmed it.

*Unit Architecture* asked that the installed verifier be exercised through
`make kani-check` with network access and filesystem writes blocked, that a
required lane run the isolation test rather than skip it, and that the version
file, mismatch, verifier-failure and remote-runner checks be retained. The
boundary suite, its strict lane and all four checks are retained; the new
command suite adds coverage of the recipe itself without replacing any of them.

*Observability* asked for a safe, redacted verifier diagnostic or an actionable
error category on failure, plus a regression test. The `kani-check` recipe now
emits one of `category=pin-unreadable`, `category=verifier-failed`,
`category=version-unparsable` or `category=version-mismatch`, and takes only a
version token from the verifier's output: the `found` value of a mismatch, and
the version named by the success line, so a verifier reporting `kani 0.66.0`
against a 0.67.0 pin prints `found 0.66.0`. The `kani-version=` field is the
expected pin as read from the file, not verifier output. The text that token
came from is never echoed. Redaction was rejected: that text comes from an
external binary, so it could carry a credential, forge a surrounding log line
or flood a terminal, and bounding plus escaping does not make arbitrary text
safe to print. The extracted version, the category and the exit status are the
actionable facts, and a hand-run of the verifier supplies anything else.
`kani_check_reports_a_category_without_echoing_verifier_output` presents a fake
secret, an ANSI escape and a forged `category=` line and asserts that none of
them reach the diagnostic, while
`kani_check_reports_unparsable_output_without_echoing_it` does the same for the
unparsable branch.

`tests/makefile_test_target.rs` enumerates every target that invokes
`cargo nextest run` and fails when that list and the file disagree, so the new
`test-verus-e2e` target joins `NEXTEST_TARGETS`; it forwards both worker bounds
like every other entry. `dylint.toml` lists both new integration crates under
`no_std_fs_operations.excluded_crates`, because each manipulates its own
temporary fixture directory. The local batch then ran sequentially to
completion: `make check-fmt`, `make lint`, `make typecheck`,
`make doc-coverage`, `make markdownlint`, `make nixie`, `make test`,
`make test-workflow-contracts`, `make test-kani-check-isolation` and
`make test-verus-e2e` each exited zero. The last two are the load-bearing ones
here: the isolation lane ran the read-only chroot case with strict mode on
rather than skipping it, and the Verus lane ran all five cases against the
pinned real verifier. Keep Status: IN PROGRESS pending review at the new head
and trusted-main warm-cache acceptance.

Revision 49 (2026-11-07): The review at `5808594893`, taken on `5b682a908`,
repeated the six pre-merge rows with sharper wording and added four inline
findings; the CodeScene delta on `adeb43dd6` then raised two new issues against
the new command suite. All are addressed on the working candidate.

*Testing (Overall)* and *Unit Architecture* asked for detection of network and
cache write attempts, including attempts whose errors are ignored, and for
attempted writes that fail silently.
`kani_check_makes_no_network_or_cache_attempts` now runs the recipe with a
recorder in place of both network-capable tools the repository defines. The
recorder marks its invocation *before* exiting non-zero, so an attempt is
visible even when the caller ignores the failure, and the isolated cache and
home directories must still be empty afterwards, which catches a write that
succeeded rather than one that failed. The read-only chroot remains the
evidence that the boundary holds for real, and the required `kani-smoke` lane
still runs it strictly.

*Developer Documentation* asked for the older gate run to be marked historical,
for the isolation summary to name the writable control path, and for the
current-head review and warm-cache items to stay open. The Outcomes section now
says the Revision 48 evidence is historical, names `/proc/self/comm` as the one
deliberately writable control path, and keeps both acceptance items open.

*Module-Level Documentation* asked the integration module to say that it also
exercises `make verus` through the pinned CLI and a fake verifier. The module
docstring now states that, and names the Rust suite that covers the same paths
against the real verifier.

The four inline findings are fixed. The Python fixtures now write a `#!/bin/sh`
launcher that passes the interpreter and the script as separate quoted
arguments, so a `sys.executable` path containing spaces no longer breaks the
shebang. Every subprocess runs through one bounded runner that starts the child
in its own session, kills the whole process group on expiry, and reports the
command, so a stalled fetch cannot hold the captured pipes open. The
default-proof test scrubs `VERUS_FLAGS` and `MAKEFLAGS` from the child
environment so an inherited override cannot change the recipe under test. The
sandbox capability probe now binds `/etc`, the repository and the fixture with
the same paths and read-only remounts the real sandbox uses, because a
non-recursive bind of a directory containing an inherited submount can fail
even when the simpler binds succeed; an unavailable mount is still a reported
skip in normal mode and a failure in strict mode.

The two CodeScene issues were real. The command suite had four near-identical
constructors and script writers, and its string-argument ratio was 43.8%
against a 39.0% threshold. A single `Behaviour` enum now selects the fake
verifier's response, one script builder renders it, and one constructor stages
the fixture, which removes the duplication and drops the string-argument share
below the threshold. The Python integration suite crossed the 400-line cap once
the bounded runner and launcher were added, so its fixtures and runners moved to
`scripts/tests/prover_tools_integration_support.py`, following the existing
`*_support.py` convention in that directory.

Three Python lints fired on the relocated code and are fixed at the source
rather than suppressed. The bounded runner raised `AssertionError` with a long
formatted message, which the df12 house lints reject twice over, so it now
raises a `CommandTimeoutError` that builds the message itself and names the
stalled command and its captured output. Pylint's `consider-using-with` then
required the `Popen` call to be a context manager; that is safe here because
both paths reap the child before the block exits, which the comment at the call
site records. Moving `pathlib` into a `TYPE_CHECKING` block broke collection
instead: Hypothesis resolves a decorated test's annotations through
`inspect.signature`, so the import is needed at runtime. It stays a runtime
import with a scoped ignore and that reason, matching the existing Cyclopts
precedent in `scripts/ci_test_evidence.py`.

The eleven commit gates then ran sequentially, one at a time, each started
once, and all passed on this candidate: `make check-fmt`, `make lint`,
`make typecheck`, `make doc-coverage`, `make markdownlint`, `make nixie`,
`make test`, `make test-workflow-contracts`,
`make test-prover-tools-integration`, `make test-kani-check-isolation`, and
`make test-verus-e2e`. They reported 5,180 nextest tests passed with 7 skipped,
every doctest target passing, 1,785 workflow-contract tests passed with 3
skipped plus 28 further cases, 11 prover-tools integration tests, 9
`kani-check` isolation tests, 5 Verus end-to-end tests, and 98% doc-comment
coverage. Compile admission was healthy throughout; queueing under four held
slots lengthened `make lint` and `make doc-coverage`, and neither queued gate
was treated as failed or restarted.

Keep Status: IN PROGRESS pending review at the new head and trusted-main
warm-cache acceptance.

Revision 50 (2026-11-07): The pull request reported a conflict against `main`
after the review-round repair was pushed, because `main` had advanced to
`6ff836603`. The branch was rebased onto that commit. Three commits conflicted,
all on the single `.PHONY` line, because `main` added a `fetch` target while
this branch added `test-prover-tools-integration`, `test-kani-check-isolation`
and `test-verus-e2e`. Each conflict was resolved as a union: the branch's own
targets plus `main`'s `fetch`, and `interrogate` kept because it is a real
target that `main` still lists. Nothing else in the diff changed, which a diff
of the pre-rebase head against the rebased head confirms: only the ten files
`main` touched differ. All eleven commit gates were then run again from the
beginning on the rebased candidate and all passed, with the same counts the
preceding paragraph records. Keep Status: IN PROGRESS pending review at the new
head and trusted-main warm-cache acceptance.

Revision 51 (2026-11-08): The review at `5816957501` raised two inline findings
and six pre-merge rows against the rebased head. Both inline findings are fixed.
`run_make_verus` and `run_make_verus_with_env` now remove `VERUS_FLAGS` and
`MAKEFLAGS` from the child environment, so an inherited value cannot change the
proof set a test runs. The repeated stdout/stderr merge became one
`output_text` helper, and the file was then split: the fixtures and runners
moved to `tests/prover_tools_verus_e2e_tests/support.rs`, leaving the parent at
206 lines and the support module at 303, both inside the 400-line cap.

The ordered-batch case now runs two distinguishable proofs through the real
verifier. The verifier prints one result line per proof and names no file, so
the case uses a minimal proof whose verified count differs from the canonical
kernel's, runs the pair in both orders, and asserts that reversing the proof
order reverses the result order. The default-proof and failure-propagation
cases are retained unchanged.

The Observability row asked for a safe redacted diagnostic or an actionable
category. Redaction was rejected in favour of the category alone: the
`kani-check` recipe no longer echoes the verifier's output at all, because
bounding and escaping arbitrary text from an external binary does not make it
safe to print. `kani_check_reports_a_category_without_echoing_verifier_output`
presents a fake secret, an ANSI escape and a forged `category=` line and
asserts that none reach the diagnostic, and the boundary suite's unparsable
case now asserts the category and the absence of the verifier's text.

All eleven commit gates were run sequentially on this candidate and each exited
`0`: `check-fmt`, `lint`, `typecheck`, `doc-coverage` (98.33% against an 80%
threshold), `markdownlint` (218 files, zero issues), `nixie`, `test` (5,181
nextest passed, 7 skipped, plus all doctests), `test-workflow-contracts` (1,785
passed, 3 skipped, plus 28), `test-prover-tools-integration` (11 passed),
`test-kani-check-isolation` (9 passed, 0 skipped) and `test-verus-e2e` (5
passed, 0 skipped). Keep Status: IN PROGRESS pending review at the new head and
trusted-main warm-cache acceptance.

Revision 52 (2026-11-08): Hosted CI on `772834947` failed two checks, and
neither was caused by this branch's diff. `build-test` aborted with
`thread 'rustc' panicked … duplicate specification of lint
string_continuation_style`,
and `Windows / lint-windows` failed with
`could not find dylint_version in …\lib\string_continuation_style@…dll`. Both
are the same defect: the Whitaker rolling release now publishes a standalone
library for every lint as well as `whitaker_suite`, and the suite already
registers all of them, so `whitaker --all` loads several lints twice. On Linux
the second registration trips rustc's lint-store assertion; on Windows the
duplicated standalone DLL fails its own metadata check. The previous head,
`24167c18e`, was green only because it ran before the archive changed: the
rolling release is republished on every push to Whitaker's `main`, and the
pinned installer (`installer-version: '0.2.11'`) predates the new crate, so its
pruning loop never removes the extra library. Upstream tracks this as
`leynos/whitaker#565`, which is still open and has no fixed installer release.

The gate now selects the suite by name instead of taking `--all`. A new
`WHITAKER_SUITE_SELECTION` Make variable carries
`--lib whitaker_suite --fail-on-no-libraries`, and both `lint-whitaker`
invocations compose it. The Windows job splats the same selection from a
`$suite` array, because the explicit flags pushed the command past yamllint's
120-column limit. This is still a whole-suite load, so a lint added upstream
arrives without a change here; it is not the per-lint `pattern` selection the
Whitaker guide's Netsuke deviation note rules out. `--fail-on-no-libraries` is
the other half of the fix: without it a missing suite would leave Whitaker
running a lint-free `cargo check` that reports success, which is exactly the
"quietly stops being a gate" failure the Windows contract test exists to
prevent.

Two contract tests pin the new shape, and both were probed for liveness by
reverting the change and watching them fail.
`lint_whitaker_selects_the_suite_and_requires_it` in
`tests/whitaker_boundary_contract.rs` asserts that every invocation composes
the variable, that none passes `--all --no-deps`, and that the variable carries
both fragments. The Windows fragments in
`tests/workflow_contracts/ci_windows_lint_test.py` now require the `$suite`
definition and the splatted invocations. `AGENTS.md`, the developers' guide
(including its Windows reproduction snippet) and the Whitaker users' guide were
updated to match, the last with a Netsuke deviation note so a re-import does
not restore `--all`.

All eleven commit gates were run sequentially on this candidate and each exited
`0`: `check-fmt`, `lint`, `typecheck`, `doc-coverage` (98.33% aggregate,
6,762/6,877, against an 80% threshold), `markdownlint` (218 files, zero issues),
`nixie`, `test` (5,182 nextest passed, 7 skipped, plus all doctests),
`test-workflow-contracts` (1,785 passed, 3 skipped, plus 28),
`test-prover-tools-integration` (11 passed), `test-kani-check-isolation` (9
passed, 0 skipped) and `test-verus-e2e` (5 passed, 0 skipped). Keep Status: IN
PROGRESS pending review at the new head and trusted-main warm-cache acceptance.

Revision 53 (2026-11-08): Hosted CI on `68afdf0ad` finally reached the Linux
lint step and failed there on a real finding rather than a download.
`build-test` attempt 6 cleared `Install Whitaker` and stopped in step 23,
`Lint`, with

```text
error: Join the physical lines in this cooked string with `concat!()`.
   --> tests/prover_tools_kani_check_tests/support.rs:159:17
error: could not compile `netsuke-build` (test "prover_tools_kani_check_tests")
```

The rule is `string-continuation-style`, and the trigger was a shell script
written as one Rust string literal continued with a trailing backslash.
`poison_runner` now builds it from two `concat!()` arguments. The cooked bytes
are unchanged: the continuation had already swallowed the second line's leading
indentation, so the second argument starts at `printf` with no padding.

Attempts 2 to 5 were extinguished by the GitHub incident "Increase in timeouts
for some unauthenticated users" (opened 2026-11-08T10:36Z), which failed the
sccache, cargo-binstall, kani-verifier, mdtablefix and Whitaker downloads; the
same URLs timed out from this host while `codeload.github.com` answered. Local
`make lint` had been green on the identical tree because the locally installed
suite library dated from 2026-11-04 and predated the lint. The rolling archive
is republished on every push to Whitaker's `main`, so a green local run does
not cover a lint the archive has gained since: `whitaker-installer install` now
fetches the prebuilt `whitaker-lints-9fc35c0-nightly-2026-05-28` suite, and
`make lint-whitaker` passes both workspace packages against it.

All eleven commit gates were then run sequentially on this candidate and each
exited `0`: `check-fmt`, `lint`, `typecheck`, `doc-coverage`, `markdownlint`,
`nixie`, `test`, `test-workflow-contracts`, `test-prover-tools-integration`,
`test-kani-check-isolation` and `test-verus-e2e`. Keep Status: IN PROGRESS
pending review at the new head and trusted-main warm-cache acceptance.

Revision 54 (2026-11-09): The review at `5819530205`, taken at `a374b6bcd`,
raised two inline findings, one outside-diff finding and five pre-merge rows,
and requested changes. An earlier review at the same head (`5818141041`,
13:52Z) had approved without listing rows; the later review (20:27Z) governs.
Both inline findings were fixed. `tests/whitaker_boundary_contract.rs` was 400
lines and is now 316, with the two `make lint-whitaker` invocation contracts
moved to `tests/whitaker_boundary_contract/whitaker_invocation.rs` (177 lines)
behind `#[path]`, alongside the module-doc paragraph that explains the split.
The `--all` check no longer looks for the `--all --no-deps` pair: it cuts the
recipe line at the recipe's ` -- ` separator, tokenizes the library-selection
arguments, and rejects an exact `--all` token anywhere in them, which is what
catches the dangerous `--all $(WHITAKER_SUITE_SELECTION) --no-deps` spelling.

The outside-diff finding was a documentation correction, and it was right: the
`kani-check` recipe prints the version token it extracted on a mismatch as well
as on a match, so "never echoes verifier output" overstated the guarantee. A
later review found the same sentence overstating the other way: the
`kani-version=` field is the expected pin read from the file, not an extracted
token. The developers' guide and both places in this plan now name only the
values actually taken from the verifier — the `found` value of a mismatch and
the version named by the success line — and never the text they came from. A
verifier reporting `kani 0.66.0` against a 0.67.0 pin prints `found 0.66.0`.

The Testing (Overall) and Unit Architecture rows asked for tests that detect
write attempts, including attempts whose errors are ignored. The sandbox case
proves a write cannot succeed; it cannot show that the recipe never tries,
because a discarded failure (`2>/dev/null`, `|| true`) leaves success behind.
`tests/prover_tools_kani_check_tests/recipe.rs` supplies the other half: it
parses the shipped `kani-check` recipe and reports every error-suppression
fragment, every write-intent command, and every redirect that names a path
rather than duplicating a descriptor. Eight representative suppressions and
writes are asserted to be reported, and three standard-stream diagnostics to be
accepted, so the check is known to fire rather than merely known to pass. Adding
`mkdir -p "$cache" 2>/dev/null || true` to the recipe was probed and fails the
contract with two findings. The strict `kani-smoke` lane and the existing
version-file, mismatch, verifier-failure and remote-runner checks are unchanged.

The Testing (Property / Proof) row asked that generated failure statuses
account for Make's normalized exit status. The two property cases already
generate batch length, failure position and status, assert the complete
successful order, and assert the exact failed prefix and the propagated status;
what was missing was the reason for the bound. `failure_cases` now records it:
statuses stay inside `1..=125` because 126 and 127 are the shell's meanings and
128 upwards denote signals, so a proof exiting with one of those would be
reported as the shell's number rather than its own.

The Observability row asked for a restore failure to be an actionable CI error
that fails the step, preserving a suite failure when applicable, and for the
path to be covered. The `kani-smoke` isolation step now takes the sysctl path
from `KANI_APPARMOR_RESTRICTION`, so the checked-in body can be executed with
`make` and `sudo` stubbed and the path pointed at a temporary file.
`tests/workflow_contracts/kani_isolation_step_test.py` runs that body and
covers seven cases: no restriction node, an unrestricted host, a restored lift,
a preserved suite failure, a refused restore with a passing suite (annotation
plus non-zero exit), a refused restore with a failing suite (annotation plus
the suite's own status), and a refused lift (an `::error::` before the suite
runs, nothing restored). Removing the status promotion from the step was probed
and fails the refused-restore case.

All eleven commit gates were run on this candidate and each exited `0`. The
batch was interrupted twice by the two-hour detached-command cap, once during
`lint-whitaker` and once during `test`, and was resumed each time; every gate
listed here ran on the unchanged tree, whose last file change preceded the
resumed `lint` run. The results were `check-fmt`, `lint`, `typecheck`,
`doc-coverage` (98.33% aggregate, 6,762/6,877, against an 80% threshold),
`markdownlint` (218 files, zero issues), `nixie`, `test` (5,202 nextest passed,
7 skipped, plus all doctests), `test-workflow-contracts` (1,792 passed, 3
skipped, plus 28), `test-prover-tools-integration` (11 passed),
`test-kani-check-isolation` (29 passed, 0 skipped) and `test-verus-e2e` (5
passed, 0 skipped). Keep Status: IN PROGRESS pending review at the new head and
trusted-main warm-cache acceptance.

Revision 55 (2026-11-09): The review at `5820412422` approved `d80aaa07d`, and
the follow-up comment at `6729345451` re-emitted four of the five pre-merge
rows as still Open. Each was answered by a change, not by an argument, except
where the row's premise did not survive inspection.

The Testing (Overall) and Unit Architecture rows asked for detection of an
attempted write or connection whose error the caller discards. The static
contract in `recipe.rs` cannot reach every shell spelling the row named
(`if wget https://example.com; then :; fi`), so the sandbox now records what
the query actually called. `tests/prover_tools_kani_check_tests/trace.rs` turns
an `strace -f -qq` log into findings, reporting a write-intent `open`, a
path-mutating syscall, or an `AF_INET`/`AF_INET6` `connect` whether or not the
kernel refused it, and accepting the character devices a diagnostic
legitimately writes. The syscall list is templated from the reader into the
sandbox script, so a call the reader looks for but the trace omits cannot
produce a clean run it never saw; a compile that dropped the template failed
with `constant TRACE_SYSCALLS is never used`. The reader has its own unit
cases: six suppressed attempts are reported, three read-only or local uses are
accepted, and a clean log reports nothing.

Two defects surfaced while making the traces real. The control verifier first
shared `fake_kani`'s file name, so it replaced the query verifier and both runs
failed; it is now `control-kani`. Its write then used `:`, a POSIX special
builtin, and a failed redirection on a special builtin aborts the shell that
runs it — so `|| true` never ran and the control exited before printing its
version. The attempt now runs in a subshell, and the chroot binds `/dev/null`,
which the shells and the recipe's own diagnostics also need. The isolation case
requires the query's trace to be empty and the control's trace to name both a
write-intent and a network attempt, with `control-exit=0` proving the failures
were discarded rather than fatal.

A `kani-smoke` step installs `strace` before the isolation step. The target's
strict mode turns an absent tracer into a failure, and the job can land on two
runner pools that are not built from one image recipe, so assuming the tracer
would be a way to go green locally and red in the lane that requires the
boundary. The reader is the oracle, and the lane that demands the boundary now
pays for the tool.

The Developer Documentation row was right on every point. The developers' guide
and both places in this plan described the `kani-version=` field as verifier
output; it is the expected pin read from the file. Only the `found` value of a
mismatch and the version named by the success line come from the verifier, and
all three sites now say so. The 4,950-test sweep is attributed to Revision 41
rather than to a "final candidate", and the sentence that attributed the
preceding evidence to Revision 48 now names Revision 41, which is the entry
that records that run.

The Testing (Property / Proof) row asked for generated orders and generated
statuses. In the Python suite the failing-order property now generates the
verifier status across `1..=125` and asserts it is propagated unchanged, and a
new property generates orderings of a batch in which nothing fails and requires
every proof to run in the order given. In the Rust suite
`BatchFixture::in_order` stages proofs in an explicit order while each keeps
the name of its original index, `ordered_failure_cases` generates a permutation
with a failing position and status, and `permutation` reaches every ordering by
sorting indices on random keys. A third property asserts the log is the
permutation up to the failure and stops there, which the creation-order cases
could not have caught.

All eleven commit gates were run on this candidate and each exited `0`:
`check-fmt`, `lint` (which caught two findings, `clippy::indexing_slicing` in
the permutation strategy and the Whitaker capability-filesystem rule in the
trace reader, both fixed and re-run), `typecheck`, `doc-coverage` (98.33%
aggregate, 6,762/6,877), `markdownlint` (218 files, zero issues), `nixie`,
`test` (5,213 nextest passed, 7 skipped, plus all doctests),
`test-workflow-contracts` (1,792 passed, 3 skipped, plus 28),
`test-kani-check-isolation` (39 passed, 0 skipped),
`test-prover-tools-integration` (12 passed) and `test-verus-e2e` (6 passed, 0
skipped). The isolation and Verus counts rise by the ten trace-reader cases and
the one generated-order property. Keep Status: IN PROGRESS pending review at
the new head and trusted-main warm-cache acceptance.

Revision 56 (2026-11-09): The pre-merge comment at `6731095126` re-emitted
three rows as still Open — Testing (Overall), Unit Architecture and Developer
Documentation — after Revision 55's changes. The trace reader proved that a
discarded write or connection by the *stub* is caught; it did not yet show the
real `cargo-kani` answering inside the boundary. The isolation case now runs a
third traced query with the verifier named by
`NETSUKE_KANI_ISOLATION_VERIFIER`, and the `kani-smoke` lane sets it to the
`cargo-kani` that lane installed, so a lane that installs nothing cannot pass
vacuously — the case fails rather than skips when the variable names a path it
cannot resolve.

That query needed more than the binary. `cargo-kani` is a launcher as well as a
driver: it resolves its payload from the Kani home and, with no readable home,
starts a first-time setup that downloads a release and installs a toolchain —
which in a networkless read-only chroot exits `2` with no version. The sandbox
therefore binds the verifier's directory at `/opt/verifier` and the Kani home
at its own absolute path, both read-only, and exports `KANI_HOME`; the
capability probe binds the same five path kinds and reports the operation a
host refused, so a host that cannot build either mount names its prerequisite
instead of failing inside the script. Run by hand with `KANI_HOME=$HOME/.kani`
and the local `cargo-kani 0.67.0`, the case passes with `installed-exit=0` and
an attempt-free trace.

The trace reader also covers the calls it was missing: `openat2` and the whole
`open` family, and `sendto` and `sendmsg` alongside `connect`, since a datagram
can carry a peer address without a connection. Each has a positive case and a
read-only or local negative case, and the three attempt families are split so a
failure names one condition.

The added machinery pushed `support` past the 400-line cap and `sandbox` to
419, so both are decomposed along their seams rather than padded or suppressed:
`support/` holds the installed-verifier lookup and payload resolution
(`installed.rs`), the query staging and trace read-back (`run.rs`), and the
fixture core; `support/sandbox/` holds the script and mount plan (`mod.rs`) and
the capability probe (`probe.rs`). Clippy's `self_named_module_files` requires
the `mod.rs` spelling once a directory sits beside a module of the same name,
so that is the layout used, and the crate root's `#[path]` points at
`support/mod.rs`. Every nested module is now at most 258 lines.

The Developer Documentation row's remaining point was that the guide described
one writable mount and two traced queries. The guide now states the second
writable mount (`/var/trace` beside `/proc/self/comm`), the third query, the
verifier and Kani-home binds and why the home is needed, and the capability
probe's five path kinds; its attempt-detection paragraph presents the runtime
and static halves as the two halves of one contract, and the command-test
sentence now says the diagnostic must carry its category and status, echo
nothing the verifier printed, and stay free of control characters.

All nine commit gates were run on this candidate and each exited `0`:
`check-fmt`, `lint`, `typecheck`, `doc-coverage` (98.33% aggregate,
6,762/6,877, against an 80% threshold), `markdownlint` (218 files, zero issues),
`nixie`, `test` (5,217 nextest passed, 7 skipped, plus the doctests),
`test-workflow-contracts` (1,792 passed, 3 skipped, plus 28) and
`test-kani-check-isolation` (43 passed, 0 skipped). The first `lint` run of the
sequence exited `2` with three findings — two `clippy::shadow_reuse` and one
`clippy::unused_self` — which the module decomposition and the associated
lookup removed; the batch was then restarted from `check-fmt` and every gate on
the finished tree passed. Keep Status: IN PROGRESS pending review at the new
head and trusted-main warm-cache acceptance.

Revision 57 (2026-11-09): The pre-merge reply at `6731095126` left three rows
Open at `1a840c133`, and two of the three needed work beyond Revision 56.
Testing (Overall) accepted the trace and the control verifier but said the
control exercises the verifier and not a suppressed attempt added to the
recipe. That is a real gap: the shipped recipe is parsed statically for
suppressions and the trace follows `make`, but nothing showed the runtime half
firing on a command the recipe itself runs. The fixture now writes a makefile
whose `kani-check` lines discard a write to `/etc` and a directory creation
under `/usr`, then report the pinned version. Both paths are on read-only
mounts, so the run changes nothing; because each failure is discarded the make
run still succeeds, which is what leaves the trace as the only record. The case
requires `dirty-exit=0` and both findings — a write-intent open and
`` `mkdir` changes a path `` — so an attempt added to the recipe, rather than
to the program the recipe runs, is shown to be caught.

Unit Architecture's point was that the isolated query still used the fixture's
stub, which Revision 56 answered with the third traced query over the installed
verifier and the `kani-smoke` lane's `NETSUKE_KANI_ISOLATION_VERIFIER`. The
Developer Documentation row's two remaining corrections — the "bounded quote"
wording and the claim that every mounted path is read-only — were also made in
Revision 56; this revision adds the second control to that paragraph.

The new helper first took the completed run plus borrowed `stdout` and `stderr`
strings, and CodeScene reported String Heavy Function Arguments for it at 9.68.
It now takes the run alone and borrows both streams itself, and the file scores
10.00 again.

All nine commit gates exited `0` on `7f6a4693d`: `check-fmt`, `lint`,
`typecheck`, `doc-coverage` (98.33% aggregate, 6,762/6,877), `markdownlint`
(218 files, zero issues), `nixie`, `test` (5,217 nextest passed, 7 skipped,
plus the doctests), `test-workflow-contracts` (1,792 passed, 3 skipped, plus
28) and `test-kani-check-isolation` (43 passed, 0 skipped). The isolation case
was also run by hand with `NETSUKE_KANI_ISOLATION_VERIFIER` naming the local
`cargo-kani`, and passed. `7f6a4693d` is the last revision all nine gates
verified; the commits after it carry only prose in this file. Keep Status: IN
PROGRESS pending review at the new head and trusted-main warm-cache acceptance.

Revision 58 (2026-11-09): The review at head `42e4e490f` raised six inline
findings and re-emitted two pre-merge rows plus the Developer Documentation
warning. Three findings were the same defect reported at different anchors: a
datagram sent with `sendmsg` was not recognized as an attempt, and neither were
`linkat`, `mknodat`, `symlinkat`, `fchmodat`, `fchownat` or `rmdir`, so the
reader would have passed a run that created a link, a node, or a permission
change. The trace filter now names syscall *classes*, `%file` and `%network`,
rather than calls one by one. That is not only shorter: `strace` refuses to
start when a qualifier names a call the host architecture does not have, and
`mkdir` has no syscall of its own on aarch64, so the enumerated list was a
portability hazard that would have failed the isolation case outright on that
architecture. The reader's mutating-call list grew to the twenty-four names
those classes deliver, and the case covers each new one.

Unit Architecture's finding was a real defect in the new query. `cargo-kani` is
a launcher: with no `<KANI_HOME>/kani-<version>` it runs a first-time setup,
creates that directory, and downloads a release bundle from GitHub. A read-only
query that reaches that state installs. Proved by hand — an empty `KANI_HOME`
makes `cargo-kani --version` populate the home and fetch — so the recipe now
inspects the payload directory after reading the pin and before invoking
anything, refusing with `category=payload-missing` and naming
`make install-kani`. The isolation case gained a further traced run over a
payload-free home: it requires the refusal and requires that trace to hold no
attempt. The refusal is what fails if the guard is removed — the run uses the
stub verifier, so a removed guard would let it succeed rather than leave a
setup attempt behind — and the clean trace only adds that the refusal attempted
nothing itself.

The Developer Documentation row's premises were both live, and Revision 57's
claim that they had already been corrected was wrong: the Progress item still
said the recipe bounded and sanitized the output it quotes — it captures that
output and prints only the extracted version token — and Outcomes still called
`/proc/self/comm` the one writable path, ignoring the trace directory at
`/var/trace` and the bound `/dev/null`. Both are corrected here. Outcomes also
names Revisions 55 to 58 in its list of gated candidates, which previously
stopped at 54.

The isolation case's assertions moved into
`tests/prover_tools_kani_check_tests/assertions.rs`, since the case file was
over the 400-line cap and the helpers were the part that could move without
weakening the case. `test-kani-check-isolation` still exercises the whole
sandbox strictly.

Gating found three defects that the focused runs could not. The new Makefile
contract sliced `&str` by byte index, which `clippy::string-slice` refuses; it
now splits on whitespace and stops at the `--` token. The sandbox run built its
`SandboxRun` in an order that disagreed with the struct declaration. And two
`kani_check` command cases ran the recipe with no Kani home carrying the pin,
so the new guard refused them before the verifier they were testing; the
fixture now supplies a home holding an installed payload for its pin, which
also makes those cases independent of whatever the host happens to have
installed.

All nine commit gates exited `0` on `9037b92cb`: `check-fmt`, `lint`,
`typecheck`, `doc-coverage` (98.33% aggregate, 6,762/6,877), `markdownlint`
(218 files, zero issues), `nixie`, `test` (5,226 nextest passed, 7 skipped,
plus the doctests), `test-workflow-contracts` (1,792 passed, 3 skipped, plus
28) and `test-kani-check-isolation` (52 passed, 0 skipped, strict). Keep
Status: IN PROGRESS pending review at the new head and trusted-main warm-cache
acceptance.

Revision 59 (2026-11-09): The review at `715970069` came back CHANGES_REQUESTED
— review `5831859656` — with nine inline findings and three thread follow-ups.
All nine were live against that head.

Two were substantive. The first is the empty `KANI_HOME` divergence. The
launcher resolves an empty value to the current directory, while the recipe's
`${KANI_HOME:-$HOME/.kani}` default resolves it to `$HOME/.kani`, so the
payload guard could pass on a home the launcher never uses. Proved by hand in a
fresh directory: `KANI_HOME= cargo-kani --version` created `kani-0.67.0/`
beside the caller and downloaded a 143 MB bundle. The finding asked for the
paths to be aligned; that remedy is the wrong one. Aligning means resolving an
empty value to `.`, so a typo would make a read-only query inspect
`./kani-<version>` instead of the home the operator meant, and the query would
then run the launcher against a directory nobody chose. The recipe refuses the
setting with `category=kani-home-empty`, which makes the ambiguity a diagnosis
rather than a guess, and `kani_check_refuses_an_empty_kani_home` requires the
non-zero status, the category, and an untouched fake verifier.

The second is the trace's pin-read assertion. It required only that some line
name `KANI_VERSION`, and the `execve` line carries `KANI_VERSION_FILE=` in its
argv whether or not the file was ever opened, so the assertion could pass on a
recipe that read nothing. It now requires a line that names the pin *and* is an
open call.

`is_device` was a third real false negative. It matched with `contains`, so
`/dev/null.log` and `/dev/ttyUSB0` were exempted as if they were the character
devices, and a write to either would have gone unreported. The path is now
compared whole, quotes included, and two cases pin the boundary.

The rest are contract and prose repairs. `.github/workflows/ci.yml` gives both
apt commands `-o Acquire::Retries=3`, because a mirror that drops one request
otherwise fails the required lane on a network fault rather than on the
boundary. `tests/prover_tools_verus_e2e_tests/support.rs` builds both
`make verus` invocations from one `make_verus_command()`, so the flag-scrubbing
rule cannot drift between them.
`tests/whitaker_boundary_contract/whitaker_invocation.rs` collects every line
that assigns `WHITAKER_SUITE_SELECTION`, requires exactly one, reports the
lines it found when there are more, and treats `+=` as an assignment operator;
the finder also matches the name under an `export`, `override`, `private` or
target-specific prefix and skips a commented-out line, and the operator check
rejects `+=` for the same reason it rejects `?=`, since it appends to an
exported value. A follow-up asked whether a command-line override should also
be blocked; it should not, and the test's doc comment now records why: the
command line is one invocation's explicit instruction, the way the suite
substitutes `KANI=` and `PROVER_TOOLS=`, while an inherited value arrives
without anyone asking.

The documentation findings were the three LanguageTool diagnostics — a comma
before `because`, a comma before `so`, and a missing serial comma — plus
sentence splitting across the four paragraphs the finding named. Splitting them
surfaced a wrong count: the paragraph said "exactly two writable mounts" and
named the control probe at `/proc/self/comm` and the trace directory at
`/var/trace`, but `/dev/null` is bound read-write as well, so it now says three
and names all three. In `docs/whitaker-users-guide.md` the two quick-start
mentions of `whitaker --all` now say it is upstream's default and point at the
Netsuke deviation note, and "fails its own metadata check" lost the redundant
possessive.

The focused runs passed on this tree: 36 tests across
`prover_tools_kani_check_command_tests`, `whitaker_boundary_contract` and
`prover_tools_verus_e2e_tests`, 36 passed and none skipped, including the new
empty-home case.

The first full batch then stopped at `lint`, and the stop was worth having.
Clippy denies `string-slice`, and the new selection finder indexed the line at
offsets `match_indices` had already proved to be character boundaries. The
three sites now use `get(..)`, the convention the rest of the suite follows, so
each slice stays a checked lookup rather than a panic site; the scan reads the
same. The re-run took all nine gates. `check-fmt`, `lint`, `typecheck`,
`doc-coverage`, `markdownlint`, `nixie`, `test`, `test-workflow-contracts` and
`test-kani-check-isolation` each exited `0` on commit `53d1cc32e`.

Revision 60 (2026-11-09): Revision 59 answered the eight inline findings, and
the walkthrough comment then showed three pre-merge rows still open. Those rows
are the review's ninth disposition item, and two of them were live: the guide's
traced-run order, and the size of the command suite. The third — the trace
reader — Revision 59 had already fixed.

The order was simply wrong. The guide called the payload-free query "a third
run", but the sandbox runs the query, then the control verifier, then the
dirty-recipe control, and the payload-free run comes after that. The passage
now says "a further run" rather than being renumbered, because a count that
names no ordering cannot drift when a case is inserted between two others. The
isolation case's own doc comment had the same fault from the other direction:
it called the installed-verifier query the fourth traced run and the
payload-free one the fifth, when the payload-free run is traced first.

The size finding was live and getting worse: the command suite was 414 lines at
`715970069`, and the empty-home case took it to 452. The fixture — the fake
verifier, its behaviours, the recorder and the invocation builder — moved to
`tests/prover_tools_kani_check_command_tests/fixture.rs` behind a `#[path]`
module in the crate root, which is the layout the sibling boundary suite
already uses. The case list is 277 lines and the fixture 192.

The third row caught a claim of mine that was false. Five places said the
payload-free run's attempt-free trace is what fails if the guard is removed.
That run is traced against the stub, not the installed launcher, and a stub
answers `--version` without installing anything: remove the guard and the run
succeeds, leaving no setup attempt for the trace to hold. What catches a
removed guard is the refusal status and category, together with the
non-invocation assertion in `kani_check_refuses_a_missing_payload`. The trace's
narrower claim is that the refusal itself attempted nothing, and the text now
says exactly that. All nine commit gates exited `0` on `ae3fbd100`.

Revision 61 (2026-11-10): Record the seven inline findings and one CodeScene
issue raised at `1f38b9e91`, their repairs, and the nine-gate batch that
validated them. CodeScene's "String Heavy Function Arguments" report is a
file-level ratio, not a per-function verdict, which is why its text named no
function: `tests/prover_tools_kani_check_tests/assertions.rs` measured nine
string arguments of twelve, and `cs check` reproduced the hosted 9.68 before
the change. Every one of those values already lives on `SandboxRun`, so each
helper now takes the run and reads the field its assertion is about; the same
file measured 10.00 afterwards. The first batch after the repairs failed at
`lint-clippy`, because the new proptest helper took `anyhow::Error` by value
and only formatted it. The second failed at `lint-python`: the import block
needed Ruff's ordering, `_run_step` held twelve locals once it read the marker,
and one assertion message reached 89 columns. Writing the stubs and the
environment into their own helpers, and letting Ruff order the imports, cleared
all three. All nine commit gates exited `0` on `5d9a6b6cf`.

Revision 62 (2026-11-10): The review at `cfdd1563f`, taken on the work Revision
61 recorded, raised two inline findings and two outside the diff. `kani-check`
gained the `home-unresolvable` refusal: an unset `KANI_HOME` with an empty
`HOME` left the `${HOME:-}/.kani` fallback naming `/.kani`, so the guard would
clear a directory the launcher never reads. That is the same guard-and-launcher
mismatch `kani-home-empty` removes, one step later, and it matters more than a
cosmetic refusal: a payload found there says nothing about the installation the
query is about to run. `kani_check_refuses_an_unresolvable_home` blanks both
variables on the child process and asserts the category and an untouched
verifier, and the developers' guide lists the category beside the others. The
guide's `whitaker-ls` sentence now opens with `Run` rather than `Use`, which is
what LanguageTool's false positive on `Use to` was reaching for; no repository
gate runs LanguageTool, so the reword is for the reader rather than for a
check. The Outcomes summary ended at Revision 58 while the document had
recorded three more, so it now names Revisions 59 to 62 and counts fourteen
gated candidates rather than ten. The remaining outside-diff finding, that
Revision 61's date should be 9 November, does not hold: the commit is dated
`2026-11-10 00:09:04 +0100`, every revision date in this document is a local
one, and the review that raised it ran at 23:20 UTC.

Hosted CodeScene then failed the resulting head with a duplication advisory on
`tests/prover_tools_kani_check_command_tests.rs` at 9.39: the new refusal case
had been written as a near copy of the empty-home case beside it. The two are
now one `#[rstest]` case over a `HomeRefusal` fixture enum, which also gives
both mechanisms a single place to be described, and `cs check` reports 10.00
for the file and for its fixture. That is the disposition the two cases always
wanted: both assert that a home the guard and the launcher cannot share is
refused before the verifier runs, and only the setting that produces the
mismatch differs.

All nine commit gates exited `0` on the final tree, from check-fmt at 00:34Z to
test-kani-check-isolation at 01:11Z, after an earlier full pass at 23:50Z had
covered the same code before the duplication refactor. The first attempt failed
at check-fmt because mdtablefix reflowed both edited documents, and `make fmt`
supplied the layout it wanted.

Keep Status: IN PROGRESS pending review at the new head and trusted-main
warm-cache acceptance.
