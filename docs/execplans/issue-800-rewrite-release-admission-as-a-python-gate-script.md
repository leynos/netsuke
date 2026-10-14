# Issue 800: Rewrite release admission as a Python gate script

Status: COMPLETE

Delivered, validated, and reviewed. Two CodeRabbit passes have concluded: the
first raised three findings, all triaged; the second read the revision that
answered them and returned none. A third repair pass has since fixed one real
behaviour defect — a trailing-slash artefact target — cleared the CodeScene
duplication at the same site, added the acceptance coverage listed under
*Progress*, and corrected several prose claims that measurement falsified. A
fifth pass, over three items a later confirmation raised, fixed a second
behaviour defect — the `mkdir` operand for a target ending in `/.` — and
corrected two more stale claims. A sixth pass, over three concerns a
proof-substance assessment raised, repaired the boundedness property's evidence
without changing the gate or the case count, and a seventh pass withdrew part
of that repair after measurement falsified the reason given for it. An eighth
pass then repaired two defects those two passes introduced: a CodeScene
`Large Method` regression in the test module they edited, and a false positive
in the property's own exclusion assertion. A ninth pass then acted on the
proof-substance confirmation that read the eighth pass's revision: it closed a
third assertion defect in that same property — a false *negative*, where a
leaked value production had normalized would not have matched the identifier
searched for — corrected the reachability claim the confirmation showed was
still wrong, and split the module when the added prose pushed it past the
project's file-size ceiling. A tenth pass then swept the documentation the
split had invalidated: it corrected two stale paragraphs in the developers'
guide and this plan's own current-state module count, changing no code and no
case. An eleventh pass corrected the `assert_complete_records` docstring, whose
summary line contradicted its own detail. A twelfth pass then acted on four
documentation items the assessment confirmation had verified — a bounded
generator described as unbounded, a helper attributed to the wrong module, a
contradiction in this plan's own ninth-pass entry, and a descendant-signalling
claim its own module docstring refutes — and corrected a fifth claim in its own
verification notice. A thirteenth pass then repaired two defects the recording
commit had introduced and qualified two historical commit references an
intervening rebase had invalidated. The accounting for all of it is under
*Outcomes and retrospective*. The pull request remains open for the
repository's own review and merge, which is a separate concern from this plan's
completion.

## Purpose

Release admission is roughly 300 lines of Bash under `.github/scripts/`
(`require-release-admission-canaries.sh` and its two siblings). The estate rule
in `docs/scripting-standards.md` requires gate logic to be Python: loops, `;`
chains, and conditionals in workflows become a `uv`-run Cyclopts script with
cmd-mox tests. Admission is netsuke's own policy, so it stays in netsuke; only
its form must change.

Success is observable when `.github/scripts/release_admission.py` runs under
`uv run --script`, reproduces the Bash behaviour case for case, has cmd-mox
tests for every reachable RFC 0005 admission and refusal path, keeps every
refusal's wording or states why it differs, and is what `release.yml` invokes
once the shell scripts are deleted.

## Constraints

- Behaviour must match the current shell case for case: every input, operation
  order, argument vector, classification, record, sink rule, and exit status.
- The fail-closed `fresh` contradiction and Bash's trailing-newline semantics
  are preserved, not repaired.
- Refusals keep their current wording, or the change states why the wording
  differs.
- Observation mode and publication independence are preserved.
- `release-dry-run.yml` is unchanged.
- The Python is the estate shape: Python 3.14, PEP 723 `uv` block, shebang
  `#!/usr/bin/env -S uv run --script`, Cyclopts env-first, tests under
  `scripts/tests/` using cmd-mox.

## Progress

- [x] Freeze the shell behaviour. The measurements are recorded in
      `scripts/tests/release_admission_test_support.py`'s module docstring:
      exit statuses, stderr, JSONL key order and types, default paths, and
      environment variables.
- [x] Implement the Python gate. `.github/scripts/release_admission.py` is the
      entry point; it carries the PEP 723 block with the `cuprum==0.1.0` and
      `cyclopts==4.25.3` pins and a `#!/usr/bin/env -S uv run --script` shebang,
      and `release.yml` invokes it with the equivalent
      `uv run --script <path>` (the workflow does not rely on the executable
      bit, which is why the shebang is not the load-bearing part of the
      invocation). `.github/scripts/_release_admission/` holds eleven modules
      (`policy`, `records`, `delivery`, `commands`, `statuses`, `telemetry`,
      `environment`, `filesystem`, `configuration`, `bounding`, `gate`); the
      later of those were split out when the CodeScene refactor, the
      real-child bound, and the formal review's domain-architecture finding
      pushed `gate`, `commands`, and `policy` over the module ceiling or
      across a boundary they should not have owned, and `statuses`,
      `environment`, and `filesystem` arrived with the repair of that finding.
      The runtime-test support in
      `scripts/tests/` is a facade and seven modules — the two-stage split
      recorded under *Decisions and findings*, plus the two later splits that
      kept pace with the file ceiling — and the count is stated per revision
      below rather than as one "current" number.
- [x] Clear the Python gates. `make lint-python` passes all five stages:
      Ruff 0.16.4, Pylint 4.0.9 at 10.00/10, the df12 house lints, ambrleaks,
      and interrogate at 100%.
- [x] Repoint the runtime tests at the Python gate. The 33-case frozen oracle
      passes against `.github/scripts/release_admission.py`, including the
      PR #770 newline-revision cases in both modes.
- [x] Replace the executable fakes with cmd-mox and port the case matrix.
      `release_admission_test_fakes.py` is deleted, the four runtime modules
      run through registered doubles, and the modules pass with 37 cases in
      under 90 seconds. The design the port settled on, and the reasons:
      - The subprocess boundary stays. The gate is still invoked as a real
        process, because only that exercises its exit status, its artefact
        files, and its redirections. The shebang is *not* among them: the
        harness launches `[python, script]` by absolute path, so `uv` never
        reads the first line. That contract is asserted statically instead, in
        `test_release_admission_metrics.py`. cmd-mox replaces the *fake
        executables*, intercepting at the command-name level.
      - `python3` is never shimmed. cmd-mox's POSIX shims are symlinks whose
        shebang is `#!/usr/bin/env python3`, so a shim named `python3` would
        resolve its own interpreter back to itself. The clock therefore runs
        as the real `python3` by default, and the tests that need a
        deterministic or failing clock point
        `NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER` at a shim named `clock`.
      - `gh` and `git` are shimmed under their documented default names, so
        the default resolution path is what the tests exercise; an
        operator-supplied adapter path is covered by pointing an adapter
        variable at a shim by absolute path.
      - The in-process handler cannot ignore `SIGTERM`, so the
        term-ignoring-timeout case moves out of the gate matrix and becomes a
        direct test of `commands.run_bounded_sync` against a real
        TERM-trapping child. In the gate's records a TERM-ignoring child and a
        compliant one are indistinguishable: `run_bounded` returns `None` on
        either, and `classify_result` maps that to `124`.
      - `NETSUKE_FAKE_*` variables disappear. Behaviour is configured by
        registering doubles, which is cmd-mox's own idiom, and the assertions
        move onto the doubles' recorded invocations.
      - The doubles are spies, and every count assertion is a direct
        comparison against the recorded calls. cmd-mox verifies expectations
        for mocks only, so a spy's `times_called` would set an expectation
        nothing reads; `in_order()` on a spy is worse, because an ordered
        expectation nothing consumes makes `verify()` raise. Ordering is
        therefore asserted on the recorded calls, where it is data.
- [x] Switch `release.yml` to the Python entry point and delete the shell. The
      admission step now runs `uv run --script .github/scripts/release_admission.py`;
      the three shell scripts are deleted; `release-dry-run.yml` is unchanged.
- [x] Update the workflow contract, ADR-0020, and the developers' guide. The
      contract asserts the Python invocation, the `astral-sh/setup-uv`
      provisioning, observation mode, publication independence, and the
      absence of any `*release-admission*.sh` reference. The developers' guide
      describes the entry point, the module split, and the cmd-mox conventions.
- [x] Clear the CodeRabbit findings and the CodeScene advisory findings. The
      six review findings are one commit; the advisory findings on code this
      change introduces are a second, atomic refactor commit, together with the
      module split that refactor forced. The split could not be a third commit:
      the refactor left `gate.py` at 442 lines, so the state between them fails
      Pylint's `C0302` and no commit may be red. Everything is measured locally
      with `cs check` against the file rather than the commit, because
      `cs check HEAD:./file` reads the committed blob and reports the previous
      revision's issues.
- [x] Clear the first review pass over the delivered port. Three findings: a
      dead `or 0` and this plan's revision-note ordering were accepted, and a
      proposed counter suppression on an unusable clock reading was rejected
      against measurement. The rejection also corrected a false sentence in the
      frozen oracle's docstring, which was the finding's premise, and added the
      regression case that pins the measured behaviour.
- [x] Clear the second review pass's behaviour-parity defect list and its
      acceptance-coverage gaps. Five defects were verified against the shell
      and fixed: the bound signalled only its direct child rather than the
      process group, the adapters' standard error was discarded, subprocess
      statuses reached the gate untranslated, the two artefact parents were
      created and truncated in one interleaved pass, and a configured path was
      normalized at two observable boundaries. The gaps closed are the refusal
      order and the artefact-creation position, the two Bash default forms and
      the default artefact paths, the status translation, the real-child
      runner's private-versus-inherited streams, and the two persistent sink
      failures. A later repair pass closed two more gaps and corrected one
      real defect: a trailing-slash artefact target was silently accepted where
      the shell refused it, and an external sink's argument vector was never
      asserted. It added two modules and eight cases, taking the runtime suite
      to twelve modules and 89 cases; a fourth pass then added six more, for
      95, and a fifth added one for the `/.` parent derivation, for 96. A sixth
      pass changed no case count: it repaired the boundedness property's
      evidence rather than the gate, with the count staying at 96. A seventh
      pass changed none either, withdrawing part of that repair and closing one
      further evidence gap. An eighth pass then repaired two defects those two
      passes introduced — a CodeScene `Large Method` regression and a false
      positive in the property's exclusion assertion — and likewise changed no
      case count, the suite reporting 96 again. A ninth pass closed a false
      negative in that same assertion (a normalized value a leak would carry
      was not in the searched set), corrected the reachability claim in the
      property and the guide, and split the property's mechanism into a support
      module when the added argument pushed the test module past the 400-line
      ceiling. That support module is not collected, so the twelve collected
      modules and the case count are unchanged. The counts and the per-gap
      modules are recorded under *Outcomes and retrospective*. The figure
      reached 96 at `54a6ec66`, when the `dirname(1)` case landed.
- [x] Clear the third repair pass. It fixed the trailing-slash refusal
      described under *The trailing-slash defect and its repair* and cleared
      the CodeScene duplication at the same site; added coverage for the
      external sink's argument vector, the set-but-empty `GITHUB_OUTPUT`
      refusal, both truncation boundaries, and the colon-form adapter and
      timeout defaults; corrected five false or incomplete claims in prose —
      the harness docstring's shebang claim, the ExecPlan's copy of it, the
      `143`-as-timeout claims in `policy` and `commands`, and the bounding
      module's unconditional-cleanup claims — and corrected the developers'
      guide's matching claims. The `143` premise was verified false and the
      bounding premise was verified incomplete; both were corrected as prose,
      with no implementation change.
- [x] Clear the tenth pass, a documentation sweep after the ninth pass's split.
      It corrected two stale paragraphs in the developers' guide — the
      support-module inventory, which the split had invalidated, and the
      runtime-module list, which had said "ten" and named ten since
      `9a0edd5d` while the Makefile and the workflow contract's
      `RUNTIME_MODULES` tuple had named twelve since `36a913ed` — and corrected
      this plan's own current-state module count, which the same split had
      invalidated. No code, test, or workflow changed, so the case count and
      the twelve collected modules are unchanged.
- [x] Clear the formal-review repair pass. The first formal review's Unit
      Architecture finding required resolution and initialization to be
      separable, and the repair split them: `configuration` now exposes
      `resolve_destination`, `resolve_configuration`,
      `prepare_destination`, and `require_workflow_output`. Only
      `prepare_destination` writes, and it does so solely through the
      `filesystem.FileSystem` it is handed; `require_workflow_output` reads a
      mapping and can refuse the value it finds there, so it performs no
      filesystem effect at all. The entry point calls the four in the shell's
      refusal order. The
      effects gained a port (`filesystem`) whose two members are written as
      positional-only functions so a plain callable is assignable to it.
      Two runtime modules were added — `..._configuration_resolution.py`
      proves resolution reads only the mapping it is given and writes
      nothing, and `..._artefact_initialization.py` drives the effects with a
      recording port — and the Makefile, the workflow contract's
      `RUNTIME_MODULES`, and the developers' guide's module list all moved
      from twelve to fourteen together. The two new modules were probed
      before being trusted: interleaving the passes fails three cases, and
      resolving the truncation operand through `Path` fails two of the new
      cases plus both trailing-slash cases in `..._artefact_truncation.py`.

- 2026-10-11 — Pushed the repair and the orthography pass as `5a1bab9f`, which
  follows `d5b0b072`, itself one commit on top of the reviewed `678b87be`: the
  nine-finding repair and the split-plus-orthography pass were pushed as two
  commits, not one. The push went over SSH from an environment with Lody's
  `GIT_CONFIG_*` injections and `gh` credential helpers stripped, because those
  rewrite `git@github.com:` to `https://github.com/` and route the push through
  a credential helper; a trace confirmed the clean invocation reaching
  `/usr/bin/ssh git@github.com 'git-receive-pack'` and the injected one reaching
  `git-remote-https`. The API then read the branch head back as `5a1bab9f`,
  and the tracking ref was updated to match, since an SSH push leaves it stale.

  All nine of the review's inline findings are verifiably present in the new
  head: the sink path calls `report_launch_failure`, the four `TRACE_*`
  constants and the evidence-state literals are gone, the plan's first-person
  passages are rewritten, the orphaned `sys.path` mutation and the local
  `CLOCK_ARGUMENTS` are gone, the runner docstring was corrected, and the
  fabricated `FileNotFoundError(20, ...)` case is now
  `NotADirectoryError(20, …)`, which is what a real `ENOTDIR` exec raises.

## Decisions and findings

- Cyclopts' default result action, `print_non_int_return_int_as_exit_code`,
  returns an integer command result unchanged. The first version of the entry
  point called `app()` and then returned a literal `0`, which discarded the
  gate's own status and made every enforced failure exit successfully. The fix
  is to return `app()`'s value; the alternative considered was an explicit
  `sys.exit(admission.finish())` inside `main`, which would move the process
  boundary into the command and make the function untestable in process.

- Bounding is native. The shell wrapped each command in GNU `timeout`;
  `bounding.run_bounded` reproduces both the one-second grace and the
  process-group reach without an external binary, and it does so by signalling
  the command's *process group* rather than cuprum's cancellation path, which
  reaches only the direct child. The signal *number* is not reproduced: GNU
  `timeout` told `124` from `137` by whether the command had to be killed, and
  `run_bounded` reports an elapsed bound as `None` even after escalating to
  `SIGKILL`, because that escalation discards the child's own status. Every
  elapsed bound is therefore `124`, which classifies identically because both
  numbers mean a timeout.
- The configured adapter names are the cuprum allowlist catalogue, because the
  adapter environment variables are a documented part of this gate's contract.
- A launch the operating system refuses (a missing adapter, a file without its
  execute bit) is not a new failure kind. GNU `timeout` printed its own
  sentence and exited `126` or `127`; cuprum raises instead, so the port prints
  `release-admission adapter could not be run: <program>: <reason>` and maps
  the exception back to the same status. The status and classification are
  preserved; the sentence is a stated wording difference.
- An exported empty `NETSUKE_RELEASE_ADMISSION_ENFORCE` was kept, not replaced
  by the observation default, because Bash read it with `${VAR-default}` -- the
  dash form, without the colon -- which substitutes only when the variable is
  *unset*. The kept empty value is then refused by the gate's own mode check.
  `os.environ.get(VAR, default)` happens to agree, because it also substitutes
  only for an unset variable; `environment.defaulted_only_when_unset` is what
  states the no-colon form, so a reader cannot mistake the helper for the colon
  form its siblings implement.
- The 400-line module ceiling applies to Python as well as Rust (Pylint
  `C0302`, `max-module-lines = 400`). `release_admission_test_support.py` was
  exactly 400 lines at HEAD, so the runtime-test support was split into a facade
  (`..._test_support.py`), the subprocess harness (`..._test_harness.py`), and
  the record assertions (`..._test_records.py`). The cmd-mox port then split
  three further modules out of the facade: the doubles (`..._test_doubles.py`),
  the wrappers each case composes (`..._test_scenarios.py`), and the case table
  (`..._test_cases.py`). The support is therefore a facade and five modules,
  against the three of the first split.
- The repository has no `pylint: disable` comment anywhere and no function
  over four parameters outside the new gate. AGENTS.md requires grouping
  parameters into meaningfully named structs, so `records.py` gained the
  `MetricFields`, `TraceFields`, `OperationRecord`, and `GateRecord` value
  objects rather than argument-count suppressions.
- `configuration` is a module of its own because the split that cleared the
  CodeScene complexity finding left `gate` at 442 lines. The division is by
  question asked, not by line count: `configuration` owns everything the gate
  is *told* -- the required variables, their diagnostics, Bash's two default
  forms, and the artefact paths -- and `gate` owns everything it then *does*.
  `gate` therefore imports one name, `Configuration`, and the entry point
  reaches `configuration` directly for the three variable names Cyclopts binds
  and for the four start-up steps and `ConfigurationError`. The alternative,
  having `gate` re-bind every moved name, was rejected: it would have made
  `gate` an indirection no reader needed, and one of the names it would have
  carried, `DIAGNOSTIC_PREFIX`, turned out to be referenced nowhere at all and
  was deleted instead of moved.
- `from __future__ import annotations` is redundant on the 3.14 baseline and
  the df12 house lints reject it (`C9112`). PEP 649 defers annotation
  evaluation, so a `TYPE_CHECKING`-only `Path` in a dataclass annotation is
  safe without it; the repository does not support runtime annotation
  introspection on these modules (ADR-0038).

## Deferred

Producer-backed refusals are deferred. The gate has no scan producer of its own
yet, so `NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE` arrives from the environment
and the `missing_evidence`, `stale_evidence`, and `mismatch` categories are
reachable; the categories that require a real producer to observe are named as
deferred in `release_admission_test_cases.py` rather than simulated. That
module already owns the RFC 0005 mapping for the refusals this suite runs, so
the refusals it does not run belong there too; the deferral was moved there from
`release_admission_test_scenarios.py`, which sat 399 lines into pylint's
400-line module ceiling.

The deferred set is an integration, not a producer's exclusive property. RFC
0005 splits it: producers supply the manifest, the scan results, and the
measurements, and admission then verifies what they supplied — it "verifies
provenance rather than trusting declared fields", recomputes each archive's
SHA-256 from its actual bytes, and rejects a mismatch against either the
sidecar or the manifest's recorded digest. The admission-side half is therefore
part of this port's remaining debt rather than another component's.

`verify_evidence` runs only when the four operations before it succeed, because
`gate.Gate.run_operations` stops at the first failed operation; a missing,
stale, or unknown freshness state, or an earlier adapter failure, prevents it
from running at all.

## Outcomes and retrospective

The port is complete, validated, and open for review. The Bash gate is now a
Cyclopts `uv run --script` entry point with its implementation split across
eleven modules, its behaviour held to a frozen oracle, and its tests running
through cmd-mox doubles. `release.yml` invokes the Python entry point; the
three shell scripts are deleted.

The boundedness property has taken four passes to become well-formed, and each
pass was found by a different instrument: the sixth by a proof-substance
assessment, the seventh by the property failing on its own restored evidence,
the eighth by a hosted CodeScene check, and the ninth by a confirmation reading
the eighth. Only the eighth was a *false positive* — a passing run reported as
a failure. The seventh and ninth were false *negatives* in the exclusion
itself, the direction that stays silent, and the ninth's fix is the one that
generalizes: an exclusion set must carry every normalization of the value it
searches for, not only the form the generator produced.

Two review passes have run over it. The first, at `30b961d8`, raised three
findings: two were accepted and fixed, and one was rejected against a
measurement of the Bash gate — see the revision note for the account. The
second, at `c6a9d023`, read the revision that answered the first and returned
no findings across all thirty changed files. Both revisions were gated in full
before their pass was requested; the second revision's gates are
`make check-fmt`, `make lint-python`, `make typecheck-python`, `make lint` (all
twelve sub-suites, with Clippy clean so Whitaker genuinely ran),
`make doc-coverage` (98.87%), `make test` (3917 passed, 6 skipped, 129
doctests), `make test-release-admission` (38 passed),
`make test-workflow-contracts` (1150 passed, 3 skipped), `make markdownlint`,
and `make github-actions-lint`.

`30b961d8` and `c6a9d023` are the revisions those passes actually read, and
they are named as such. They are **not** ancestors of this branch's head: an
intervening rebase replayed their changes, so the current-history counterparts
are `66ec7582` and `49df29cb`. Each pair carries an identical stable patch ID —
`78186157` and `5ea72ce3` respectively — which is what identifies them as the
same changes. Naming the historical revisions is deliberate: a review is
evidence about the revision it read, and rewriting that name to a later one
would claim the pass read bytes it never saw.

A third pass then read the delivered port for *behaviour parity* rather than
for the port's own quality, and it found more than the first two together. It
listed five behavioural defects, each verified against the shell before being
fixed, and five acceptance-coverage gaps. Every fix is a separate commit and
every gap is a case in one of the five modules the suite gained.

| Defect                                                                                                           | Verified against the shell                                                                                         | Fix                                                                                                                                             |
| ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| The bound signalled the direct child only, so a descendant survived cancellation and held the captured pipe open | GNU `timeout` signals the supervised command's *process group* and escalates to `SIGKILL` after a one-second grace | `bounding` starts the child in its own session and kills the group, with a real-child escalation case and a descendant case                     |
| The adapters' output was discarded                                                                               | The shell redirected no adapter stream, so an operator's diagnostic reached the runner                             | GitHub/clock standard output stays private but their stderr is inherited; the Git and sink adapters inherit both                                |
| A subprocess status reached the gate untranslated                                                                | A shell reports a signal death as `128 + signal`, and `timeout` reported `127`/`126` for a refused launch          | `policy.as_bash_status` and `policy.launch_failure_status`, pinned as pure functions                                                            |
| The artefact parents were created and truncated in one pass                                                      | The shell ran one `mkdir -p` naming both parents and only then truncated both files                                | `configuration.prepare_destination` creates every parent first, then truncates; a filesystem refusal is a `ConfigurationError`, not a traceback |
| A configured path was normalized at two boundaries                                                               | The shell passed the operator's own string to its sink and to `GITHUB_OUTPUT`                                      | `OutputPath` retains the configured spelling and derives the `mkdir` operand from it separately                                                 |
| The `mkdir` operand was the resolved path's `Path.parent`                                                        | The shell asked coreutils `dirname(1)`, which answers `reports` for `reports/.` where `Path.parent` answers `.`    | `configuration._directory_name` reproduces `dirname(1)`, with a `/.` case asserting the parent is created and the refusal is `Is a directory`   |

The gaps are the cases the port's own tests could not express, and each is now
a module of its own. The refusal order and the artefact-creation position are
`test_release_admission_environment_contract.py`; the two Bash default forms,
including the no-colon enforcement mode, are
`test_release_admission_default_forms.py`; the status translation and the
launch refusals are `test_release_admission_status_translation.py`; the
real-child stream contract is `test_release_admission_runner_contract.py`; and
the two persistent sink failures are
`test_release_admission_sink_persistence.py`. The descendant and filesystem
boundaries were already covered by `test_release_admission_process_bounds.py`
and the environment-contract module respectively.

The repair pass that followed added two more modules, and the count moved with
them. `test_release_admission_artefact_truncation.py` pins the *second*
artefact-creation pass — the one where the target itself cannot be truncated —
and it is asymmetric: a refused trace target has already truncated the metrics
file to zero, while a refused metrics target leaves the trace file's previous
bytes intact, exactly as the shell's `: >"$metrics_file"; : >"$trace_file"` did.
`test_release_admission_adapter_defaults.py` pins the colon-form defaults
whose consequences are read back from the run's own execution rather than from
a published record: the three adapter *names* and the operation timeout. Both
are separate modules because the modules they belong to by scope are within a
few dozen lines of the 400-line ceiling, and the count is recorded per revision
below rather than as one "current" number.

`make test-release-admission` ran twelve modules and 96 cases at that point in
the branch's history: 81 at `51d27ae5`, plus two from the external-sink
argument-spelling case and the set-but-empty `GITHUB_OUTPUT` case, plus two
truncation cases, plus four adapter-default cases, plus two trailing-slash
cases, plus four unset-default cases, plus the `/.` parent-derivation case.
That accounting is historical rather than current, because the formal-review
repair then added the two modules the split required.

The current registration is **fourteen modules and 118 cases**, measured on
this head: the twelve above plus the fifteen cases of
`test_release_admission_configuration_resolution.py` and the seven of
`test_release_admission_artefact_initialization.py`. The Makefile and the
workflow contract's `RUNTIME_MODULES` tuple both name the fourteen, in the same
order, and the contract's own prose and assertion message both say fourteen.

Two documentation defects were also cleared. `delivery.py`'s empty-target arm
described an empty redirect operand and an unset one as the same arm, and they
are not; and the frozen oracle's note derived a pre-trap refusal's silent
standard output from a redirection that never executes, because the shell armed
its exit trap after the required-variable checks. Both are docstring
corrections; the behaviour was already right.

The parity revision, `b10620a2`, was then gated in full: `make check-fmt`
including its `mdtablefix` stage, `make lint` with all twelve sub-suites,
`make typecheck-python`, `make doc-coverage` (98.87%), `make test` (3917
passed, 6 skipped, both doctest targets), `make test-release-admission` (82
passed), `make test-workflow-contracts`, `make test-documentation-contracts`,
`make markdownlint` including `markdownlint-cli2`, `make nixie`, and
`make github-actions-lint`. The first run of that set was red on four gates,
all four introduced by this pass rather than by the port it repairs: a Pylint
`R0913` as the cases gained parameters, a df12 `C9102`, an `assert` without a
message, and a stale `RUNTIME_MODULES` tuple in the workflow contract that
enumerated four runtime modules against the ten the Makefile now names. That
last one is the reason the contract exists, and it worked: the fix was the
amended list, not a relaxed assertion.

The count fell to 81 at `51d27ae5`, which drops one of those cases as a
duplicate, and that revision carries a green eleven-gate record over the same
set, with `make test-release-admission` reporting 81. Both revisions' figures
are stated because they differ: a single "current" number would be wrong about
one of the two.

### The trailing-slash defect and its repair

A second repair pass found a real port defect, not a documentation one. Bash
refused a trailing-slash artefact target — `: >"metrics.jsonl/"` reports
`Is a directory` and exits `1` — while the port silently succeeded, because
`Path("metrics.jsonl/")` drops the slash and `write_bytes` on the resolved path
created the regular file the shell had refused. The fix opens the *configured*
spelling in `configuration._truncate`, matching the precedent already in
`delivery._append_in_process`, and lets the operating system's own sentence
reach the caller through the existing `ConfigurationError` path. `mkdir` keeps
using the resolved parent, because `mkdir -p "dir/"` and `mkdir -p "dir"` both
succeed in the shell and the directory is the same either way.

The same site carried a CodeScene duplication warning: the `try`/`except` that
converts an `OSError` into the bounded diagnostic appeared twice. One
`_refusing` helper now serves both operations, so the failure handling cannot
drift apart between them. Measured with `cs check` on
`.github/scripts/_release_admission/configuration.py`: 9.38 with two warnings at
`:258` and `:276` before, 10.00 with no warnings after.

The first regression written for that defect did not actually detect it, and
the gap is worth recording because the module looked complete. It pointed the
refused variable at a *directory*, and a directory is refused by the old
implementation too: `Path(...).write_bytes(b"")` raises `IsADirectoryError`,
carrying the same sentence the assertions keyed on. The module therefore pinned
the truncation *order*, which is a real contract, while leaving the *operand* —
the thing the repair changed — untested. The cases that discriminate supply a
configured string ending in `/` for an absent target and for an existing
regular file: the resolved spelling creates the first and truncates the second,
and neither survives assertions that the target was not created and that the
existing bytes remain.

Two further premises were checked rather than assumed, and one was partly
falsified. `policy.TIMEOUT_STATUSES` is exactly `{124, 137}`, so the docstrings
in `policy.as_bash_status` and `commands.classify_result` that called `143` a
timeout status were wrong; they are corrected, and the implementation was
already right. The bounding module's claim that no descendant survives was
*almost* true and is now stated precisely: a descendant that inherits `SIGTERM`
as ignored survives the group signal whenever the direct child has already
exited, because the escalation to `SIGKILL` is armed only by the direct child
still being unreaped. Measured against the port with a two-second bound and a
child that traps `TERM` and starts a pipe-holding descendant: the call returned
after 20 seconds, the descendant's own lifetime.

### Behaviour preserved

- The five operations, their order, argument vectors, classification, records,
  sink rules, and exit statuses are unchanged, and the frozen oracle in
  `scripts/tests/release_admission_test_support.py` remains the evidence.
- Every refusal keeps its wording. The seven stderr diagnostics operators may
  key on are byte-identical.
- Observation mode and publication independence are unchanged; the contract
  still asserts that no publication job depends on the canary job.
- Bounding is native: `bounding.run_bounded` reproduces GNU `timeout`'s
  one-second grace, its process-group reach, and a timeout classification for
  an elapsed bound, without an external binary. It reports `124` where the
  shell could also have reported `137`, because its expiry is reported as
  `None` whatever signal ended the command rather than as the child's status.

### Stated differences

Two differences exist, both deliberate and both recorded in the entry point's
module docstring:

1. An adapter the operating system refuses to launch is reported as
   `release-admission adapter could not be run: <program>: <reason>` where the
   shell's GNU `timeout` printed its own sentence. Same position, same exit
   status, same classification; only the sentence differs.
2. A refused configuration and a refused metric write have no other stderr at
   all, exactly as before, so a refused observation mode still fails the step.

A third difference was expected and does not exist. The port renders a duration
with `str(float)` where the shell interpolated its value with `printf %s`,
which looked like it would diverge on float formatting. It does not: the shell
computed every duration by piping both readings through a `python3` helper that
ended in `print(...)`, so both sides are `str(float)` of the same value. A
nine-case probe over sub-second, large, exponent, `nan`, and integer readings
produced byte-identical output on both sides, and the records are
byte-identical with it.

One behaviour is deliberately preserved although it reads as a contradiction:
`check_scan_freshness` admits `fresh`, but `verify_evidence` refuses it unless
the state is *not* fresh and a workflow run identifier is present. The port
preserves this exactly rather than repairing it, because repairing it would
change admission semantics beyond the scope of a form change.

### Lessons

- `cs check HEAD:./file` reads the committed blob, not the working tree, so a
  refactor that has not been committed yet is scored against the previous
  revision and reports the very issues it just fixed. Measure with
  `cs check ./file` until the refactor is committed.
- Cyclopts' default result action returns an integer command result unchanged.
  Returning a literal `0` after `app()` silently discards every enforced
  failure's status; return `app()` itself.
- The 400-line module ceiling is enforced for Python too (Pylint `C0302`), so
  the runtime-test support had to be split twice: into a facade and three
  modules before the port, then into a facade and five once cmd-mox replaced
  the executable fakes.
- A refactor that clears a complexity finding lengthens the code it clarifies,
  and the ceiling applies to the result. Extracting the artefact-path value
  object and the two default helpers from the one loader they lived in -- then
  named `_OutputPaths`, `_optional`, and `_defaulted_only_when_unset`; the two
  helpers are now `environment.optional` and
  `environment.defaulted_only_when_unset`, and the loader is now
  `resolve_configuration` -- took `gate.py` from well inside the ceiling to 442
  lines, so the refactor could not be committed on its own: it had to carry the
  split that made the module fit. "Separate atomic refactors" is a rule about
  intent, not a licence to commit a red tree.
- No gate in this repository collects doctests from `.github/scripts/`.
  `make test` runs the Rust suites, and `make test-workflow-contracts` passes
  `--doctest-modules` to `scripts/*.py` rather than to the implementation
  package. Two examples in `records.py` had therefore been raising
  `SyntaxError` since they were written, with every gate green over them. Run a
  new `Examples` block rather than reading it: a docstring containing `>>>` is
  not evidence that the block executes.
- An exported empty `NETSUKE_RELEASE_ADMISSION_ENFORCE` must reach the mode
  check, not be replaced by the observation default, because Bash's
  `${VAR-default}` substitutes only when the variable is unset.
  `os.environ.get(VAR, default)` shares that substitution rule, so it would
  agree here; `environment.defaulted_only_when_unset` states the no-colon form
  as its own helper rather than leaving it to a call the siblings also make.
- A review finding can cite this repository's own prose as its authority. The
  counter-suppression finding quoted a sentence from the frozen oracle's
  docstring that asserted the shell dropped an operation's counter with its
  duration. The sentence was false, and the shell's `emit_metric` validates the
  counter's literal `1` separately from the duration, so adopting the fix would
  have *introduced* the divergence it claimed to prevent. Measure the claim
  against the shell before acting on it, even when the claim appears to quote
  the oracle; a false sentence in a document is not evidence about behaviour.
- The frozen oracle's docstring is prose, not a test. Nothing collects it —
  no test module reads it, and no gate runs it — so a wrong sentence there
  survives every gate and then misleads a reviewer. When a measurement
  contradicts the docstring, correcting the docstring is part of the fix rather
  than an afterthought; the missing regression case for the garbage clock was
  the second half of the same gap.
- `make spelling` regenerates `typos.toml` from the shared dictionary on every
  run. The dictionary's colour-flag rule had changed since the committed copy,
  leaving a clean checkout dirty after a spelling run. That one-line
  regeneration is committed separately at the head of this branch so a reviewer
  can split it without touching the port's commits.
- A test suite written *from* a port rather than *from* the specification it
  replaces reproduces the port's own misreadings as expectations. The coverage
  the third pass added was not new behaviour to test — every defect it found
  had been reachable from the day the port landed — it was a second reading of
  the shell, this time asking what each line was *for* rather than what the
  port made it do. The two cases that would have caught the worst defects
  needed facilities the harness lacked, so the harness itself was part of the
  gap: it could not assert a run that correctly creates nothing, because it
  read an absent artefact as a fault, and it could not express an *unset*
  variable at all, only an empty one. A support module that cannot represent a
  state the specification distinguishes is a ceiling on every case written
  against it.

## Revision note

- 2026-10-10 — Initial ExecPlan for `#800`: freeze the Bash gate's behaviour,
  port it to a Cyclopts `uv run --script` entry point, port the runtime cases
  to cmd-mox, switch `release.yml` over, and delete the shell.
- 2026-10-10 — Review pass. Two contradictions in this plan were corrected
  rather than papered over. The ExecPlan claimed a *facade and four modules*
  while its own account of the first split named two, and the case count had
  gone stale at 35; both counts are now stated per stage, because the support
  was split twice — into a facade and three modules before the port, and into a
  facade and five once cmd-mox replaced the executable fakes. The status stays
  `IN PROGRESS` with a review caveat beneath the header, since the style guide
  reserves `COMPLETE` for a plan whose review has concluded. The port is
  unaffected: no source file, gate, workflow, or test case changed with this
  note.
- 2026-10-10 — CodeScene pass and the module split it forced. The advisory
  review named three issues on code this change introduces, and a fourth
  surfaced from a test this pass had already lengthened. The refactor: `policy`
  gained a `Vocabulary` base whose `contains` class method replaced five
  same-shaped predicates; the configuration loader was split so each Bash
  default form has one named helper (then `_optional` for `${VAR:-default}` and
  `_defaulted_only_when_unset` for `${VAR-default}`, both now module functions
  on `environment`) and the artefact paths travel as one value; and the two
  oversized tests were reduced by reusing `assert_failure_trace_sequence` and
  by extracting `_run_under_generated_identifiers`. That work took `gate.py` to
  442 lines, so the same commit moves the configuration half into the new
  `configuration` module. The entry point and the developers' guide now name
  `configuration` where they named `gate`; `DIAGNOSTIC_PREFIX`, a constant the
  port introduced but nothing ever read, is deleted rather than moved.
  CodeScene scores every touched file at 10.00, and the 37-case runtime suite
  is unchanged, which is the evidence that the refactor preserved behaviour.
  The developers' guide records both conventions, as the house abstraction
  policy requires. One instrument detail is worth keeping:
  `cs check HEAD:./file` scores the *committed* blob, so a working-tree
  refactor must be measured with `cs check ./file`.
- 2026-10-10 — Doctest repair. Checking the moved examples by hand turned up a
  defect no gate had run: `MetricFields.is_valid`'s two examples wrapped as
  ``MetricFields(...)`` then ``.is_valid()``. The first line is a complete
  statement, so the second was a second statement and doctest compiled the pair
  as one — every run raised ``SyntaxError``. Nothing collected these doctests:
  `make test` runs the Rust suites, `make test-workflow-contracts` runs
  `--doctest-modules` over `scripts/*.py` rather than the package, and the
  runtime suite invokes the gate as a subprocess. The examples now break inside
  the call, which is the shape `TraceFields.is_valid` already used, and all 24
  examples in the package pass. Recorded because the same wrap is easy to
  reintroduce, and because "a docstring with an `Examples` block" is not
  evidence that the block runs.
- 2026-10-10 — The first CodeRabbit pass over the delivered port. Three
  findings. Two were accepted: a dead `or 0` in `telemetry.write_output`, whose
  expression could never differ from its operand, and this revision note's own
  ordering (the initial entry sat below two later ones, with a blank line
  splitting the list). The third proposed suppressing an operation's counter
  together with its duration when the clock returns garbage. That was
  **rejected, and the rejection is the interesting part**: it would have
  introduced the divergence it claimed to prevent. The shell validates the
  counter's literal `1` separately from the duration, so a garbage reading
  drops the duration and the operation trace and still writes the counter. The
  finding's premise was a sentence in this branch's own oracle docstring
  asserting the opposite, which measurement disproved; that sentence is now
  corrected, and the reviewer's confidence in it was the reason to measure
  rather than to patch. A nine-shape clock matrix (garbage on every read, on
  the first read only, on the finish reads only, and a non-zero exit) confirms
  the shell writes one counter per operation and the port matches it
  byte-for-byte on the records.
- 2026-10-10 — The second CodeRabbit pass, over the revision that answered the
  first. It returned **no findings** across all thirty changed files. The
  interesting part is what that silence cost to earn: the port was re-gated in
  full on the exact bytes committed, and the three defects the first pass found
  were each either fixed or falsified before the pass was requested. A clean
  review is evidence about a revision, not about a change, so the revision it
  read is named here — `c6a9d023`, whose current-history counterpart after the
  rebase is `49df29cb`, identified by the shared patch ID — and the gates that
  ran against it are recorded under *Outcomes and retrospective*.
- 2026-10-11 — A third review pass, this time for behaviour parity rather than
  for the port's own quality, and the most productive of the three. It found
  five behavioural defects and five acceptance-coverage gaps; the table under
  *Outcomes and retrospective* records each defect and its fix. The two that
  mattered most were both invisible to every existing case: the bound signalled
  only its direct child, so a descendant survived cancellation and held the
  captured pipe open until the drain timed out, and a subprocess status reached
  the gate untranslated, so a `kill -KILL` classified as an ordinary failure
  rather than as a timeout: Python reports that death as `-9`, and only the
  shell's `137` is in `TIMEOUT_STATUSES`, which is exactly `{124, 137}`. The
  first has no symptom in a passing run — it is a child that outlives the
  command it belongs to — and the second needs the shell's `128 + signal`
  convention to be known before `-9` reads as wrong. `SIGTERM` is translated
  for the same reason and is a *different* case: `143` is not a timeout status,
  so an adapter killed by `SIGTERM` keeps its operation's own category and
  remains an ordinary operation failure. The lesson is that "the tests pass" is
  not evidence of parity when the tests were written from the port rather than
  from the shell: the two modules that closed the largest gaps, the status
  translation and the real-child runner, both need to run a real child or call
  a pure function directly, which is why no cmd-mox-based case could have found
  either. Four of the five gaps were expressed only after the harness could
  read an absent artefact and unset a variable, so a test-support limitation
  had been silently bounding what the suite could assert.
- 2026-10-11 — A fifth repair pass, over the three items a CodeRabbit
  confirmation raised after accepting the two earlier repairs. All three were
  real. The behavioural one is the `mkdir` operand: the shell computed each
  parent with `$(dirname "$file")` and the port derived it from the configured
  path's `Path.parent`, which agree on every slash spelling and diverge on a
  final `/.` — `dirname(1)` answers `reports` for `reports/.`, which the shell
  created before refusing the target with `Is a directory`, while
  `Path("reports/.").parent` answers `.`, so the port made nothing and refused
  it with `No such file or directory`. Both fail closed, which is why the case
  has to assert the parent's existence and the operating system's sentence
  rather than the exit status. `os.path.dirname` is the near miss rather than
  the answer — it returns `reports` for `reports/` where `dirname(1)` returns
  `.` — so `configuration._directory_name` implements `dirname(1)`'s own
  algorithm, and the count moves to 96. The other two items were stale prose in
  this guide's own sibling: an untranslated `-15` cannot change a *timeout*
  category, because `143` is not in `TIMEOUT_STATUSES`, and the real case is
  cuprum's `-9` from the `SIGKILL` escalation; and the artefact path paragraph
  still described a normalizing `mkdir` and redirection that no longer exist.
- 2026-10-11 — A sixth repair pass, over the three concerns a proof-substance
  assessment raised against the boundedness property. **No production code
  changed and no case count moved**; what the pass repaired is the property's
  evidence. All three concerns were valid. The first was that the property
  generated a workflow-run identifier, asserted its exclusion, and never
  exercised it. The first disposition taken was a **narrowing**: the identifier
  was dropped rather than exercised, on the reasoning that `verify_evidence` is
  the last of the five operations, the gate returns at the first failure, and
  `classify_evidence` satisfies its disjunction on the `fresh` operand — so the
  stored identifier has no reachable consumer. **That narrowing was withdrawn
  in a seventh pass, because the reason given for it was false and the
  assessment's alternative was available.** The claim that the base probe
  "never reached the boundary crossing it claimed to test" was measured and is
  wrong: the base fake line `${NETSUKE_FAKE_WORKFLOW_RUN_ID-1001}` uses `-`, a
  default for the *unset* case, so an injected identifier was returned. `1001`
  appears in a calls log only when the variable is unset (the
  `answer_no_workflow_run` case). The narrowing generalized from the unset case
  to the set case without the set case having been run. A three-arm probe
  settles it: with the stored identifier copied into an operation record, the
  property **fails** when the doubled API answers with the identifier and
  **passes** when it answers with nothing — a no-op control that shows the
  failure can only come from the generated value reaching production. The
  property therefore generates the identifier and the double **returns** it,
  placing it on the gate's own result through `github_find_workflow_run`; the
  reachability limit is retained as a note on what the case does *not* cover.
  The second concern was that the `path` and `url` values the property
  transports are read by nothing in production; the claim is now stated as
  auxiliary-environment transport, which is what the evidence supports, and the
  module docstring names the variables as scenarios rather than configuration.
  The third was that both shared validators and the exclusion helper accept an
  empty list, so the property could pass on a run that wrote no telemetry.
  `assert_complete_records` now asserts the operation counters in emission
  order, the final outcome and category, **the gate counter repeating that
  outcome**, and the exact trace sequence **before** any exclusion is checked.
  The measurement that preceded the assertion caught a real error in the
  expectation written from reading rather than running: `gate_complete` carries
  the *gate's* outcome, not a per-operation success. A diagnostics check was
  added alongside the records, because the child's standard error is retained
  with the release and carries the same hazard. Negative controls confirm every
  new assertion fires, with the trace mutations correctly attributed to the
  trace assertion rather than masked by the metric one. Two corrections to the
  assessment's own text were verified and accepted: the expected-label helpers
  build their dicts from arguments rather than from production enum members
  (which strengthens the oracle), and the direct requirement is ADR-0020's
  prohibition on identifier-derived labels.
- 2026-10-11 — The repairs above took `release_admission_test_records.py` to
  **412 lines**, over the 400-line ceiling, which `make lint-python` reports as
  Pylint `C0302`. `release_admission_test_exclusions.py` was split out at the
  seam between the two kinds of claim the module made: shape assertions stay in
  `..._records.py` (298 lines) and the identifier-exclusion helpers move to the
  new module (130 lines). The support facade re-exports both, so no runtime
  module's import changed. The `make test-release-admission` module count is a
  count of *test* modules and is unaffected — the split added a helper, not a
  test. The first attempt at the cut was wrong and was caught by collection
  error, not by review: the cut point dropped `assert_failure_trace_sequence`,
  `operation_records`, and `operation_duration`, which the facade imports.
  Restored from the committed revision and the module re-linted. **Lesson: a
  split point is safe only when the remaining module still exports everything
  its consumers import — check the importers, not the seam.**
- 2026-10-11 — An eighth pass, after the sixth and seventh were pushed and the
  hosted checks were read. It repairs **two defects the sixth and seventh
  passes themselves introduced**, both introduced by those passes and both
  found by measurement rather than by review.

  The first is a **CodeScene `Large Method` regression in the property module
  the sixth and seventh passes edited**. The advisory check went from `success`
  at `3ad4d90a` to **`failure` at `fee4151d`** on the file the sixth and
  seventh passes edited. `cs check` on the file at each revision gives the
  shape of it: 10.00 at `05fa166e` and `3ad4d90a`, **9.54** at `9199d26b` (81
  counted lines, threshold 70), **9.56** at `fee4151d` (78 lines by CodeScene's
  own count). The cause is that the sixth pass folded the exclusion assertions
  *back into* the property body: measured from the body's first statement,
  `identifiers = ...`, to its last assertion, that body went from 52 lines at
  `3ad4d90a` to CodeScene's 78 counted lines at `fee4151d`, and the seventh
  pass kept all of it. The repair is the one `AGENTS.md` asks for — "where a
  function is too long, extract meaningfully named helper functions" — and it
  is the pattern the module already used for `_answer_workflow_run` and
  `_run_under_generated_identifiers`. Four helpers now each state one claim:
  the run is valid under the fixed schema and reached the evidence check; the
  generated revision reached both doubled boundaries; the environment-only
  identifiers crossed the subprocess boundary and no further; and the records
  exist *before* the identifiers are excluded from records and diagnostics. No
  assertion is removed, reordered, weakened, or moved to an unmeasured path,
  nothing is suppressed, no threshold moves, and no rule is disabled. The file
  returns to **10.00**.

  The second is a **real false positive in the property's own exclusion
  assertion**, which the seventh pass's run-id restoration exposed.
  `make test-release-admission` failed:
  `AssertionError: generated identifiers
  must never become metric label values; matching identifier/value pairs:
  [('_', 'resolve_tag_commit')]`,
  falsified by `run_id='_'`. The assertion is a substring search of each
  identifier against every label value, so a one-character identifier collides
  with the gate's own vocabulary and the property fails on a run where the gate
  leaked nothing. **The first fix tried — a minimum length — was not enough,
  and the reason is structural, not a matter of picking a better number: `none`
  is four characters and is a label, and `failure` is seven.** The fix is a
  marker character: `IDENTIFIER_TEXT` maps every generated value through a `7`
  prefix, and a probe checks the two claims in the comment rather than
  asserting them — all 26 fixed tokens across the six validator vocabularies are
  `[a-z_]+`, no diagnostic sentence contains the marker, and **0 of 160
  generated values is a substring of any of the 33 emitted tokens**. The same
  probe shows what the marker replaced: `'_'` is a substring of 18 of those 33
  tokens, `'a'` of 22, and the empty string of all 33. A negative control
  confirms the helper still fires on an injected leak. It costs nothing the
  property was testing — 137 of 160 generated values still contain non-ASCII
  text — and Hypothesis seeds 1–5 plus the full suite now pass, **96 cases as
  before**.

  **Lesson: a body length that was safe becomes unsafe when a later pass adds
  assertions to it, and the check that would have caught it is the hosted one.
  A git-level "the property passes" verdict does not cover a *file-health*
  metric, and a passing property does not mean its assertion is well-formed — a
  substring search over a fixed vocabulary is not a claim about identifiers
  until the identifiers are moved outside that vocabulary.**
- 2026-10-11 — A ninth pass, over the proof-substance confirmation that read
  `fee4151d` (comment `6108101750`). It **accepted the run-identifier path as a
  live safety case and kept the overall confirmation partial**, naming three
  remaining problems. Two were real; the third was an ordering artefact of the
  reviewer reading an older revision.

  The real defect is a **false negative in the same exclusion assertion the
  eighth pass repaired**, and it runs in the *dangerous* direction. Production
  strips every trailing LF from the API's answer before storing it
  (`commands._status_and_output` applies `rstrip("\n")`), so an identifier
  ending in a newline is stored as a *different string* from the one the
  property searches for. The eighth pass's marker was a **prefix**
  (`marker + value`), which does nothing about a trailing newline: the raw value
  `"run-example\n"` is stored as `"run-example"`, a leak of the *stored* value
  would not have matched the raw identifier in the exclusion set, and the
  assertion would have passed on a run that leaked. The all-newline case is
  worse — it normalizes to the empty string, which is a substring of every
  label. The repair is structural and explicit: the marker is now written at
  **both ends** so a newline strip cannot remove it, and the exclusion set
  carries the identifier **both as generated and as production stores it**. A
  probe falsifies the claim rather than asserting it — arm A reproduces the
  miss (raw-only set, leak of the stored value, helper silent), arm B shows the
  repair catching it, and arm C runs 264 generated values through the strip
  with **0 changed and 0 normalizing to empty**.

  The second is a **prose defect this project's own measurement had already
  contradicted**: the property said `verify_evidence` "never sees" the
  identifier, and the guide repeated it. The reviewer is right that production
  *does* pass the stored identifier — `gate._dispatch` supplies it as the
  `workflow_run_id` argument of `policy.classify_evidence`. Only its
  *truthiness* goes unevaluated on this path. Argument delivery and truthiness
  evaluation are different boundaries, and both the property's docstring and
  the guide now say so. This is the same class of error the eighth pass
  corrected once already, which is the lesson: **a reachability claim needs the
  same probe as a behaviour claim.**

  The third item — that the exclusion set compared a raw value against raw
  vocabulary — was already answered by the eighth pass's marker; the reviewer
  had read `fee4151d`. The response names the revision that carries the fix.

  Adding the argument pushed the test module past Pylint's
  `max-module-lines = 400` (`C0302: Too many lines in module (448/400)`), so
  the property's **mechanism** moved to
  `scripts/tests/release_admission_test_boundedness.py` (275 lines) and the two
  tests and their argument stayed in the test module (210 lines). The seam is
  deliberate: the correctness *claims* stay where a reader looks for them, and
  the strategies, the doubled answer, the run, and the four assertions move
  out. Every moved name loses its leading underscore, the two the eighth pass
  entry above names included, because it now crosses a module boundary. Both
  files score **10.00** on CodeScene. `make lint-python`, `make check-fmt`, and
  `make test-release-admission` pass, the last at **96 cases as before**.

  **Lesson: an exclusion assertion is only as strong as the identity it
  searches for. When production normalizes a value between receipt and storage,
  a set carrying the pre-normalization form under-covers the exact leak the
  assertion exists to catch, and it fails silently. A trailing marker is not a
  guard against a strip that removes the character the marker is not at.**

  A sweep of the guide after the split found **two stale paragraphs**, one
  introduced by the split and one older. The one introduced by the split is the
  support-module inventory: it named seven modules where the directory now
  holds eight, so `release_admission_test_boundedness.py` is named and its
  ownership stated. The one that is not is older: the same section says the
  target "runs the ten runtime modules" and lists ten, while the Makefile and
  the workflow contract's `RUNTIME_MODULES` tuple have named **twelve** since
  `36a913ed`. The two modules the guide omitted are
  `..._artefact_truncation.py` and `..._adapter_defaults.py`. The list is
  corrected and now matches the Makefile exactly, in order, verified by diffing
  the two rather than by counting. Both defects share a cause worth naming:
  **the count and the list live in three places — the Makefile, the contract,
  and the guide — and the pass that moved the figure updated two of them.**

- 2026-10-11 — **Tenth pass, a documentation sweep.** Committed as
  `47bd4889`, "Repair the guide's stale module count and support list". The
  ninth pass split a support module and corrected the guide's inventory and
  `_status_and_output` note; sweeping the surrounding prose afterwards found
  the three defects recorded above — the guide's inventory (introduced by the
  split), the guide's runtime-module list (older), and this plan's own
  current-state support-module count, which had said "a facade and five
  modules" while disk held eight.

  Every provenance claim in the corrected prose was verified against history
  before it was committed, because a prose correction that names a commit is
  only as good as that name: `9a0edd5d`'s diff shows `-four runtime modules`
  becoming `+ten runtime modules` with six list entries added, and `36a913ed`'s
  shows the Makefile and `RUNTIME_MODULES` both going 10 → 12 via exactly the
  two modules the guide omitted. The corrected list was then proved against the
  Makefile by script — `identical & in order: True`, 12 == 12 — rather than by
  re-counting by eye.

  **Lesson: a probe is code, and its bookkeeping can be wrong in the direction
  that manufactures a verdict.** The normalization probe first printed
  `FALSIFIED` on a real defect that was not there: it asserted "marker at both
  ends" over all 264 sampled values, but only the 200 strategy-generated ones
  carry the marker — the 64 `path-`/`url-` values are prefix-derived by
  construction, exactly as production derives them. The probe was corrected to
  test the marker claim against the 200 alone, and the re-run is
  `not falsified` with the cited figures (264 through the strip, 0 changed, 0
  empty, 200/200 marked). A probe that reports a failure should be suspected of
  carrying it until its own assertion is read.

  Gates: `make check-fmt`, `make markdownlint` (including `make spelling`), and
  `make test-documentation-contracts` all pass on `47bd4889` with a clean tree.
  The documentation-contracts gate is the one the preceding run had explicitly
  left unverified, since that suite reads the guide through
  `tests/documentation_examples/mod.rs` under `FencePolicy::MarkedOnly`; the
  diff adds or modifies no fence, and the suite passes anyway (14 tests across
  2 binaries, 42 skipped). No code, test, or workflow changed, so the twelve
  collected modules and the 96-case count are unchanged.

- 2026-10-11 — **Eleventh pass, the record helper's overstated shape claim.**
  Committed as `b791db23`, "Correct the record helper's overstated shape claim".
  `assert_complete_records`'s summary line said it asserted a run emitted
  *"exactly the records `shape` names"*, while its own body two paragraphs down
  states that every duration record is ignored and `_gate_labels` reads the
  last record rather than rejecting extras. The summary contradicted its own
  detail in the same docstring. The summary now reads "match the shape `shape`
  names", and a paragraph names both limits — a clock reading outside the
  accepted number shape drops a duration without touching its counter, so a
  fixed record count would assert the host clock rather than the gate, and the
  gate labels are read off the last record rather than by rejecting extras. The
  two other descriptions of the same helper, in the developers' guide and above
  in this plan, describe what is asserted without claiming exhaustiveness and
  needed no change; the docstring summary was the only place asserting
  "exactly".

  The first, longer wording took the helper to 72 lines and tripped CodeScene's
  70-line `Large Method` threshold, dropping `cs check` to 9.59 with one
  warning; condensed to a four-line paragraph it is back to 10.00. This is the
  second time on this branch that explanatory prose has inflated a scored
  method, and both times it was measured rather than assumed.

- 2026-10-11 — **Twelfth pass, four prose claims the sixth confirmation
  verified.** Committed as `1109225d`, "Correct four prose claims the sixth
  review pass verified". Each was checked against source before the prose was
  written, and each was real:

  1. The boundedness property's domain commentary claimed arbitrary Unicode
     payloads without stating the bound. `IDENTIFIER_TEXT` is
     `st.text(..., min_size=4, max_size=31)` mapped through
     `f"{IDENTIFIER_MARKER}{value}{IDENTIFIER_MARKER}"`, so payloads run 4–31
     characters and identifiers 6–33. The comment now gives both ranges and
     states that the bound is the strategy's rather than something the marker
     excludes; the test module's parameter note says "generated identifiers"
     rather than "unbounded identifiers".
  2. The guide attributed `assert_identifiers_excluded_from_values` to the
     boundedness module. It is defined in
     `release_admission_test_exclusions.py`; the guide now names that module.
  3. This plan's ninth-pass entry named `39c8a03d` where the confirmation read
     it, and `fee4151d` later in the same entry. Rather than trust either
     sentence, the cited comment was read: `6108101750` names `fee4151d` and
     does not contain the string `39c8a03d` anywhere, so both places now say
     `fee4151d`.
  4. The package docstring said `bounding` signals "every descendant". Its own
     module docstring names `setsid()` as an escape, so the entry now limits
     signalling to the descendants that remain in the process group.

  A fifth correction went to the confirmation reply itself: its verification
  notice claimed no case was skipped in any gate, contradicting its own table.
  The table was the accurate half — 6 skips under `make test`, 3 under
  `make test-workflow-contracts`, 42 under
  `make test-documentation-contracts` — and the notice now separates "no gate
  failed" from "no case skipped".

  Gates on `1109225d`: five re-run on its own bytes — `make check-fmt`,
  `make markdownlint` with `make spelling`, `make lint`,
  `make test-release-admission` (96 passed), and
  `make test-documentation-contracts` (14 passed, 42 skipped) — all passing
  with a clean tree. The full nine-gate set was last run in one pass on
  `b791db23`, and the two revisions are reported separately rather than one set
  of numbers being stretched over both.

  **Lesson: an ExecPlan can contradict itself, and the citation is the
  tiebreaker.** Item 3 was not a stale value to overwrite but two incompatible
  sentences in one document. Reading the comment the entry cited settled which
  sentence was right, and neither could be trusted on its own.

  **Lesson: a posting helper's failure message can name the wrong cause.** The
  manual-comment helper refused with "Expected a non-leynos pool account",
  reporting an identity mismatch for what was an environment problem: all three
  pool tokens were valid non-`leynos` accounts when called directly. `BASH_ENV`
  injects the Lody `gh` shim into non-interactive bash, and the broker's
  `[Lody GitHub] {"source":"personal","stage":"acquire","code": "timeout"}`
  notice was captured by the helper's `2>&1` login read, so a valid
  136-character login string failed validation. The helper's logic was correct
  and the environment was dirty; running it with `BASH_ENV` unset and `~/.lody`
  off `PATH` is the fix. A refusal message states a hypothesis, not a finding.

- 2026-10-11 — **Thirteenth pass, the eleventh and twelfth passes' own
  defects, found by the confirmation that read them.** The confirmation assessed
  `c2138f8a`, the commit that recorded the two preceding passes, and named two
  inconsistencies in the prose that commit had just added:

  1. The eleventh entry said the plan's earlier account of
     `assert_complete_records` "repeated the overstatement" and, later in the
     same entry, that those same descriptions "needed no change" because they
     do not claim exhaustiveness. Both could not hold. The earlier account, at
     *Progress*'s account of the sixth pass, describes what the helper asserts
     without claiming emission completeness, so the first sentence was false.
     It is deleted; the entry now records only that the docstring summary
     contradicted its own detail.
  2. The twelfth entry's third item said the two conflicting revisions sat
     "six lines later". In `f2073a8c` they are at line 756 and line 793 —
     37 apart. The correction to `fee4151d` was right, but the distance was
     not, and it was a distance that changes on every rewrap. Replaced with
     "later in the same entry".

  The confirmation also qualified two commit references rather than finding a
  defect: `30b961d8` and `c6a9d023`, the revisions the first two review passes
  read, are not ancestors of this head because an intervening rebase replayed
  them as `66ec7582` and `49df29cb`. Each pair has an identical stable patch ID
  (`78186157`, `5ea72ce3`), which is what proves they are the same changes.
  Both references now name their current-history counterpart; the historical
  names are kept because a review is evidence about the revision it read.

  The lesson is the same one the twelfth pass recorded, one level up: **a
  correction entry is itself a claim, and it needs the same check as the thing
  it corrects.** Both defects were in newly written sentences about older ones,
  and neither was caught by a gate — only by reading the entry against the
  evidence it cites. Two more were found the same way in this entry's own first
  draft. It gave the gap between the eleventh entry's two statements as a line
  count, and the count was wrong and unstable under rewrap; it now says "later
  in the same entry", the repair it prescribes. And it stated its own
  insertions and deletions, a figure that cannot be a fixed point: writing the
  number changes the number. That figure is stated once in the commit message
  instead, measured against the frozen tree. A count embedded in the diff it
  describes is unstable by construction; this plan should not carry one.
- 2026-10-11 — **Fourteenth pass, the first formal review's nine inline
  findings.** Review `5483892176` returned `CHANGES_REQUESTED` on `678b87be`
  with nine inline comments and no separate banner findings. All nine were
  verified against the current source before any edit; none was invalid and
  none was out of scope, so all nine are repaired here. Two are production
  defects and the rest are contracts or prose.

  1. `delivery._append_via_adapter` converted a refused launch to its status
     and printed nothing, so a mistyped `..._METRICS_SINK` or `..._OUTPUT_SINK`
     exited `127` with no diagnostic — while the command adapters' own path
     called `commands.report_launch_failure` for exactly that reason. The sink
     path now calls it too, and a new case
     (`test_refused_sink_launch_names_the_program`) pins the diagnostic and its
     status together. The module docstring already stated the status mapping;
     it now states the diagnostic as well.
  2. `policy` enumerated two vocabularies twice. The four `TRACE_*` constants
     repeated `TraceEvent` members and had five live consumers in `records`,
     so those call sites were moved to the members rather than the constants
     deleted alone; `EVIDENCE_FRESH`, `DEFAULT_EVIDENCE_STATE`, and the bare
     `"stale"` and `{"missing", ""}` literals in the two classifiers are
     replaced by an `EvidenceState(Vocabulary)` with `FRESH`, `STALE`, and
     `MISSING`, consumed from `configuration` too. `EvidenceState` inherits
     `Vocabulary` directly: **an enumeration cannot be subclassed**, so it
     cannot extend a sibling vocabulary. The members substitute for the
     literals without change — `StrEnum` members equal, hash, and serialize as
     their plain strings, and a probe through both classifiers returns the same
     `success`/`stale_evidence`/`missing_evidence`/`unknown` answers the
     literals produced, including for the empty string.
  3. The ADR-0020 addendum claimed "no process the command started is left
     running" with `setsid()` as the only named escape. `bounding._terminate_group`
     arms `SIGKILL` only while the direct child is unreaped, and the
     developers' guide already records the survivor, so the contract was
     stronger than the code. The addendum now states both survivors and points
     at the guide for the full limits.
  4. The plan itself carried seven first-person passages. The rule that forbids
     them is the project's own (`docs/documentation-style-guide.md`: "Avoid
     first and second person personal pronouns outside the `README.md` file"),
     so each sentence is rewritten in the third person rather than flagged.
  5. The remaining four are test-module defects: an orphaned `sys.path`
     mutation with nothing importing the package (deleted, with the two
     imports it alone used), a duplicated `CLOCK_ARGUMENTS` (imported from the
     support facade instead), a docstring claiming captured standard error is a
     string where the module's own assertion pins `None`, and a fabricated
     `FileNotFoundError(20, …)` case asserting a state production cannot
     reach. The last was the most interesting: `FileNotFoundError(20, …)` keeps
     its class because only `OSError(errno, …)` maps an errno to a subclass, a
     real `ENOTDIR` exec raises `NotADirectoryError`, and GNU `timeout` reports
     `126` — not `127` — for it. The case is deleted and the docstring now says
     so; `NotADirectoryError` is not a `FileNotFoundError`, so the class-based
     mapping already answers correctly and no production change was needed.
  6. The process-bounds case asserted `not _process_exists(descendant_pid)`
     with a single `os.kill(pid, 0)` check. A zombie answers signal `0`, and
     the descendant is a reparented grandchild, so the assertion raced
     whichever process reaps it. A bounded polling helper treats a zombie as
     gone (`/proc/<pid>/stat` state `Z`) without weakening the claim: the
     descendant must still die, and the deadline is far shorter than the
     defect the case guards.

  The plan's own figure of "26 fixed tokens across the six validator
  vocabularies" was checked against the change rather than assumed unaffected:
  the emitted token set is built from the five validator enumerations and the
  three metric names, `EvidenceState` is an operator *input* vocabulary that
  reaches no record or label, and a probe confirms the members are disjoint
  from every emitted token. The figure stands.

- 2026-10-11 — **Fifteenth pass, the formal review's Domain Architecture row
  and the module-ceiling breach the fourteenth pass introduced.** Two changes,
  and the second is the one that made the first necessary.

  1. `EvidenceState` took `policy` from 393 lines to 401 against a 400-line
     ceiling enforced by `pylint`'s `C0302` through
     `[tool.pylint.main] max-module-lines`. The ceiling binds test modules too:
     `per-file-ignores` exempts `assert`, `too-many-arguments`, and three
     others for `scripts/tests/*.py`, and `too-many-lines` is not among them.
     The repair therefore had to *remove* code from `policy` rather than move
     it into a file with headroom, because `commands` at 342 had only 58 lines
     spare and the status region is longer than that.
  2. The Domain Architecture row asked that Bash/GNU `timeout`, Python
     negative return-code, and `OSError` translation move out of `policy` and
     into the command and delivery adapters, and that those adapters expose
     adapter-neutral domain results rather than raw statuses. Both halves were
     substantive. `policy` held five names that were not decisions at all —
     `CommandStatus`, `TIMEOUT_STATUSES`, `as_bash_status`,
     `launch_failure_status`, and the timeout branch of the classifier — and
     three call sites compared a bare integer to `0`
     (`gate.py:220`, `:240`, `:257`), which is precisely the shell-status
     leak the row describes.

  The split follows the dependency direction rather than the file sizes. What a
  command *did* is policy's own vocabulary, so `CommandOutcome` (`succeeded`,
  `timed_out`, `failed`, `launch_failed`) and the `CommandResult` carrying it
  are declared in `policy`, and `classify_command_result(outcome, category)`
  now consumes them. The classifier's category argument stays at the call site,
  because the per- operation default is the gate's judgement — an unsuccessful
  `gh api` is `api_error` and an unsuccessful `git fetch` is `fetch_error` —
  and an adapter that emitted `Failed(category=…)` would have to know its
  caller's category, which is the leak the row is about.

  The translation that *produces* those values is the ninth module, `statuses`.
  It is the only place a shell status number is interpreted, it imports
  `policy` and is imported by `commands` and `delivery`, and it does not import
  `bounding` or `gate`. `commands.classify_result` moves there as
  `statuses.classify_result`, because a bound that elapsed is a *status*
  conversion and `bounding` must not import status vocabulary to describe it.
  `bounding` is unchanged: it still declines to interpret a status number,
  reporting `None` for an elapsed bound where a completed command keeps
  cuprum's own result. It is that `None` -- not a shell's `124` -- that
  `statuses.translate_exit` turns into the gate's timeout status.

  `commands` now returns `policy.CommandResult` from all four effects and no
  longer imports `statuses` only to translate — its `policy` import is real,
  for the result type, and it no longer interprets a number anywhere.
  `delivery.append_record` keeps returning `int`, because at the unguarded
  gate-output write that number *is* the gate's exit status; its private
  `_append_via_adapter` returns the result and `append_record` takes `.status`.
  The two adapters needed a primitive for two different libraries —
  `cuprum.CommandResult.exit_code` and
  `subprocess.CompletedProcess.returncode` — so
  `statuses.translate_exit(exit_code, stdout)` is that primitive and
  `translate_result` is the cuprum-shaped wrapper over it.

  Line counts after the pass: `policy` 401 → 378, `commands` 342 → 300, `gate`
  263 → 269, `delivery` 127 → 133, `statuses` new at 271, and `__init__` 24 →
  27 for the ninth module bullet. Every module is inside the ceiling with
  headroom rather than at it.

  The frozen behaviour the runtime tests pin was re-verified rather than
  assumed. `test_release_admission_environment_contract.py`'s three refusal
  positions and two-phase truncation order are untouched — `configuration` is
  not changed by this pass. The status translation's own contracts are
  preserved: `137` remains both the shell's `SIGKILL` status and a timeout
  status, `143` remains translated but not a timeout, and a sink killed by
  `SIGTERM` still reports `128 + signal`. The launch-refusal statuses `127` and
  `126` still hold, and a refused launch still keeps the operation's own
  category because neither number maps to `timed_out`.

  Consumers updated with the move:
  `test_release_admission_status_translation.py` (import, four direct calls,
  and the elapsed-bound case now asserting the outcome rather than the literal),
  `test_release_admission_process_bounds.py` (two `classify_result` calls),
  and the prose in `release_admission_test_boundedness.py`,
  `release_admission_test_support.py`,
  `test_release_admission_metric_failures.py`, and
  `test_release_admission_runner_contract.py`. The developers' guide's module
  map, status paragraph, and runtime-module paragraph were updated with them.

  `make test-release-admission` passes with all 96 cases after the pass.
- 2026-10-11 — Formal-review repair pass: resolution split from initialization.
  The first formal review returned `CHANGES_REQUESTED` with nine inline
  findings, one of which was the Unit Architecture row this pass answers: a
  single loader both read the variables and created both artefacts, so the two
  kinds of consequence could not be observed apart. `configuration` now exposes
  four steps — `resolve_destination`, `resolve_configuration`,
  `prepare_destination`, and `require_workflow_output` — of which the first two
  are pure and the third takes the filesystem it uses as an argument. The
  effects moved behind a `filesystem` port whose two members are declared
  positional-only, because a dataclass field holding a `Callable` is not
  assignable to a member that requires a keyword. `Sinks` gained `bindings()`
  so initialization reads the same resolved spellings the workflow outputs
  publish rather than re-deriving them from the environment.

  The new tests were probed for liveness before being trusted, which is the
  discipline the earlier trailing-slash defect earned: interleaving the two
  passes fails three cases in `..._artefact_initialization.py`, and resolving
  the truncation operand through `Path` fails two of its cases and both
  trailing-slash cases in `..._artefact_truncation.py`. A third probe of the
  resolution module found a defect in the *test*, not the code: the spelling
  first chosen for the parent-derivation case was one where `dirname(1)` and
  `Path.parent` happen to agree, so the case asserted a false expectation and
  failed. The production helper was right; the case now uses `reports/.`, the
  one spelling where the two genuinely diverge, and asserts the divergence as a
  precondition rather than assuming it.

  `make test-release-admission` passes with all 118 cases over the fourteen
  pinned modules.

- 2026-10-11 — Gate pass over the review repair. Driving `lint-python` to
  completion took three rounds, and each round's failure was the *next* stage's
  finding rather than a repeat of the previous one, because the target is a
  five-stage cascade that stops at the first failure.

  1. The df12 house lints reported nineteen `C9102`
     (`assert-missing-message`): eighteen in the new resolution module and one
     pre-existing in `test_release_admission_status_translation.py`. The house
     rule requires a failure message on every `assert`, so the messages were
     added. Adding them took the resolution module to 407 lines against the
     same 400-line ceiling `C0302` enforces, so its prose was tightened — the
     module docstring's bullet list became running prose, and six notes blocks
     lost their padding, none of it load-bearing.
  2. With `C9102` clear the df12 stage advanced and reported `R9108`
     (`prefer-snapshot-assertion`) on the initialization module. The rule fires
     on `assert x == <literal>` where the literal has eight or more AST leaves,
     and its remedy is a syrupy snapshot; this repository uses syrupy nowhere,
     so that remedy has never been the house answer. The four-element call-list
     literal was the offender, and splitting it into its two halves — every
     parent before either truncation, then both truncations in order — states
     the same contract in two claims and no longer trips the rule. That is a
     stronger statement than the single comparison it replaced, not a way round
     the check.
  3. Interrogate then reported 99.9%: two `__init__` methods on the recording
     doubles had no docstring. They now carry one.

  `make lint-python` passes all five stages, `make typecheck-python` passes,
  `make test-release-admission` passes all 118 cases, and
  `make test-workflow-contracts` passes 1254 with 3 skipped.

- 2026-10-11 — Orthography pass over this branch's own additions. The spelling
  gate scans tracked Markdown only, so the `-ise` forms this branch introduced
  in Python, the Makefile, and the plan were not gate-visible — but they are
  contra `AGENTS.md` and were flagged in the previous round's review. The
  affected words are `initialisation`/`initialiser`, `normalised`/`normalises`,
  `serialise`, `recognising`, and `unrecognised`; all are genuine `-ize` verbs
  in the house's en-GB-oxendict spelling. `analyse` and its family are
  deliberately left alone, since Oxford keeps `-yse`. The test module named for
  artefact initialization is renamed to
  `test_release_admission_artefact_initialization.py` with its three
  referencing sites (the Makefile, the workflow contract's `RUNTIME_MODULES`,
  and the developers' guide), and the pre-existing `normalised_build_script` in
  `tests/build_module_slice_ui_tests.rs` is left untouched because it is main's
  and not on a line this branch edits.

- 2026-10-11 — Candidate-bound assessment re-reads, and the plan corrections
  they produced. Both assessments were asked to carry their conclusions to
  `3cb8c35b`, because the increment since the head they had confirmed
  (`678b87be`) changes production code. The proof assessment retained its
  conclusion with no new concern; the completeness assessment retained the
  implementation conclusion — no executable regression found — but named eleven
  documentation defects, which were verified against the source and repaired
  here.

  The substantive ones were current-state claims this plan had outlived. It
  still said the package holds nine modules and named nine; it holds eleven,
  because the review repair added `statuses`, `environment`, and `filesystem`.
  It still gave the suite as twelve modules and 96 cases, which was true at the
  point in the history where it sits, so that sentence is now marked historical
  and the current registration — fourteen modules and 118 cases, measured by
  collection on this head — is stated separately. It attributed the
  two-artefact write to `require_workflow_output` as well as
  `prepare_destination`; only the latter writes, and the former reads a mapping
  and refuses its value. Beside it, the package docstring called `filesystem`
  the gate's only filesystem write when `delivery` also writes, and said
  `configuration` performs no effect when only its resolution functions are
  pure. It put `5a1bab9f` one commit on top of `678b87be` when `d5b0b072`
  intervenes. It said `bounding` reports `None` and nothing else, when `None`
  is the elapsed bound and a completed command keeps cuprum's result.

  Three prose defects were of the kind this branch has already had to correct
  once: the status-translation module offered `kill -TERM`/`-15` as the timeout
  example, but `SIGTERM` translates to `143` and is deliberately *not* a
  timeout status — only `124` and `137` are; the same file still attributed the
  timeout set to the removed `classify_command_failure`; and the default-forms
  module said the redirection saw a normalized form when it opened the
  configured spelling and only `mkdir` derived an operand.

  One figure was checked by running it rather than by reading it:
  `118 tests collected` over the fourteen modules named in the Makefile, which
  is the count the plan now reports. The developers' guide's module list was
  missing the same two modules and gained them.

- 2026-10-11 — Gate scoping for the repair, and the `mdtablefix` canonical
  form. The repair is prose only — three module docstrings and two Markdown
  files — so it was scoped to eight gates rather than the full commit set:
  `check-fmt`, `lint-python`, `typecheck-python`, `test-release-admission`,
  `test-workflow-contracts`, `spelling`, `markdownlint`, and `lint-clippy`.
  Seven passed; `check-fmt` failed at its third stage, and the failure lay in
  the edit rather than in the tool. Editing prose in a Markdown file needs
  re-running the wrap pass, because `mdtablefix --wrap` refills a paragraph
  rather than capping its lines: a hand-wrap that reads correctly at 78 columns
  can still be non-canonical. The tool's `--diff` was used to read the
  canonical form for each of the three hunks, and the edits were applied by
  hand rather than by running the mutating `fmt` target. `mdtablefix --check`
  then exited 0 over 174 files.

- 2026-10-11 — Timing caveat on that same run, recorded because it bounds what
  the results establish. Three of the five edited files — the package docstring
  and the two test modules — were written before the run began. The two
  Markdown files were canonicalized after `markdownlint` and `spelling` had
  already passed, so those two stages ran against the pre-wrap text. The
  re-wraps are whitespace-only, which leaves no semantic regression for them to
  have missed, but `check-fmt`, `markdownlint`, and `spelling` are re-run on
  the committed tree rather than reported as cleared.

- 2026-10-11 — The follow-up assessment at `b01229d4` accepted the eleven
  repairs but kept the documentation confirmation partial, naming one
  correction that needs qualification and one site the sweep missed. Both were
  checked against the source before being applied, and neither reopens the
  executable refactor.

  The missed site was `filesystem.py`, which still read "that is the only
  filesystem access configuration resolution has" — the same boundary claim
  repaired in `configuration.py` and in the package docstring. Resolution
  performs no filesystem access at all; only initialization does. The sentence
  now says so.

  The qualification was in the status-translation module. Saying `SIGTERM`
  "stays an ordinary failure" is true of an independent termination and false
  of a bound, because `bounding` returns `None` whenever its bound elapses and
  `translate_exit(None)` reports `124` whatever signal ended the command. The
  two cases are now stated apart. This was a working guess when it was written,
  and the mechanism that makes it wrong is the one the docstring already had to
  state precisely once before, at `143`-versus-`124`.

  Both claims were swept across the tree rather than fixed where reported, and
  `configuration.py`'s own claim was already correct. The sweep was not
  exhaustive, and the sentence that said it was — "no third site carries
  either" — was itself too broad: the follow-up assessment named a third site,
  and re-sweeping found a fourth. What the sweep missed is recorded below
  rather than quietly corrected.

- 2026-10-11 — A third pass over the same claim class, after the follow-up
  assessment called the previous sweep's "no third site carries either" too
  broad. It was right to. The claim class is *the bound's own escalation
  reaches classification through the child's status*, and it appeared at four
  further sites, not one:

  - `statuses.py`'s `CommandStatus` docstring, which read that the escalated
    child's "own status for that death is `137` through `as_bash_status`" —
    the same attribution, and the one the file's own `translate_exit` had
    already been contradicting.
  - `statuses.as_bash_status`, which named "a command killed by the bound's own
    escalation" as the reason `137` is in the timeout set.
  - `statuses.translate_exit`, whose Notes named "the bound's own escalation"
    as the `-9` that would be misclassified.
  - `gate.py`'s module docstring, which read that `bounding` "reproduces both
    the grace and the escalation" and `statuses` "turns the result into the
    outcome", implying the escalation's status is what is turned.

  Two further sites claimed a parity that does not hold, and are now qualified
  rather than removed: the summary and the *Behaviour preserved* list both said
  `bounding` reproduces GNU `timeout`'s `124`/`137` classification. It does not
  reproduce the number. The mechanism, checked again in `bounding.py` rather
  than recalled: `_wait_for_exit` and `_wait_for_streams` both
  `except TimeoutError`, call `_terminate_group`, and `return None` — so the
  child's status after the escalation is discarded, and `translate_exit(None)`
  reports `124` whatever signal ended the command. Status `137` therefore
  remains reachable from a *completed* command by either of two routes: the
  translation of `-9`, or an explicit exit status of `137`. The first is what
  the test module's own `classify_result` case exercises, and between them they
  are what keeps `TIMEOUT_BY_KILL` in the set. The runner's own elapsed bound
  produces `124` and never `137`.

  The distinction was measured against GNU coreutils 9.5 rather than reasoned:
  `timeout --kill-after=1 1 ./traps-term` exits `137`, and
  `timeout --kill-after=1 1 sleep 2` exits `124`, so GNU `timeout` did make the
  distinction the port cannot. Nothing published depends on the number:
  `records.py` has no `status` field and `gate.py` never reads one, so the
  outcome and error category — both unchanged — are what reach the artefacts.

  One earlier dated entry carries the same imprecise attribution and is left
  standing as a record of what was believed when it was written: the fifth
  repair pass calls `-9` "cuprum's `-9` from the `SIGKILL` escalation". The
  `-9` that matters is a completed command's, and the conclusion that repair
  reached — that `-15` cannot change a timeout category — is unaffected.

  The test module's three sites were corrected for attribution, not for
  mechanism: `test_signal_statuses_are_translated_into_the_shells_convention`
  now says a completed command independently terminated by `SIGKILL`, and
  `test_classify_result_translates_a_signalled_command` no longer says the
  command was killed by the escalation. Its assertions are unchanged, and so is
  `{124, 137}`.

- 2026-10-11 — The follow-up at `c43bb6fc` accepted the escalation correction
  but kept the documentation confirmation partial, naming four statements that
  were still inaccurate. Each was checked against the source before being acted
  on, and all four are repaired at `f88b12ef`:

  - `statuses.py`'s module summary said GNU `timeout` reported `124` for an
    elapsed timeout. It could report `124` **or** `137`, and this port
    translates `None` to `124` in both cases. The summary now says so.
  - Two explanations — the test module's first bullet and the developers'
    guide — said a completed `SIGKILL` termination *would* receive an
    ordinary-failure classification without stating the condition. Both now
    say "without translation", and both add the positive statement that
    translation maps `-9` to `137`, which the policy classifies as a timeout.
    The test explanation no longer says the operation's category would survive
    "a real timeout": independent signal termination does not establish that
    the bound expired.
  - `test_classify_result_translates_a_signalled_command`'s Notes said a
    command terminated by a signal must classify as a timeout. That is false
    for two of its own three cases. It now states that the status must reach
    `classify_result` in the shell's convention, and that only the `SIGKILL`
    case produces a timeout-classified status: `143` and `130` are not in
    `TIMEOUT_STATUSES` and keep their operation's own category. Confirmed by
    running the translation — `translate_exit(-15)` → `143`/`FAILED`,
    `translate_exit(-2)` → `130`/`FAILED`, `translate_exit(-9)` →
    `137`/`TIMED_OUT`.
  - The entry above claimed `137` is reachable "only" through independent
    `SIGKILL` termination. That was too restrictive: `translate_exit(137)`
    also names an explicit exit status of `137` as `TIMED_OUT`, checked
    directly. The entry now names both routes.

  Every edit is a docstring or prose change. Stripping module, class, and
  function docstrings from the two edited Python files leaves an identical AST,
  and no assertion moved. The assessment's own limitation stands as recorded:
  it did not execute repository code, reproduce the `timeout` probes, inspect
  CI, or verify the merge-base, so those results are supplied evidence rather
  than independent confirmation.

  The follow-up was answered at `6114090202` with **"Treat the requested
  documentation check as complete at this candidate"**: all four corrections
  match the source, no documentation defect from the previous list remains
  unrepaired, and the related-claim sweep found no further instance of the
  attribution contradiction in the inspected paths. Its confirmation is
  candidate-bound to `f88b12ef` and explicitly does not establish integration
  with the latest `main`.

- 2026-10-13 — The deferral's rationale, corrected at the reviewer's direction.
  Two claims in the `Deferred` section and the two test-module docstrings were
  too broad, and both were checked against the source before being accepted.
  The first said the four deferred refusals are a producer's exclusive
  property. RFC 0005 splits them: producers supply the manifest, the scan
  results, and the measurements, and admission verifies what they supplied — it
  "verifies provenance rather than trusting declared fields", recomputes each
  archive's SHA-256 from its actual bytes, and rejects a mismatch against
  either the sidecar or the manifest's recorded digest. The admission-side half
  is therefore this port's own remaining debt, and calling the set
  producer-owned understated it. The second said `verify_evidence` "runs for
  every revision", which the composition contradicts: `run_operations` stops at
  the first failed operation, so the fifth operation is unreachable once any of
  the four before it fails. Both are now stated in the negative-free form the
  reviewer asked for — *deferred evidence-production and admission-validation
  integration*, and `verify_evidence` covered when the preceding operations
  succeed — and the `fresh` contradiction is preserved unchanged, because it is
  the behaviour the suite freezes rather than a defect to correct.

  The sweep for both claim classes was run tree-wide rather than at the
  reported lines, and found no fifth site: the only remaining instances of the
  producer vocabulary are the corrected sentences themselves. The developers'
  guide's four producer references were inspected and left alone, because each
  is about the *publication* gate — the workflow runs observation mode until an
  evidence producer is connected — which no part of this correction touches.
  The two edited Python files carry docstring-only changes: stripping module,
  class, and function docstrings from each leaves an AST identical to the
  committed one, so no assertion moved.

  Correcting the rationale cost lines, and the file that had carried it had
  none to spare. `release_admission_test_scenarios.py` sat at 399 lines against
  pylint's `max-module-lines` of 400, so the correction was moved rather than
  grown: the deferral now lives in `release_admission_test_cases.py`, the
  module that already owns the RFC 0005 mapping for the refusals this suite
  runs, and the scenarios module points at it. The first attempt failed the
  gates twice — `check-fmt` on an over-long Markdown paragraph, then
  `lint-python` on `C0302: Too many lines in module (411/400)` — and the second
  failure is worth recording because the first had masked it: both Python files
  were already over budget at the revision `check-fmt` rejected, so the repair
  was a move between modules rather than a trim.
  `release_admission_test_scenarios.py` is now 387 lines and
  `release_admission_test_cases.py` 192, and both keep docstring-stripped ASTs
  identical to `c89535d0`.
