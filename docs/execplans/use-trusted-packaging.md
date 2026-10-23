# Publish `netsuke-build` through crates.io Trusted Publishing

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

The plan is approved and implementation has begun: EP-M1 (the plan) and EP-M2
(Stage B) are complete, with Stages C and D open. No release, tag, registry
publication, GitHub Release, credential, or account setting has been changed by
this work.

## Purpose / big picture

Netsuke currently ships a binary on six platform targets through
`.github/workflows/release.yml`, which stages a **draft** GitHub Release and
leaves a human to publish it. Nothing in that path touches a registry, so a
user who runs `cargo install netsuke-build` today gets whatever bytes a
maintainer uploaded by hand, with no release job able to say which commit they
came from. After this change, a single tagged run resolves one candidate
commit, builds and verifies the platform artefacts and the Cargo package from
it, publishes the package to crates.io through Trusted Publishing, verifies the
registry bytes, and only then publishes the GitHub Release. A user can
`cargo install netsuke-build --version 0.1.0-beta4` and get a crate the release
run can account for.

The registry upload itself stops depending on a long-lived secret. GitHub mints
a short-lived, repository-scoped token from an OIDC identity; the token exists
only inside the one step that needs it and is revoked by the action's own
post-step. No maintainer rotates it, and no log, cache, or job output can leak
it.

A maintainer watching a tagged release sees one ordered sequence, and can
re-run any part of it without double-publishing or corrupting an
already-published version. A reviewer can read the release run and see, for
every uploaded file, which commit it came from and which digest was expected. A
pull request still rehearses the whole release without a token, without an
environment approval, and without touching any remote state.

## Context and orientation

This section assumes no prior knowledge of the repository.

**The crate.** `Cargo.toml` declares `package.name = "netsuke-build"` with
`[lib] name = "netsuke"` and `[[bin]] name = "netsuke"`. The command users
type, the manual page, and the operating-system packages are all `netsuke`;
only the registry identity is `netsuke-build`. This divergence is decided and
recorded in
`docs/adrs/adr-0007-publish-the-crates-io-package-as-netsuke-build.md`, and it
is why a publisher must never derive a user-facing name from Cargo package
metadata.

**The workspace.** `Cargo.toml` is both the package manifest and the workspace
root; `test_support/` is a second, `publish = false` workspace member. Only
`netsuke-build` may be published. An indiscriminate `cargo publish` with no
`-p` selector would attempt `test_support` and fail, or worse succeed under a
name nobody reviewed.

**The release today.** `.github/workflows/release.yml` is 688 lines on `main` —
grown from the 438 this plan was drafted against, and now well past the
400-line ceiling that `AGENTS.md` sets for code files. It runs on a `v*.*.*`
tag push, and as a reusable workflow (`workflow_call`) with inputs `publish`,
`dry-run`, and `wix-extension-version`, and outputs `bin_name`, `version`,
`should_publish`, `dry_run`, `should_upload_workflow_artifacts`,
`wix_extension_version`.

Its jobs are:

1. `metadata` — resolves the repository name, the release modes through the
   `determine-release-modes` shared action, the crate version through
   `ensure-cargo-version` (with `check-tag` bound to `should-publish`), the WiX
   extension version, and the binary name through `export-cargo-metadata`.
2. `build-linux`, `build-windows`, `build-macos` — each a two-leg matrix
   calling `./.github/workflows/build-and-package.yml`, which compiles, packages
   (`.deb`, `.rpm`, `.msi`, `.pkg`), runs the release-help tooling, and
   uploads workflow artefacts.
3. `windows-native-recipe-smoke` — a native Windows debug build plus
   `./scripts/windows-recipe-smoke.ps1`, skipped on a dry run except for the
   `ready_for_review` event.
4. `release-admission-canaries` — the ADR-020 admission-evidence gate.
5. `release` — the read-only **rehearsal**. It runs only when
   `dry_run == 'true'` (`release.yml:415`), holds `contents: read`, downloads
   the workflow artefacts into `dist/`, hoists the cargo-binstall archives to
   the release root, and calls the shared `upload-release-assets` action at pin
   `a5765019…` with `dry-run: true`. It creates no release and uploads nothing.
6. `publish-release` — added by #834 after this plan was drafted. It runs only
   when `should_publish == 'true'` (`release.yml:548`), holds
   `contents: write`, and creates the draft release
   (`scripts/ensure_draft_release.py` at `release.yml:589`), then runs the same
   download, hoist and upload sequence. It calls the shared
   `upload-release-assets` action at the newer pin `0606ca89…`, so the
   repository now invokes one shared action at two revisions, chosen by job.

The two jobs are mutually exclusive: a dry run takes `release`, a tag push takes
`publish-release`, and a job that needs a skipped job is itself skipped.
Nothing in that path publishes to a registry, and nothing un-drafts the
release: today a human publishes the draft.

**The packaging contract.** `.github/release-staging.toml` names six targets
(`linux-x86_64`, `linux-aarch64`, `windows-x86_64`, `windows-aarch64`,
`macos-x86_64`, `macos-aarch64`), the binary and its extensions, `LICENSE`,
five shell completions, per-platform manual pages or PowerShell help, and the
`cargo-binstall` archive shape. `scripts/hoist_binstall_archives.py` moves the
`{name}-{version}-{target}.tar.gz` archives and their `.sha256` sidecars from
the nested workflow-artefact layout to `dist/` root.

The hoist's original docstring gave the reason as "the shared action namespaces
any file not already at the root". That reason is **false**, not merely
imprecise: the action computes a namespaced name but passes it as `gh`'s
`#label`, and `gh release upload` names the asset from the source file's
basename, so nesting does not survive the upload either way. The observed
consequence is recorded under `Surprises & discoveries`: the 86 candidate files
the beta4 run uploaded collapsed onto 29 published names. The root placement is
still required, for a different reason — the binstall `pkg-url` template
resolves from the release root, and the basename rule means a nested archive
would publish under its last path segment. The hoist's docstring and the
`conftest.py` that describes it were corrected on 2026-10-19; the behaviour is
unchanged and no test asserted the false reason.

**The upload action's real selection rule.** Read from `leynos/shared-actions`
at pin `0606ca899ce2995a0c4bc78e660e80832e6e8855` (the newer of the two pins
`main` now carries; the `release` job still runs
`a5765019912a8ab6882b12db049c7cde635f3a85`), its `_is_candidate` accepts
exactly: a file named `{bin-name}`, `{bin-name}.exe`, or `{bin-name}.1`; any
file whose name ends `.sha256`; and any file whose suffix is `.deb`, `.rpm`,
`.pkg`, or `.msi`. It therefore **rejects** `.tar.gz`, `.crate`, `.json`, and
`.sigstore` bundles. `clobber` defaults to `"true"`, and the action never fails
its own step: a `gh` failure sets `upload_error=true` and `uploaded_count=0`,
which `release.yml`'s `Check asset upload errors` step converts into an exit 1.
The selection rule is unchanged between the two pins, so this plan's
asset-selection findings hold at either revision.

**The dry run.** `.github/workflows/release-dry-run.yml` calls `release.yml`
with `dry-run: true` on `pull_request` events `opened`, `synchronize`,
`reopened`, and `ready_for_review`. It no longer passes `secrets: inherit` —
pull request 833 removed that, and the file now carries a comment explaining
that a called workflow receives `GITHUB_TOKEN` regardless. A `concurrency`
block cancels superseded pull-request runs (pull request 777), and
`pr_concurrency_test.py` guards it.

**How contract tests read these files.** `tests/workflow_contracts/` is a
PyYAML-based suite run by `make test-workflow-contracts`. It parses workflows
with a YAML 1.2 boolean resolver and duplicate-key rejection
(`workflow_loading.py`), follows reusable-workflow calls transitively
(`workflow_call_closure.py`), and drives both parsed values and raw text. A
second, Rust-side suite (`tests/workflow_release.rs`,
`tests/workflow_shared_actions_pins.rs`) asserts raw substrings and full-commit
pins.

**The file-size ceiling.** No source file may exceed 400 lines (`AGENTS.md`,
enforced for Python by Pylint's `max-module-lines = 400`, for Rust by Whitaker's
`module_max_lines`, and by convention for `.github/workflows/*.yml` — see
`docs/developers-guide.md`, which states that each lane's cache steps live in a
composite action *because* the workflow files must stay inside that limit).

**Trusted Publishing.** crates.io Trusted Publishing exchanges a GitHub OIDC
token for a short-lived registry token. Its registration fields are the
repository owner, the repository name, the **workflow filename** (a bare file
name), and an **optional environment**. Two properties of that design drive
this plan: the exchange must therefore happen in a job that lives in
`release.yml` itself, not in a reusable workflow (whose filename would be
presented instead); and the crate must already exist on crates.io, because the
very first publication cannot be trusted-published and needs a manual API
token. That precondition is **already satisfied**: `netsuke-build` is
published, with four unyanked versions — `0.1.0-beta1` (2026-08-05), `beta2`,
`beta3` and `beta4` (2026-09-27, the current `max_version`). No first
publication remains to be done, and the risk recorded at *Risks* for it is
stale; see the corresponding entry in `Surprises & discoveries`.

## Signposts: documentation and skills

Read, in this order, before writing code:

1. `AGENTS.md` — the repository's own agent instructions.
2. `docs/developers-guide.md` — especially the sections on cache ownership,
   the 400-line limit, and workflow contract tests.
3. `docs/repository-layout.md` — where a new script, action, or workflow
   belongs.
4. `docs/documentation-style-guide.md` — the *ExecPlan* section, which is
   authoritative for this file's header.
5. `docs/adrs/adr-0007-publish-the-crates-io-package-as-netsuke-build.md` and
   `docs/adrs/adr-0020-bounded-release-admission-observability.md`.
6. `docs/whitaker-users-guide.md` — `module_max_lines` and
   `no_std_fs_operations`, both of which constrain new Rust and Python.

Skills to load when their subject comes up: `rust-router` before touching
`tests/*.rs`; `python-router` before touching `scripts/*.py` or
`tests/workflow_contracts/*.py`; `firecrawl-mcp` for any external lookup;
`release` only as a checklist reminder, never as publication authority.

## Conformance basis

No technical-design document covers release publication mechanics, so none is
cited for it; this plan does not invent one. A Terms of Reference (ToR) does
now exist — `docs/terms-of-reference.md`, Draft v0.2, added by #786 and absent
when this plan was first drafted. It does not prescribe publication mechanics,
but it does state release-adjacent success criteria and dependencies that this
plan must not contradict: crates.io downloads are named as the first external
adoption signal, and the three release-admission canaries are named as a
dependency. It is cited for those, not for the mechanics. The governing
artefacts are:

```plaintext
AGENTS.md                      -> EP-M1..EP-M6  -> every gate named below
docs/terms-of-reference.md     -> EP-M1, EP-M4   -> 7.3 strategic / 8.3 dependencies
docs/adrs/adr-0007-...              -> EP-M2         -> tests/binstall_metadata_tests.rs
docs/adrs/adr-0020-...              -> EP-M6         -> make test-release-admission
docs/documentation-style-guide.md -> EP-M1      -> tests/execplan_status_contract_tests.rs
docs/developers-guide.md      -> EP-M2, EP-M6   -> docs updates
docs/repository-layout.md     -> EP-M2         -> docs updates
```

PR #834 landed on `main` after this plan was drafted and already supplies
infrastructure this plan must build on rather than duplicate. It is a governing
artefact for EP-M3 to EP-M6:

```plaintext
scripts/release_operation_observability.py       -> EP-M6  -> bounded release telemetry
tests/workflow_contracts/release_publish_path_permissions_test.py -> EP-M3 -> permission separation
tests/workflow_contracts/release_publish_path_mutations.py        -> EP-M3/EP-M5 -> mutation harness
tests/workflow_contracts/release_publish_path_*.py                -> EP-M3..EP-M5 -> publish-path contracts
tests/workflow_contracts/actions_expression_evaluator.py          -> EP-M6  -> expression evaluation
```

External dependencies, cited by revision and treated as axioms. The shared
action now carries two pins on `main`, because `release` and the new
`publish-release` job were bumped separately; this plan must pin whichever it
invokes to the newer of the two:

```plaintext
rust-lang/crates-io-auth-action  c6f97d42243bad5fab37ca0427f495c86d5b1a18  (v1.0.5)
leynos/shared-actions (release job)         a5765019912a8ab6882b12db049c7cde635f3a85
leynos/shared-actions (publish-release job) 0606ca899ce2995a0c4bc78e660e80832e6e8855
```

Cuprum (`leynos/cuprum`) is a motivating context only. Its release design — a
pure-policy module, a thin CLI, and workflow `curl`/`gh` steps as the sole
network boundary — is the shape reused here. Cuprum issues #487 and #499 are
the sources of two gaps this plan closes locally; nothing in this plan closes
them upstream, and no Cuprum test or workflow is modified.

## Constraints

Hard invariants. Violating any of these requires escalation, not a workaround.

1. Do not bump any version, create, push, or move a release tag, publish to
   crates.io, publish a GitHub Release, change repository protections, or
   create or revoke credentials.
2. Exercise publication only through local test doubles and genuinely
   non-publishing rehearsals. A passing mocked test is never evidence that a
   live trusted publish works.
3. The executable and library remain `netsuke`. Do not rename them, and do
   not publish a second crate.
4. Never publish `test_support`, and never run a workspace-wide publish.
   Every publish invocation names `-p netsuke-build`.
5. Do not use `--allow-dirty` to make a tagged release pass.
6. Do not add a Cuprum runtime dependency. Do not copy Cuprum's PyPI
   filename-first identity rule, its cache configuration, its archived build
   trees, or its oversized runners.
7. Do not persist a registry token (`cargo login`, job outputs, artefacts,
   caches, logs), print it, or provide a long-lived-token fallback.
8. Do not restore a build or tool cache from untrusted provenance into a
   privileged publisher.
9. Do not use `pull_request_target`, and do not grant a publisher permission
   to a job that runs candidate code.
10. Do not claim rollback or cross-service atomicity. Do not present an
    attestation of a fresh rebuild as provenance for different retained bytes.
11. Do not weaken a guard, hide a failure with `continue-on-error`, or
    replace substantive execution with a test that only asserts a step name
    exists.
12. Do not treat a duplicate-version error as unconditional success, and do
    not treat `--clobber` alone, or `clobber: false` alone, as reconciliation.
13. Keep every file at or below 400 lines.
14. Every code change follows Red-Green-Refactor, and every commit is gated.

## Tolerances (exception triggers)

Stop and escalate when any of these is breached.

- Scope: more than 30 changed files, or more than 1 500 net changed lines,
  excluding this plan and generated documentation.
- Interface: a change to the `release.yml` `workflow_call` input or output
  contract, or to any existing reusable workflow's interface, beyond adding
  optional inputs with defaults.
- Dependency: any new external action, Python package, or Rust crate that is
  not already pinned in the repository.
- Iterations: three failed attempts at any single gate.
- Time: any one milestone taking more than four hours of wall-clock work.
- Ambiguity: any choice between two materially different asset-inventory or
  reconciliation semantics.
- Environment: any need to actually reach crates.io with a token.

## Risks

- Risk: a mocked OIDC exchange passes while the real one would not, because
  the repository/workflow/environment triple does not match the crates.io
  registration. Severity: high Likelihood: medium Mitigation: make the triple
  an explicit, tested value (`crates-io` environment, workflow filename
  `release.yml`, owner `leynos`, repository `netsuke`); assert it in a contract
  test; and state plainly in the PR and the runbook that no live exchange was
  validated.

- Risk: the registry's crate metadata is not what the release expects, so the
  trusted-publisher exchange or the byte verification fails against a package
  name that is absent, yanked, or owned by another publisher. Severity: medium
  Likelihood: low, now that the package is known to exist. Mitigation: have the
  publisher detect the registry's "crate does not exist" response and fail with
  an actionable message rather than a stack trace, record the trust boundary as
  an item of the maintainer checklist, and reconcile the published bytes
  against the recorded digest as EP-M5 specifies. This entry previously read
  that "the initial publication cannot be trusted-published at all, because the
  package does not yet exist on crates.io", rated severity and likelihood both
  high. That premise is false: `netsuke-build` was first published 2026-08-05
  and has four unyanked versions, so no first publication remains. The entry is
  rescoped rather than deleted, because the failure it names is still reachable
  for a different reason.

- Risk: `cargo publish --dry-run` is slow on the release lane and blows the
  job timeout, as it already does on Windows for the existing packaging test
  (`tests/packaging_smoke_tests.rs` cites 240.8 s median, 265.6 s max).
  Severity: medium Likelihood: medium Mitigation: run the package job on a
  Linux lane with a measured timeout, and allow `--no-verify` only where a
  prior verified build exists for the same digest.

- Risk: a re-run of a tagged workflow double-uploads an asset or attempts to
  re-publish an existing version. Severity: high Likelihood: high Mitigation:
  `concurrency` pinned to `cancel-in-progress: false`, reconciliation that
  inspects remote state before acting, and a digest comparison before any skip.
  A tag re-run therefore QUEUES behind the in-flight release rather than
  cancelling it, which is also the safer semantics for a publication. It cannot
  be made event-dependent: `tests/workflow_contracts/pr_concurrency_test.py`
  pins `RELEASE_CONCURRENCY` by exact equality (`:275-278`) and separately
  rejects any `cancel-in-progress` that `is not False` for a workflow with no
  `pull_request` trigger (`:261-269`).

- Risk: expanding `make github-actions-lint` to cover composite actions
  breaks the existing contract test that pins its recipe, or breaks the `lint`
  target for an unrelated reason. Severity: medium Likelihood: **certain** —
  this is not a risk but a measured constraint. Mitigation: give
  composite-action validation its OWN Makefile target and wire it into `lint`
  alongside `github-actions-lint`. See `Surprises & discoveries`; appending a
  recipe line to `github-actions-lint` is impossible.

- Risk: a new composite action under `.github/actions/` silently violates the
  repository-wide source-build or cache policy, because three existing suites
  glob `**/action.yml` and apply those policies to anything added. Severity:
  medium Likelihood: medium Mitigation: the consumer smoke is specified as a
  digest-checked archive extraction rather than a `cargo install` (see
  `Surprises & discoveries`), and no new action saves a cache. Both are checked
  by running `make test-workflow-contracts` after the first new action lands,
  before more are added.

- Risk: adding jobs to `release.yml` pushes it past 400 lines.
  Severity: medium Likelihood: high Mitigation: move every extracted step body
  into a composite action under `.github/actions/`, leaving only the job graph
  inline.

- Risk: a contract test that enumerates release jobs or runner assignments
  fails after a job is added. Severity: **none** — RETRACTED, this was my
  inference from the contract's shape and it is false. Mitigation: none is
  needed. `DIRECT_RUNNER_SOURCES` is a handwritten tuple of **specific** jobs,
  and `_checked_in_runner_assignments` gathers only those plus matrix-derived
  and reusable-workflow entries; a job absent from both tables is never
  collected, so the exact-equality assertion in
  `has_required_runner_assignments` is satisfied while omitting it entirely.
  The repository's own precedent settles it: `publish-release`, added by PR
  #834, appears in neither table and the gate is green
  (`grep -c 'publish-release'` returns 0 in both files). The ONE real
  registration step is per-job `permissions:` —
  `tests/workflow_contracts/release_publish_path_permissions_test.py:13-24`
  loops over **every** job asserting `"permissions" in job`, and separately
  requires workflow-level `permissions == {}`. `timeout-minutes` is NOT
  required: that suite is parameterized over named Ubicloud jobs, none in
  `release.yml`.

- Risk: the binstall archives the hoist stages are never actually uploaded, so
  the documented `cargo binstall netsuke-build` fetches nothing. Severity: high
  Likelihood: high (it is not a risk — it is a live defect, found on 2026-10-12
  and recorded under `Surprises & discoveries`). Mitigation: this plan's
  `verify-release-inventory` job compares the whole release against the
  identity record, which fails on the missing archives; the maintainer
  checklist records that the defect predates this branch and that the
  `release`/`publish-release` upload call sites are the ones to correct.

- Risk: **`verify-release-inventory` cannot read the draft it must compare.**
  Severity: high Likelihood: certain. GitHub's REST documentation states it
  plainly for the release listing — "Information about published releases are
  available to everyone", while "**Only users with push access will receive
  listings for draft releases**" — and the Actions `GITHUB_TOKEN` is not
  granted push access by `contents: read`. `cli/cli#3037` is the observed form:
  a workflow holding `contents: read; actions: read; security-events: write`
  receives HTTP **403 `Resource not accessible by integration`** when it reads
  a draft. The report was closed in 2021-05 on the theory that a permission was
  missing, reopened in 2021-09, closed again in 2024-06-11, and reported again
  in 2025 by a user who had already set `contents: write`; the accepted answers
  are `contents: write`, a personal access token, or a GitHub App token, and a
  commenter argues `contents: read` is *intentionally* insufficient because
  otherwise a fork could read a draft.

  Mitigation: `verify-release-inventory` holds `contents: write`, which is what
  makes the draft visible to it, and it exercises **only** reads — its whole
  body is a listing, a download, and a local comparison. This is a deviation
  from the 2026-10-15 decision that exactly two jobs hold `contents: write`,
  and it amends that decision rather than contradicting it: what LEM-5 states,
  and what its contract asserts, is the *disjointness* of `id-token: write` from
  `contents: write` and the uniqueness of `environment: crates-io`, and
  neither moves. The writer count was a design choice recorded inside a
  decision entry, not part of the lemma text. The verifier is not a competing
  writer in the sense that motivated the two-writer rule: it never calls a
  mutating API, which the LEM-5 suite now asserts of its composite action, so
  the upload/un-draft separation the count existed to protect is unchanged.
  Date/Author: 2026-10-20, implementation agent.

## Verification plan

The change introduces one central invariant and a small family of lemmas around
it.

**INV-1 (single candidate).** Every file published by a release run — registry
bytes, GitHub release assets, and the identity record — is derived from one
peeled commit SHA, validated against one Cargo package identity.

- Obligation: `INV-1` holds for every publication path.
- Method: parameterized tests over the explicit event/mode matrix, plus a
  property test over generated identity records.
- Rationale: the failure this prevents (publishing bytes from a different
  commit than the one claimed) is a discrete identity error, so exhaustive
  enumeration of the reachable event/mode pairs is proportionate.
- Domain: events `push` (tag), `workflow_call` from a pull request,
  `workflow_call` at `ready_for_review`, `workflow_call` from a branch or
  manual dispatch; modes publish / dry-run.
- Artefact: `tests/workflow_contracts/release_publication_test.py` and
  `scripts/tests/test_release_candidate.py`.
- Evidence: `make test-workflow-contracts` and the `scripts/tests` pytest
  target both pass; before the change, the identity record does not exist.
- Non-vacuity: the mutation module
  `tests/workflow_contracts/release_publication_mutations.py` supplies a
  stale-candidate record, a wrong-digest record, and a record whose tag and
  package version disagree. Each must be rejected; a run over the unmutated
  records must be accepted. The `ready_for_review` row is reachable and is the
  exact row the current skip condition exempts.

**LEM-1 (tag/version agreement).** A `v`-prefixed tag accepts exactly the
package version it spells, prerelease included, with no lossy normalization.

- Obligation: `v0.1.0-beta4` accepts `0.1.0-beta4` and nothing else.
- Method: parameterized table over accept and reject cases, including
  `0.1.0-beta3`, `0.1.0`, `0.1.0-beta4+meta`, `v0.1.0-beta4` and `0.1.0-beta04`.
- Rationale: a finite, enumerable standards corpus.
- Artefact: `scripts/tests/test_release_candidate.py`.
- Evidence: the reject rows fail before the validator exists.
- Non-vacuity: both an accept and a reject row must be present, so a
  validator that always raises and one that never raises both fail.

**LEM-2 (inventory exactness).** The set of files presented for publication
equals the declared inventory, with no missing, extra, empty, duplicate, or
escaping path.

- Obligation: `LEM-2` holds for the six staging targets and the crate
  package.
- Method: parameterized table plus a property test over generated file sets.
- Rationale: the inventory is a finite, explicit manifest; generating
  near-miss sets explores the collision and traversal boundaries cheaply.
- Artefact: `scripts/tests/test_release_inventory.py`.
- Evidence: fuzzed near-miss sets are rejected with a reason naming the
  offending path.
- Non-vacuity: generators must reach the empty-file, duplicate-name,
  `..`-component, and absolute-path classes; a generator that produced only
  well-formed names would make the check vacuous and must be fixed.

**LEM-3 (reconciliation is state-based).** For crates.io, version absence,
identical package, conflicting package, yanked version, and indeterminate error
are five distinct outcomes with four distinct actions; only an identical
checksum permits a skip.

- Obligation: `LEM-3` holds for every classification input.
- Method: parameterized table over recorded registry response bodies, plus a
  property test over generated digests.
- Rationale: the classification is a decision table over a small, closed
  input set.
- Artefact: `scripts/tests/test_release_reconcile_crates.py`.
- Evidence: a duplicate-version response whose digest differs must fail; the
  same response with a matching digest must be a no-op success.
- Non-vacuity: the conflicting case must be reachable, which it is, because
  the digest is generated rather than fixed.

**LEM-4 (GitHub reconciliation never clobbers).** An existing draft asset is
either verified against the trusted digest and skipped, or reported as a
conflict; it is never re-uploaded.

- Obligation: `LEM-4` holds for the draft-reuse path.
- Method: parameterized table over existing-asset states.
- Rationale: small closed set.
- Artefact: `scripts/tests/test_release_reconcile_github.py`.
- Evidence: an existing asset with a mismatched digest must produce a
  conflict and a non-zero exit.
- Non-vacuity: a matching asset must produce a skip, so a reconciler that
  always conflicts also fails.

**LEM-5 (permission separation).** No job holds both a registry credential path
and GitHub release-write authority; the registry-exchange job is the only one
carrying `id-token: write` **and** `environment: crates-io`.

- Obligation: `LEM-5` holds for every job in `release.yml`.
- Method: contract test over parsed jobs.
- Rationale: this is a static property of the file, decidable exactly.
- Artefact: `tests/workflow_contracts/release_permissions_test.py`.
- Evidence: a synthetic job carrying both scopes must be reported.
- Non-vacuity: the check must find at least one job with
  `id-token: write` and at least one job with `contents: write`, or the
  separation assertion would be vacuously true.

**LEM-6 (dry runs publish nothing).** A dry run exchanges no token, creates no
release, uploads no asset, and requires no environment approval.

- Obligation: `LEM-6` holds for every dry-run event.
- Method: contract test over the mode matrix.
- Rationale: the property is a static consequence of the gates, but the
  failure mode is a gate that reads as false while being true.
- Artefact: `tests/workflow_contracts/release_publication_test.py`.
- Evidence: a synthetic workflow that runs the publisher under
  `dry-run: true` must be reported.
- Non-vacuity: at least one job must be skipped on a dry run and at least
  one job must still run, or the assertion is vacuous.

**AXIOM-1.** The `crates-io-auth-action` at pin
`c6f97d42243bad5fab37ca0427f495c86d5b1a18` exchanges an OIDC token for a
registry token on `steps.<id>.outputs.token` and revokes it in a post step.
Verified by reading its `action.yml`. Its internals are not verified here.

**AXIOM-2.** `crates.io` presents the OIDC `job_workflow_ref` of the file that
declares the job, and requires the registered environment name to match
exactly. Consequence: the exchange job must be declared in `release.yml`.

**AXIOM-3.** `cargo publish` with `CARGO_REGISTRY_TOKEN` set, and no `--token`,
uploads the `.crate` that `cargo package` produced for the same source tree,
and rejects a duplicate version with a distinct error.

**AXIOM-4.** `static.crates.io` serves a published crate's `.crate` file
without a token, so post-upload verification needs no credential.

**AXIOM-5.** `gh release upload --clobber` replaces an asset, and the shared
upload action's `_is_candidate` accepts the exact name set given in
`Context and orientation`. Verified from its source at the pin.

**LEM-7 (the smoke installs nothing).** No added workflow or composite action
contains a `cargo install`, so the repository-wide source-build policy stays
discharged.

- Obligation: `LEM-7` holds for every file this change adds or edits.
- Method: reuse the repository's own detector
  (`source_build_data.source_build_commands`) over the concatenated text of the
  changed workflows and actions; it is already a tokenizer rather than a
  substring search, so it does not need reimplementing.
- Rationale: the policy already exists and already globs
  `.github/actions/**/action.yml`; reusing its detector proves the new files
  are covered by the same rule rather than by a parallel one.
- Domain: the full text of `.github/workflows/*.yml` and
  `.github/actions/**/action.yml` after the change.
- Artefact: `tests/workflow_contracts/release_publication_test.py`.
- Evidence: `source_build_commands(...)` returns `[]` over the changed text.
  Before the change, the consumer-smoke prototype that used
  `cargo install netsuke-build` returns a non-empty list and so fails.
- Non-vacuity: the detector is first exercised on a positive control — the
  literal string `cargo install netsuke-build` — which is asserted to be
  *detected*. Without that control, a detector that had been broken or narrowed
  to match nothing would make this lemma pass for the wrong reason.

No non-trivial Rust invariant is introduced: this change touches no Rust source
file, so no Kani harness or Verus proof is owed. That conclusion is recorded
here rather than left implicit.

## Plan of work

Stages, each ending in validation.

- Stage A: this plan only. No code.
- Stage B: red tests. Every new Python module and every new contract test is
  written and run, and fails for its intended reason.
- Stage C: implementation, developed alongside the tests.
- Stage D: wiring — Makefile targets, CI jobs, documentation, and the
  composite-action guard.

### Stage B — the failing suites

New pure modules under `scripts/`, each stdlib-only and under 400 lines:

1. `scripts/release_candidate.py` — `ReleaseIdentity` dataclass and
   `resolve_identity(...)`; validates the tag/version agreement (LEM-1),
   records the peeled commit, package name, version, prerelease flag, binary
   name, expected asset names, and expected digests, and serializes to JSON
   with a stable key order.
2. `scripts/release_inventory.py` — `expected_assets(...)` from
   `.github/release-staging.toml` plus `Cargo.toml`, and
   `validate_inventory(...)` implementing LEM-2: non-empty, unique, relative, no
   `..` component, exact set equality against the manifest.
3. `scripts/release_reconcile_crates.py` — `classify_registry_state(...)`
   returning one of `ABSENT`, `IDENTICAL`, `CONFLICT`, `YANKED`,
   `INDETERMINATE` from a recorded response body and the intended digest
   (LEM-3), and `plan_crates_action(...)`.
4. `scripts/release_reconcile_github.py` — `plan_asset_actions(...)`
   implementing LEM-4.
5. `scripts/release_digest.py` — SHA-256 of a file, sidecar parsing and
   comparison, constant-time compare.
6. `scripts/verify_crates_io_bytes.py` — digest comparison of a downloaded
   `.crate`, given a local path. **This module performs no download.** The
   workflow step performs the request and writes the bytes to a file; the
   module reads that file. Its names predate the split — `download_url` builds
   the URL the workflow step fetches, and `required_headers` returns what that
   step must send — but neither opens a connection, and
   `test_policy_modules_import_no_network_client` in
   `release_publication_test.py` enforces that with a positive control.

Each gains a `scripts/tests/test_<module>.py`. The contract tests gain:

- `tests/workflow_contracts/release_publication_test.py` (INV-1, LEM-6).
- `tests/workflow_contracts/release_publication_mutations.py` (input
  mutations, no tests of its own).
- `tests/workflow_contracts/release_permissions_test.py` (LEM-5). It must land
  in the same commit as the `publish-crates-io` job: two of its tests assert
  that a job *does* carry `id-token: write` and `environment: crates-io`, so
  committing it first leaves `make test-workflow-contracts` red. The other four
  assert the separation itself and pass vacuously until the job exists, which
  is why the existence checks are not optional.
- `tests/workflow_contracts/composite_action_contract_test.py` (new).
- `tests/workflow_contracts/release_modules_wired_test.py` (Cuprum #499) is
  **superseded**: `release_publication_test.py`, landed at `ef02f499`, already
  performs the Makefile and continuous-integration wiring checks plus the
  orphan check this suite was planned to add.

### Stage C — implementation

`.github/actions/` gains one composite action per extracted body:

- `package-crate/` — resolve and record the identity.
- `publish-crates-io/` — probe, decide, upload, verify registry bytes.
- `verify-release-inventory/` — compare the whole release against the
  identity record.
- `finalize-github-release/` — reconcile the draft, then un-draft.
- `consumer-smoke/` — download, verify, extract and run the published bytes.

A sixth action, `upload-declared-assets/`, is called by the existing
`publish-release` job rather than by a new one; the Decision log records why
the upload step becomes a composite action instead of an inline `run` block.

Each new job's name matches the composite action it calls, so a reader moving
between `release.yml` and `.github/actions/` does not have to translate.

**The new actions pass values by environment, never by `${{ }}` in a `run`
block.** `tests/workflow_contracts/release_run_interpolation_test.py` forbids
`${{` inside any `run` in `release.yml` or `package.yml`. When this paragraph
was written its `_step_contexts` skipped every job carrying `uses`, so moving a
step body into a composite action **removed that shell from the contract's
reach entirely** — the extraction this stage performs would have *weakened* the
rule it was written to satisfy. That gap is now closed: the predicate and its
containers live in `tests/workflow_contracts/action_run_sources.py`, and
`composite_action_contract_test.py` applies the identical rule to every
manifest under `.github/actions`, so a job body moved into an action stays
under the contract it was written for. A path-scoped exemption would have
reintroduced the hole for every later action.

Re-measured at `c1f3f2ed`, the five existing actions carry **68** expression
sites between them (`windows-gate-cache` 26, `windows-msi-upgrade-validation`
18, `linux-gate-cache` 17, `kani-cache` 7, `memory-sampler` 0), and **none of
the 68 sits inside a `run` block**. Partitioned by the key that owns each site:
32 under `env:`, 28 under `with:`, and 8 in a manifest's declared `outputs:` —
every one a place where an expression is a substituted value rather than
executable source. The rule therefore extends to **every** manifest under
`.github/actions`, not by path to the five this work adds and not by an
allowlist of known-violating files. An allowlist is the weakening the mission
forbids; a path-scoped rule would have to be extended by hand each time an
action is added, and would leave the next action uncovered until someone
remembered.

Earlier revisions of this plan justified a path-scoped rule by asserting a
blanket extension "would red immediately". That assertion is falsified: it
rested on a stale figure of 84, which counted a `cross-build-macos` action
since deleted (`9e1bf27e`, #798). A blanket sweep is green today, and stays
green because it constrains an invariant the tree already satisfies. The
deliberate negative mutation in
`tests/workflow_contracts/composite_action_contract_test.py` discharges its
non-vacuity, so the stricter rule cannot pass by having nothing to check.

`.github/workflows/release.yml` gains, after the existing `metadata`, `build-*`,
`windows-native-recipe-smoke`, and `release-admission-canaries` jobs:

1. `package-crate` — `needs: metadata`; `contents: read`; ubuntu-latest. The
   order is: resolve candidate -> package -> verify.
2. `publish-crates-io` — the registry exchange. Its dependency list is long
   because the package must be verified against a complete platform build, so
   it is shown as a fence rather than an inline span:

   ```plaintext
   needs: [metadata, package-crate, build-linux, build-windows,
           build-macos, windows-native-recipe-smoke,
           release-admission-canaries]
   if: needs.metadata.outputs.should_publish == 'true'
   environment: crates-io
   permissions: {contents: read, id-token: write}
   ```

3. `verify-release-inventory` —
   `needs: [metadata, publish-crates-io, publish-release]`; `contents: write`;
   `if: … should_publish == 'true'`. The mode gate is stated directly rather
   than inherited. `release` and `publish-release` are mutually exclusive —
   `release` requires `dry_run == 'true'`, `publish-release` requires
   `should_publish == 'true'` — and GitHub skips any job that needs a skipped
   job. Depending on `release` here would make this job unreachable in both
   lanes: on a tag push `release` is skipped, and on a pull-request dry run
   `publish-crates-io` is skipped.

   The scope is `write` because the assets this job downloads belong to a
   **draft** release, and a token needs write scope to see a draft's assets at
   all: with `contents: read` the download returns nothing, the inventory check
   compares an empty directory against a 20-name declaration, and the release
   fails for a reason unrelated to its contents. The job's own operations
   remain read-only — it downloads, hashes and compares — and the scope grants
   reach it does not use for mutation. This is the third member of the writer
   set recorded in the decision dated **Amended 2026-10-20** below: two
   mutating writers plus one read-only holder that needs write scope to see a
   draft. The `verify-release-inventory` composite action is required to
   contain no `gh release` call at all, which is what keeps "holds write scope"
   from drifting into "writes". `id-token: write` stays out, so this job
   remains disjoint from the registry-exchange job's credentials.
4. `finalize-github-release` — `needs: [verify-release-inventory]`;
   `contents: write`, no `id-token`. Named `finalize-` rather than
   `publish-github-` to keep it distinct from the existing `publish-release`,
   which it follows rather than replaces.
5. `consumer-smoke` — `needs: [finalize-github-release]`; verifies the
   published bytes without compiling anything (see `Surprises & discoveries`:
   `cargo install` is forbidden in CI by policy). It fetches
   `https://static.crates.io/crates/netsuke-build/netsuke-build-<version>.crate`,
   checks its SHA-256 against the staged digest, then downloads the published
   cargo-binstall archive by its public release URL, verifies it against its
   checksum sidecar, extracts it, and runs the shipped `netsuke --version`.
   That exercises the real public-consumer download path — the one
   `cargo-binstall` takes for a user — and executes the shipped bytes.

`publish-release` — not `release` — is the job holding `contents: write` and
running the draft-creation steps; `release` holds `contents: read` and runs the
dry-run rehearsal. `publish-release`'s `needs` grows `package-crate`, so the
draft is not created before a package exists. Stage C as first drafted
attributed the write permission to `release`; that predates PR #834, which
split rehearsal from publication.

`concurrency.cancel-in-progress` stays **`false`** and is **not** made a
function of the event. `pr_concurrency_test.py:275-278` pins
`RELEASE_CONCURRENCY` by exact equality, and `:261-269` independently rejects
any `cancel-in-progress` that `is not False` for a workflow with no
`pull_request` trigger. A tag re-run therefore QUEUES behind the in-flight
release rather than cancelling it, which is the safer semantics for a
publication anyway: cancelling mid-upload is the failure mode this whole design
exists to prevent.

Each new job's body is a single `uses: ./.github/actions/...` step. The
justification for extracting them is that the composite-action convention is
the repository's own documented practice (`docs/developers-guide.md:1574-1575`
records it for the cache steps), **not** that a contract enforces a 400-line
cap on workflow YAML — no such contract exists. `release.yml` is 688 lines on
`main` and is not failing anything.

### Stage D — wiring and documentation

- `Makefile`: a `test-release-publication` target running the new
  `scripts/tests` modules with pinned `pytest` and `hypothesis`, and a
  `test-release-negative-mutation` target.
- `Makefile`: a **new, separate** target validates
  `.github/actions/*/action.yml` — yamllint with the same policy, plus a
  `bash -n`-based syntax check over each composite action's `run` blocks — and
  is wired into `lint` alongside `github-actions-lint`. It cannot be appended to
  `github-actions-lint` itself; see `Surprises & discoveries` for the
  positional pin that forbids it. The two pre-existing defects in the existing
  five manifests — `windows-msi-upgrade-validation/action.yml` missing its
  `---` document start, and `windows-gate-cache/action.yml` carrying a trailing
  blank line — were repaired ahead of this target, in commit `b07de6038`, so
  its first run is green on the threshold and any red it reports is
  attributable to the manifests this branch adds. Both findings and both
  repairs are recorded in `Surprises & discoveries`; neither file is otherwise
  touched by this branch, and neither repair adds an exemption.
- `.github/workflows/ci.yml`: run the new targets in `build-test`.
- `docs/developers-guide.md`: the release runbook — job graph, package and
  asset identity contract, provenance verification, re-run procedure, and
  partial-publication recovery.
- `docs/contents.md` and `docs/repository-layout.md` if a new workflow file
  is added.
- `docs/adrs/adr-0007-publish-the-crates-io-package-as-netsuke-build.md`: an
  addendum naming the publisher.

## Milestones and plateaus

**EP-M1 — the plan.**

- Outcome: an approved, self-contained plan.
- Requirements: none discharged.
- Acceptance evidence: this file passes
  `tests/execplan_status_contract_tests.rs` and `make markdownlint`.
- Conformance check: one bare `Status:` line; mandatory sections present.
- Recovery: revert the commit.
- Remaining gaps: everything below.
- Compatibility decision: none.

**EP-M2 — pure policy modules and their tests.**

- Outcome: seven pure modules under `scripts/`, each with a focused suite, all
  runnable locally without network access. The plan first named six;
  `release_semver` was added during Stage B to hold the version grammar the
  identity resolver and the inventory both consult. Stage C added two more
  under the same discipline — `release_digest_record` for the trusted record's
  shape and `release_staged_digests` for hashing the staged tree — so the
  milestone's count is seven and the tree's is nine.
- Requirements: LEM-1, LEM-2, LEM-3, LEM-4 discharged.
- Acceptance evidence: `make test-release-publication` passes; each module's
  reject rows fail before the module exists.
- Conformance check: no module exceeds 400 lines; no module imports a network
  client, asserted by `test_policy_modules_import_no_network_client` over every
  `SOURCE_MODULES` entry (with a positive control, so the check cannot pass by
  matching nothing); Ruff, Pylint, Interrogate, and ty pass.
- Recovery: the modules are additive; revert the commit.
- Remaining gaps: no workflow wiring yet.
- Compatibility decision: none; these are new private modules.

**EP-M3 — the GitHub-side publication sequence.**

- Outcome: `release.yml` carries the full sequence, every new job body
  extracted to a composite action so the job graph stays readable at a glance.
  (Not "under 400 lines": no contract caps workflow YAML, and `release.yml` is
  already 688 lines on `main`. The extraction follows the repository's
  documented composite-action convention, `docs/developers-guide.md:1574-1575`.)
- Requirements: INV-1, LEM-5, LEM-6 discharged.
- Acceptance evidence: `make test-workflow-contracts` passes, including the
  new permission and mode-matrix tests.
- Conformance check: the six existing outputs are unchanged; the three
  `workflow_call` inputs are unchanged.
- Recovery: the sequence is additive after `release`; a revert restores the
  current behaviour exactly.
- Remaining gaps: no crate is published by any run, because the trusted
  publisher's registration is a maintainer action. The package already exists
  on crates.io (`0.1.0-beta1` through `beta4`), so no initial publication is
  outstanding; only the registration and the account settings are.
- Compatibility decision: the reusable interface is preserved because
  `release-dry-run.yml` and the repository's release runbook depend on it.

**EP-M4 — the crate package and the registry publisher.**

- Outcome: `package-crate` and `publish-crates-io` exist, with the ephemeral
  token confined to one step.
- Requirements: AXIOM-1, AXIOM-2, INV-1.
- Acceptance evidence: the dry run produces a `.crate`, its digest, and its
  file list, and reaches no network write.
- Conformance check: no long-lived token anywhere; `persist-credentials: false`
  on the publisher's checkout; no cache restore in the publisher.
- Recovery: both jobs are gated on `should_publish`, so a revert or a skip
  leaves the current release path intact.
- Remaining gaps: the live exchange is unvalidated by construction.
- Compatibility decision: none.

**EP-M5 — recoverability and reconciliation.**

- Outcome: re-running a tag run reconciles rather than duplicating; the
  finalizer completes a matching release and fails closed otherwise.
- Requirements: LEM-3, LEM-4.
- Acceptance evidence: a simulated second run against a partially populated
  release reports exactly the missing assets and uploads only those.
- Conformance check: `cancel-in-progress` is `false` for a tag; no
  `--clobber` is used for reconciliation.
- Recovery: a failed run leaves the draft and the published version intact
  and is safe to re-run.
- Remaining gaps: no cross-service atomicity is claimed.
- Compatibility decision: none.

**EP-M6 — gates, guards, and documentation.**

- Outcome: a deliberate negative mutation is caught by the same route CI
  runs; composite actions are linted; no new test is orphaned.
- Requirements: Cuprum #487 and #499 closed locally.
- Acceptance evidence: `make test-release-negative-mutation` fails with the
  injected mutation and passes without it; the new composite-action target
  covers `.github/actions` and `make lint` runs it.
- Conformance check: the full gate set passes sequentially.
- Recovery: revert.
- Remaining gaps: none known.

## Concrete steps

Run everything from the worktree root,
`/home/leynos/.lody/repos/github---leynos---netsuke/worktrees/cb816962-5a41-4818-baff-47414fe83884`.

Focused Python suites (fast, no network):

```plaintext
make test-release-publication 2>&1 | tee /tmp/test-release-publication-use-trusted-packaging.out
make test-workflow-contracts   2>&1 | tee /tmp/test-workflow-contracts-use-trusted-packaging.out
```

Expected: the first reports the new modules' pass counts; the second reports
the whole contract suite green.

The negative mutation, which must fail:

```plaintext
make test-release-negative-mutation 2>&1 | tee /tmp/test-release-negative-mutation-use-trusted-packaging.out
```

Expected transcript, in outline:

```plaintext
control run: 42 passed
mutated run: 3 failed
OK: the mutation was caught by scripts/tests/test_release_reconcile_crates.py
```

YAML and shell lint:

```plaintext
make github-actions-lint 2>&1 | tee /tmp/lint-github-actions-use-trusted-packaging.out
```

Full gates, sequentially and only through `scrutineer`:

```plaintext
make check-fmt
make lint
make test
make doc-coverage
make test-workflow-contracts
make test-release-admission
```

## Validation and acceptance

A reviewer accepts this change when all of the following hold.

1. `make test-workflow-contracts` passes and the suite reports at least three
   new test modules.
2. `make test-release-publication` passes and the suite reports at least six
   new test modules.
3. `make test-release-negative-mutation` exits non-zero when a mutation is
   injected and zero when it is not; the control run is part of the same
   command, so neither half can pass alone.
4. `make github-actions-lint` lints `.github/workflows` with yamllint and
   actionlint, **unchanged in shape**; the new composite-action target lints
   `.github/actions/*/action.yml` with yamllint and the shell-syntax check, and
   `make lint` runs both. These are two targets, not one: the existing recipe's
   first and last lines are pinned verbatim by
   `github_actions_validation_test.py:218-227`.
5. `wc -l .github/workflows/release.yml` reports a value at or below 400.
   **Unsatisfiable as written.** The file is 688 lines on `main` before this
   change, and no repository contract enforces a 400-line cap on workflow YAML:
   `ci.yml` (642) and `build-and-package.yml` (441) exceed it too. The premise
   behind this criterion — that the cap is enforced on workflow files and that
   only the new jobs stand between the file and compliance — is false. It
   cannot be discharged by this plan's scope, which does not restructure the
   existing `metadata`, `build-*`, `release-admission-canaries`, `release`, or
   `publish-release` bodies. See `Decision log`; the criterion is retained
   unedited so the deviation stays visible rather than being redefined away.
6. `make lint` and `make test` pass unchanged.
7. A dry-run rehearsal on a pull request runs `package-crate` to completion
   and skips every publishing job, with no environment approval requested.

Red-Green-Refactor evidence: each Stage B module is committed with its test
failing; each Stage C implementation is committed with that test passing; each
Stage D wiring change is committed after the gate that covers it passes.

## Idempotence and recovery

Every step is re-runnable. The publisher inspects registry state before acting,
so a second run over an already-published version with a matching digest is a
no-op success, and a second run with a differing digest fails closed. The
GitHub finalizer inspects the draft and **refuses** rather than repairing: it
plans an action per declared asset, reports `EXIT_UPLOAD_REQUIRED` when one is
missing and `EXIT_CONFLICT` when one disagrees, and only un-drafts when every
asset is already the declared one. Its own upload path was removed by the
2026-10-19 Decision entry, because a job that both verified and wrote could not
report a missing asset as a failure. Nothing in this change deletes a remote
object.

Local recovery: `git switch --detach ebcedaef` restores the pre-change tree.
Re-running a release is a maintainer action and is documented, not performed
here.

## Artefacts and notes

The plan file itself is the artefact for EP-M1. Later milestones append the
observed test counts and the digest comparisons here.

## Interfaces and dependencies

New Python surface, all private to the repository:

```python
# scripts/release_candidate.py
@dataclasses.dataclass(frozen=True)
class ReleaseIdentity:
    commit: str
    package: str
    version: str
    is_prerelease: bool
    bin_name: str
    assets: tuple[str, ...]
    digests: cabc.Mapping[str, str]

def resolve_identity(*, tag: str, commit: str, package: str,
                     version: str, bin_name: str) -> ReleaseIdentity: ...

def validate_tag_against_version(tag: str, version: str) -> None:
    """Raise TagVersionMismatch when the tag does not spell the version."""
```

```python
# scripts/release_reconcile_crates.py
class RegistryState(enum.Enum):
    ABSENT = "absent"
    IDENTICAL = "identical"
    CONFLICT = "conflict"
    YANKED = "yanked"
    INDETERMINATE = "indeterminate"

def classify_registry_state(body: str | None, *, intended_sha256: str,
                            http_status: int) -> RegistryState: ...
```

```python
# scripts/release_reconcile_github.py
class AssetAction(enum.Enum):
    UPLOAD = "upload"
    SKIP_VERIFIED = "skip-verified"
    CONFLICT = "conflict"

def plan_asset_actions(*, expected: cabc.Mapping[str, str],
                       present: cabc.Mapping[str, str]) -> tuple[tuple[str, AssetAction], ...]: ...
```

Dependencies: none new. `rust-lang/crates-io-auth-action` at
`c6f97d42243bad5fab37ca0427f495c86d5b1a18` is the only new external action.

## Progress

- [x] (2026-09-26) Reconnaissance: read `release.yml`,
  `release-dry-run.yml`, `release-staging.toml`, the shared upload action's
  source at its pin, and every workflow-contract suite that constrains
  `release.yml`.
- [x] (2026-09-26) Resolved the `crates-io-auth-action` pin and read its
  `action.yml`.
- [x] (2026-09-27) Drafted this plan.
- [x] (2026-09-27) Verified the plan's load-bearing claims against the source:
  the runner-assignment equality, the three suites pinning the job name
  `release`, and the repository-wide `cargo install` prohibition now covering
  composite actions. Corrected the consumer-smoke design and recorded the
  finding in `Surprises & discoveries`, `Risks`, and `Verification plan`.
- [x] (2026-10-11) Rebased onto `main` at `557966c8`. One conflict-free
  replay; the `typos.toml` refresh became empty and was skipped (see
  `Surprises & discoveries`), leaving the branch a single commit, `0132d0c1`.
  Corrected four stale figures in this plan against the rebased tree, cited the
  Terms of Reference added by #786, and recorded the #834 collision.
- [x] (2026-10-11) Ran the rebased tree through the four validation gates at
  the frozen head `6483aa7c`: `check-fmt`, `typecheck`, `lint`, and
  `test-workflow-contracts` all exit zero, and `nixie` and `markdownlint` pass.
  `make test` is red on two *different* nested-Cargo tests across two runs,
  each by the 300 s per-test allowance rather than by an assertion; both are
  proven environmental. See `Surprises & discoveries`.
- [x] (2026-10-12) Approval granted (EP-M1). The `sdlc-implementation` skill
  was invoked for this plan; its own contract is that an explicit instruction
  to proceed with the plan supplies execution approval and is not re-requested.
- [x] (2026-10-12) Reconnaissance for Stage B and Stage C, read from the tree
  rather than from this plan. Established the exact contract surfaces any new
  job must satisfy (see `Surprises & discoveries`), and found that criterion 5
  of `Validation and acceptance` is unsatisfiable as written and that the
  2026-10-11 decision to host the crates.io exchange inside `publish-release`
  contradicts LEM-5. Both are recorded in `Decision log`; neither blocks Stage
  B, which touches no workflow file.
- [x] Stage B: pure modules and their failing suites (EP-M2). LEM-1 through
  LEM-4 are complete and wired: `release_candidate.py`, `release_inventory.py`,
  `release_semver.py`, `release_digest.py`, `verify_crates_io_bytes.py`,
  `release_reconcile_crates.py` and `release_reconcile_github.py` each have
  their suite, and `release_publication_test.py` pins every module and suite
  name in the `test-release-publication` recipe so a new module cannot land
  unwired.
- [x] (2026-10-12) LEM-2 landed as `scripts/release_inventory.py` (397 lines,
  stdlib-only) with `scripts/tests/test_release_inventory.py`, 22 tests. The
  rule is exact set equality against the declared set, not a per-file
  acceptability check: the weaker rule accepts a release that is merely
  *missing* an asset, which is the live binstall defect recorded under
  `Surprises & discoveries`, and the `a-binstall-archive` row is that release's
  verdict written down as a test. Archive names are delegated to
  `hoist_binstall_discovery.expected_archive_names` rather than restated, so
  the inventory and the hoist cannot drift about what an archive is called. Two
  acceptance rows and two Hypothesis witnesses make the predicate two-sided,
  and a `FrozenInstanceError` row pins the entry as immutable.
- [x] (2026-10-12) Closed the orphan window the plan's EP-M6 requires. The
  LEM-1 suite had landed with no selector collecting it — `make test` runs the
  Rust suite only — which is precisely the Cuprum #499 failure this plan exists
  to avoid, so committing it unwired would have shipped the defect the work is
  meant to prevent. Added the `test-release-publication` Makefile target
  (mirroring `test-linux-package-metadata`'s pinned pytest/hypothesis and
  isolated-configuration shape), declared it phony, and added the
  `Release publication tests` step to `ci.yml`'s `build-test` after the
  workflow-contract gate.
  `tests/workflow_contracts/release_publication_test.py` (9 tests) pins all of
  that, and its orphan check — every `scripts/tests` module must be named by
  some recipe — was probed live: it fails naming a deliberately uncoupled
  module and passes once it is removed. Verified before writing it that all 20
  existing modules were already wired, so the guard is green at introduction
  rather than failing for an unrelated reason.
- [x] (2026-10-12) Ran the focused suites and then the full
  `test-workflow-contracts` gate on the changed tree: 118 passed for the three
  publication suites, and 1224 passed with 3 skipped (pre-existing) for the
  contract suite after both the Makefile and `ci.yml` changed.
- [x] (2026-10-12) LEM-1 landed as `scripts/release_candidate.py` and
  `scripts/release_semver.py`, with `scripts/tests/test_release_candidate.py`
  and `scripts/tests/test_release_semver.py` (96 tests, 7 doctests when they
  were later wired to run — see the correction below). Both modules are
  stdlib-only and under the 400-line cap (394 and 139). Three seeded mutations
  are each caught by multiple rows, and the identity record's both directions —
  accept a well-formed version, refuse a near-match — are witnessed. Two
  findings were recorded while doing it: the first version of the leading-zero
  rule was stated too broadly and refused a version Cargo accepts (see
  `Surprises & discoveries`), and the `ambrleaks` stage of `make lint-python`
  reports findings for an explicitly named `.py` file while passing over the
  same file reached through a directory walk.
- [x] (2026-10-15) Corrected an inherited overstatement and repaired its cause.
  The "7 doctests" above were counted from the source files, not from a run:
  nothing collected them, because the recipe carried no `--doctest-modules` and
  named no source module. The recipe now does both, the wiring is pinned by
  `SOURCE_MODULES` and a new parametrized contract test, and the repair was
  proved live by mutation rather than by asserting the flag's presence.
  Measured effect on the target: **118 → 126 passed**, the difference being
  exactly the eight source-module doctests (4 in `release_candidate.py`, 1 in
  `release_inventory.py`, 3 in `release_semver.py`). `release_inventory.py`
  sits at exactly 400 lines after this, the inclusive cap boundary.
- [x] (2026-10-15) Ran the full nine-gate batch against `ef02f499` and read
  every log to a terminal verdict: **eight green, one environmental timeout,
  and no branch failure**. `check-fmt`, `lint`, `typecheck`, `doc-coverage`,
  `markdownlint` (with the spelling gate it inherits), `nixie`,
  `test-workflow-contracts` (27 passed) and `test-release-publication` (126
  passed) all pass. `make test` fails only on
  `netsuke-build::command_env_ui_tests cli_configuration_fixture_compiles`, a
  `TIMEOUT [300.020s]` with **no `FAIL` line anywhere in the log**: 3903 of
  3904 run tests reported `passed`, 6 skipped, and the doctest pass reports
  `ok … 0 failed`. The timeout is contention, not the branch — the diagnostic
  is a count, `grep -c "sccache server is unreachable"` = **573** against a
  healthy 1, identical on two independent runs of this commit, and this is the
  measurement the existing tolerance names. The tolerance is therefore used as
  written and **not** widened. doc-comment coverage closed at **98.87%**
  (4972/5029) against the 80% threshold.
- [x] (2026-10-15) Proved the `--doctest-modules` repair by observed mutation
  rather than by the earlier derivation. With the line
  `-p no:cacheprovider --doctest-modules` reduced to `-p no:cacheprovider` **in
  that recipe alone**, `test-workflow-contracts` reports `1 failed, 11 passed`;
  restored, it reports `12 passed`. The failing case is
  `test_the_target_recipe_pins_its_interpreter_and_modules`. The earlier entry
  above was written from source counting and is superseded by this observation;
  the tree was confirmed clean on both sides of the mutation.
- [x] (2026-10-15) Observed the LEM-5 suite red for the *right* reason before
  removing it. Run against the current `release.yml` it reports
  `2 failed, 6 passed`: the failures are the two assertions that require
  `publish-crates-io` — a job that **does not exist yet**, because it is Stage
  C work. Red for absence of the object under test is the intended Red stage,
  not a defect. The suite is drafted and held outside the tree until Stage C
  can land it in the same commit as the job it constrains, so the orphan guard
  is never asked to tolerate an unwired module.
- [x] (2026-10-15) Landed the last four Stage B modules and their four suites
  in one commit: `scripts/release_digest.py`, `verify_crates_io_bytes.py`,
  `release_reconcile_crates.py`, `release_reconcile_github.py` and their
  `scripts/tests/` suites, with `release_publication_test.py`'s
  `SECTION_MODULES`, `SOURCE_MODULES` and the `test-release-publication` recipe
  extended in the same commit. Measured on the committed content: **126 → 212
  passed**, the increase consisting of the four new suites' test functions plus
  **13 source-module doctest items** (4 `release_digest`, 8
  `verify_crates_io_bytes`, 2 `release_reconcile_crates`, 3
  `release_reconcile_github` — the last two are my run's counts, and the
  per-module collection is recorded in the batch report). All eight modules sit
  under the 400-line cap, the largest at 396. Acceptance criterion 2's "at
  least six new test modules" is now **satisfied as enumerated**: the target
  collects six `scripts/tests/` modules added by this work, not counting the
  three that predate it. A pedantic reading would require six modules *added by
  the target*, which has collected nine since `ef02f499`, and is unachievable.
  The reading chose the one that is satisfiable and is stated here rather than
  left implicit.
- [x] (2026-10-15) Cleared the Stage B Python diagnostics (`51ab8275`). The
  Stage B commit was gated on `make test-release-publication` alone, so it
  passed 212 tests while `make lint` failed on 13 Ruff diagnostics in the
  modules it added. Repairing those exposed 2 Pylint and 53 df12 house-lint
  findings that Ruff had been masking: `make lint` stops at its first failing
  stage, so a single run's problem count is a **lower bound**, never a total.
  68 diagnostics in files that had never been linted at the 3.14 baseline. All
  repairs are behavioural no-ops; an AST comparison of assertion expressions
  against `118b8c9c` shows identical expressions everywhere except the two
  comparison simplifications the house lints required. Measured:
  `make lint-python` exits 0 through all five stages and
  `make test-release-publication` still reports 212 passed.
- [x] (2026-10-15) Rebased onto `main` (`486b876c`), finishing at `c09e9e99`.
  Nine commits, zero conflicts. Performed BEFORE Stage C because the
  `local_reference_*` family (11 modules) exists only on `main` and constrains
  exactly the composite actions Stage C adds. Two predictions confirmed: the
  dead `.github/actions/cross-build-macos` action — present at the old base
  `557966c8`, deleted by `9e1bf27e` (#798, "Remove dead release code and stop
  tests reading GoReleaser YAML", PR #859) before the new base — is gone, and
  the coverage probe's uncovered count fell from 1 to 0 (inventory 71 → 85).
  The branch still changes zero `.rs` files.

  The `range-diff` for this replay is **not** empty, and two later citations of
  it were wrong. Replayed against the range that existed when the sentence was
  first written (`557966c8..0440bfaa` versus `486b876c..c09e9e99`) all nine
  commits read `=`, but only because that range had been **amended** that
  afternoon; the replayed range is `486b876c..7339ed3c`, and against the
  *pre-amend* old range (`557966c8..f537009f`, three commits) `range-diff`
  reports `-: -------- > 4: …` through `> 11: …` — eight commits exist only on
  the new side. That is an artefact of comparing a range that grew before the
  replay against one that grew after it, not a replayed patch that changed, and
  the correct comparison for "did the replay alter the patches" is the amended
  range. Both figures a later entry and the decision log quote are now
  corrected to the post-amend range.
- [x] (2026-10-15) Rebased a second time, onto `fa5a4ffa`, finishing at
  `565d5572` (the current head). Trigger: a stop-hook instruction to rebase onto
  `origin/main` and re-run `check-fmt`, `test`, `typecheck` and `lint` before
  a leased force push. Eleven commits, zero conflicts; every commit replayed
  byte-identically (`range-diff fa5a4ffa..565d5572` against
  `486b876c..7339ed3c` is all `=`, eleven lines). Audited rather than assumed:
  the diffstat is identical on both sides — `fa5a4ffa..565d5572` and
  `486b876c..7339ed3c` each report
  `18 files changed, 5679 insertions(+), 1 deletion(-)`, and
  `17 files / 3996 insertions(+) / 1 deletion(-)` excluding this plan, so the
  delta was **base movement, not patch movement**; `git diff --check` is clean;
  and `git diff 7339ed3c 565d5572` is 38 inherited files, 6071 insertions, 44
  deletions, every one of them upstream work this branch never touched — with a
  single, benign exception, the `Makefile`, which both sides edit and which a
  raw path-intersection test flags for that reason alone. The two edits do not
  overlap: upstream moved `PROVER_TOOLS_SOURCE` from
  `rust-prover-tools@b07ef696` to `@2ba17da6` at the pin line, while this
  branch adds its `test-release-publication` recipe elsewhere in the file, so
  both survive at the new base. Recovery refs
  `recovery/trusted-packaging/OLD_HEAD-7339ed3c` and
  `recovery/trusted-packaging/TARGET-fa5a4ffa` are held. The branch is 0 behind
  and 11 ahead of `origin/main`, which still carries `local_reference_*` (11
  modules) so the ordering constraint above still holds. **Not yet done at this
  head: the four re-run gates and the force push** — the pre-rebase batch
  belonged to `7339ed3c`, and a rebased head is a new candidate whose gate
  evidence is not inherited.
- [x] (2026-10-15) Corrected five false premises in this plan after checking
  each against source rather than against a contract's shape. Retracted: the
  claim that new `release.yml` jobs need two-place runner registration (the
  gatherer never collects an unregistered job, so it is invisible, not failing —
  `publish-release` on `main` proves it); the event-dependent
  `cancel-in-progress` design (pinned by exact equality); and the
  `github-actions-lint` extension (recipe positionally pinned). Corrected: the
  composite-action rationale is the repository's documented convention, not a
  400-line cap that no contract enforces. Recorded the scope-tolerance breach
  (3 995 net lines excluding this plan against a 1 500 budget) as a deviation
  in prose.
- [x] (2026-10-15) Cleared three self-inflicted Markdown-gate failures in this
  plan. Two were mine and visible (`mdtablefix` wanted a reflow; `typos` read
  `hand-written` as a misspelling of `handwritten`), and the third —
  `markdownlint` MD040 on three unlabelled fences — was **masked**: it never
  ran, because `markdownlint` depends on `spelling` and the prerequisite
  aborted the target. Re-measured the tolerance figures rather than trusting
  the first reading: the all-paths value had been carried forward as `5495`
  from an earlier head and is `5679` at this one, and the passage now leads
  with the figure **excluding** this plan, which is the one the tolerance turns
  on and the only one that does not move when the plan is revised to record it.
  No formatter caught this one: `check-fmt` passed at `7339ed3c` both before
  and after the figure was corrected, because a stale figure is a documentation
  defect and not a gate input whatsoever. It was found by a provenance check on
  the SHA the figure cited. Whether `check-fmt` still passes is now a question
  for the re-run owed at `565d5572`.
- [x] (2026-10-15) Ran the full commit-gate batch at `c1f3f2ed`, the first
  batch against the rebased head: nine of ten targets exit zero (`check-fmt`,
  `spelling`, `markdownlint`, `nixie`, `lint` 59 m, `typecheck` 8 m,
  `doc-coverage` 98.89% against an 80% threshold, `test-workflow-contracts`
  1291 + 27 passed, `test-release-publication` 212 passed). `make test` exits 2
  on one nested-Cargo trybuild test terminated at its 300 s cap with **no
  `FAIL` line anywhere in the log**, its two siblings having passed at 272.487
  s and 240.973 s against the same cap. The branch changes no Rust, so no
  mechanism exists by which it slows a `netsuke-build` compile;
  `build-limits-status` reported capacity 4 with all 4 held and peer waiters
  queued 83.8 s, and the gate's own log carries 573
  `sccache server is unreachable` lines, so every compile ran uncached. The red
  is recorded as **environmental, not a branch defect** — and as attempt one of
  the `Tolerances` limit of three. The `cargo test --doc` half of `make test`
  never ran, because the nextest abort stopped the target before reaching it;
  doctests at this revision are **unobserved evidence**, not a pass.
- [x] (2026-10-16) Ran the full batch again at `ce5ff20f`. **One real defect,
  and it was mine:** `spelling` exits 2 on a hyphenated word at line 1166 whose
  trailing element the shared dictionary reads as a misspelling of "emptied".
  The word entered in this candidate's own Progress entry and is absent at
  `c1f3f2ed`. Repaired by rewording to "stopped the target before reaching it"
  rather than by adding a `typos.local.toml` exception, since the exception
  would be a standing tolerance for a phrase nobody needs. This entry therefore
  describes the defect without reproducing either spelling, so that the record
  of the repair cannot itself re-trigger the gate it repairs. `markdownlint`
  exits 2 on the same run with a **byte-identical log** — it never ran, because
  it depends on `spelling` and the prerequisite aborted it, so its red is
  masked, not independent; the same masking is already recorded in Surprises.
  `make test` exits 2 again: **0 `FAIL` lines**, 3969 passed, 1 timed out, at
  the same slot (3970/3976) as the previous run.

  That second timeout is stronger evidence than the first, and it changes the
  disposition. The timed-out test is
  `locale_stub_ui_tests::split_build_fixture_compiles_through_the_direct_rustc_harness`.
  In `.config/nextest.toml` it sits in an `[[profile.default.overrides]]` block
  (`:98-101`) alongside `production_build_module_slice_has_expected_boundary`
  and four others; that block sets only `test-group = "nested-cargo-builds"` and
  `success-output = "immediate"`, **no** `slow-timeout`, so the test keeps the
  profile's own five periods. The same file's argument for leaving such a test
  on that allowance is written against its *sibling*
  `cli_configuration_fixture_compiles` (`:57-63`), whose child `cargo check`
  runs in the same four-slot compiler-admission pool: its wall clock "is the
  queue's to decide rather than the test's". `docs/developers-guide.md`
  (`:9431`) records the timed-out test itself only as retaining "a small
  isolated nested build to cover the complete Cargo-to-rustc boundary". What
  both share is the pool, and so the exposure. Both runs hit the cap at 300.007
  s and 300.010 s, 0.003 s apart, because `period = "60s", terminate-after = 5`
  is arithmetic and not a measurement. `make test` is therefore adjudicated
  **environmental for the second time and at the same site**, and is **not**
  evidence against this branch. The doctest half again did not run — for the
  same reason, and it remains **unobserved evidence**, not a pass. That is
  attempt two of the `Tolerances` limit of three for the `test` target
  specifically; the `spelling` red is a different target, and it is repaired
  rather than tolerated.
- [x] (2026-10-16) Plan maintenance before the re-gate, in two parts. First,
  the Progress entry above that recorded the `spelling` defect **reproduced the
  offending token** while describing it, so the record of the repair would have
  re-triggered the gate it repairs; it now describes the defect without either
  spelling. Second, the 2026-10-12 OIDC-separation decision cited the older
  permissions test as stating that only publication holds `contents: write`,
  which reads as a violated constraint now that a second writer is decided
  (`finalize-github-release`); it is qualified to say that the intent governs
  the GitHub publication path, that the test asserts per-job scopes and never
  set-equality over holders, and that Stage C must extend or rename it so its
  name does not outlive its truth. Both edits changed whitespace only, as
  re-formatted by `mdtablefix`: `--check` reports 184 files unchanged, the
  token stream is identical to the pre-format copy, and the line-initial
  ordered-list marker census is 44 before and after.
- [x] (2026-10-16) Ran the full ten-gate batch at `048b49f8`. **The spelling
  gate reded a second time, and again the defect was this branch's own prose
  about its own repair.** The previous entry, which documents a spelling
  failure and the rewording that fixed it, added a fresh `-ise` form while
  describing the re-formatting, and the en-GB-oxendict dictionary requires the
  `-ize` spelling of it. So the same gate failed twice in consecutive
  candidates by two mechanisms: first the entry quoted the offending token,
  then it introduced a new one. This is the *same defect class* recorded in
  `docs/execplans/6-1-1-split-rfc-0006-set-into-focused-child-rfcs-and-task.md`
  at its own third instance, and that plan's answer applies unchanged: a
  repo-wide `-ise` sweep is **not** a safe repair, because a backticked
  quotation of another revision's gate output must keep its original spelling
  or it becomes a falsified record. This plan holds no such quotation —
  checked, and none exists — so the single plain-prose occurrence is the whole
  repair, and it now reads "changed whitespace only, as re-formatted by
  `mdtablefix`". `markdownlint` again exits 2 with a **byte-identical log**, so
  `markdownlint-cli2` never ran and has **no verdict**; that is an unknown, not
  a pass. `typos.toml` was not modified this run (tree clean afterwards).

  **The four Rust gates did not fail; they were refused.** `typecheck`, `lint`,
  `doc-coverage`, and `test` each aborted on
  `build-limits: the supervisor refused: unhealthy` — the shared
  compiler-admission supervisor went unhealthy mid-batch, so `cargo` could not
  even run `rustc - --print` to learn target information. `lint` cites 17
  refusals and `typecheck` 15; every compile failure is a third-party crate
  compiled under `--cap-lints allow` (13 distinct in `typecheck`), and none is
  netsuke code. `test` reports `Error 102` from `test-nextest` having never
  started, so the doctest half again did not run. None of these four is
  evidence about this candidate: admission itself was unavailable. The
  supervisor recovered shortly afterwards (`healthy, caching through sccache`),
  so a re-run is the disposition rather than a workaround, and `build-limits:`
  refusal is reported rather than bypassed.

  Net verdict for `048b49f8`: six gates reported (five green — `check-fmt`,
  `nixie`, `test-workflow-contracts`, `test-release-publication` — and one real
  red, `spelling`), `markdownlint` masked with no verdict, and four Rust gates
  void by infrastructure refusal.
- [x] (2026-10-16) Ran four gates at `7d3f18c2`, the commit that repaired the
  second spelling defect. `check-fmt` exits 0 in 2 s and `spelling` exits 0 in
  12 s, so **the repair is confirmed by the gate itself** and not only by the
  ad-hoc check that preceded the commit.

  **`markdownlint` then executed and reded on a real defect of this branch's
  own prose.** This entry originally claimed that execution was the first in
  the plan's history, and that the two preceding reds had been masking a live
  failure. **Both claims were false, and the correction matters.** Re-checking
  every revision's log shows the true sequence: at `c1f3f2ed` the target ran
  and **passed** — 184 files, `0 issues in 0 files` — and the same is visible
  in that revision's own log; at `ce5ff20f` and at `048b49f8` it was masked by
  a red `spelling` prerequisite and never ran. So only `048b49f8` masked a
  *defect*, and it did so because that single commit introduced both failures
  at once — the spelling error that aborted the prerequisite, and the MD013
  violation the prerequisite was hiding. `ce5ff20f` masked nothing, because at
  that revision the defect did not yet exist.

  The general lesson is the opposite of the one first written here, and is the
  reason the error is left recorded rather than quietly overwritten: a masked
  gate is an unknown at the time it is masked, and retroactively equating one
  mask with a hidden failure is itself an unsupported claim. The mask can only
  be adjudicated by re-running the gate at that revision, which is what
  established this sequence.

  The violation was at line 1185 of this file *as it stood at that revision* —
  an 89-column line whose over-long content was a long Rust test path. (The
  number is not re-derivable: the line has since been reflowed out of
  existence, so it names a position in a superseded revision of this document
  rather than one in the current file.) The cause is specific and worth
  recording, because the same file already carries lines of 125 and 81 columns
  that pass. MD013's only live exemption here is the **final** whitespace-free
  run of the line: the rule rewrites `line.replace(/\S*$/u, "#")` before
  measuring, so a line whose *last* token is one unbreakable span is exempt no
  matter how long that span is. On line 1185 that last token was a
  one-character em-dash, so the 85-column code span before it was counted and
  the dash itself was the exempt run. Repaired by ending the sentence at the
  code span and starting the em-dash clause as a new sentence, which makes the
  span line-final and matches the two lines that already pass. Targeted probes
  after the edit: `markdownlint-cli2` reports **0 issues** on the plan, and
  `mdtablefix --check` reports it unchanged. The verdict was obtained by
  running the tool, never by predicting it from the rule source.

  `make test` was refused a second time by the same admission failure, so it
  again carries no verdict and the doctest half is again unobserved. The
  supervisor then became **unqueryable** for a period — `build-limits-status`
  exiting 1 with `cannot query the supervisor ... timed out` rather than
  reporting `UNHEALTHY` — and has since recovered to
  `healthy, caching through sccache`, currently
  `capacity 4, held 2, compiling directly`, with peer builds from other
  worktrees waiting in the same pool. A re-run is therefore the disposition for
  every Rust gate, and admission refusal is reported rather than worked around.
  Two genuine `test` timeouts (at `c1f3f2ed` and `ce5ff20f`) remain the only
  real failures recorded against that target; the later refusals are admission
  unavailability and are not counted as failed attempts against the
  `Tolerances` limit of three.
- [x] (2026-10-16) Ran the full ten-gate batch at `9494a7e8`. Six gates
  passed: `check-fmt`, `spelling`, `markdownlint`, `nixie`,
  `test-workflow-contracts` (1291 passed, 3 skipped) and
  `test-release-publication` (212 passed). Four were killed by admission
  refusals and carry **no verdict**: `test` (one refusal), `doc-coverage` (one),
  `typecheck` (seventeen) and `lint` (sixteen). The `lint` run is worth
  recording in detail because it got further than any previous attempt:
  `cargo doc` finished in 2 m 08 s and
  `cargo clippy --all-targets --all-features` finished clean in 43 m 36 s, with
  **zero** refusals, before `lint-whitaker` (`Makefile:373`) went down with the
  supervisor when its dylint dependency build was refused. `typecheck` likewise
  compiled for roughly forty minutes before the supervisor went unhealthy
  mid-build and refused the remaining invocations: the log holds **17
  `build-limits: the supervisor refused` lines, zero rustc diagnostics and zero
  `-->` location lines**, and each `could not compile` wrapper is caused by
  `build-limits-rustc` itself exiting 1 rather than by a compiler error. That
  is the distinguishing evidence: a candidate defect would carry a rustc
  diagnostic with a source location, and there is none. The doctest half of
  `make test` again did not run and remains unobserved. An earlier attempt at
  this same revision is retained as `-r2` and refused identically, so the
  outcome is not a single unlucky invocation.
- [x] (2026-10-16) Repaired three factual defects in this plan, all found while
  checking the pull request description's claims against retained evidence. (1)
  The Progress entry of `ce5ff20f` attributed the sentence "is the queue's to
  decide rather than the test's" to `docs/developers-guide.md` and to the test
  that times out. Neither holds: the sentence is at
  `.config/nextest.toml:59-60`, it is written against the *sibling*
  `cli_configuration_fixture_compiles`, and the timed-out test's own override
  block (`:98-101`) sets no `slow-timeout` at all. The guide's real sentence
  about the timed-out test is at `:9431`. Corrected to cite both accurately and
  to claim only the shared compiler-admission pool. (2) The same entry dated
  the two timeout runs "eleven hours apart"; the retained commit timestamps are
  20:25:08 and 22:47:49 on the same day, roughly two hours twenty minutes
  apart, and the sidecars record only `DURATION=1671` and `DURATION=1613`. The
  unsupported interval is removed. (3) The first-publication premise was stale
  in two places (the Trusted Publishing paragraph and the EP-M3 remaining-gaps
  bullet) and one `Risks` entry rated it severity and likelihood both **high**.
  `netsuke-build` has been published since 2026-08-05 and carries four unyanked
  versions, `0.1.0-beta1` through `beta4` (`beta4` is the current
  `max_version`, published 2026-09-27). The plan contradicted itself: line 26
  already offers `cargo install netsuke-build --version 0.1.0-beta4`, and the
  reconnaissance at `:1657-1660` records beta2–beta4 release assets, so the
  plan assumed publication had happened while its risk register assumed it had
  not. The risk is rescoped rather than deleted, because the failure it names
  is still reachable for a different reason. All three repairs are prose only:
  this branch changes no Rust source, no workflow body and no gate
  configuration, so they alter no executed behaviour.
- [x] (2026-10-17) Closed the review's third defect — the verifier's network
  boundary — by making the invariant executable rather than by rewording the
  claim. `scripts/verify_crates_io_bytes.py` already took a local path and did
  no I/O, so the implementation was right and the *statement* was wrong: Stage
  B described it as a "token-free download" while EP-M2 requires the modules to
  perform none. Stage B item 6 now says what the module does and what the
  workflow step does, and EP-M2's conformance check names the test that holds
  it. The test is `test_policy_modules_import_no_network_client` in
  `tests/workflow_contracts/release_publication_test.py`: it parses each of the
  seven `SOURCE_MODULES`, collects every `import` and `from … import` name, and
  fails on any that equals or nests under a network client. Non-vacuity is
  proved in two directions — a positive control over four spellings
  (`import urllib.request`, `from urllib.request import urlopen`,
  `import http.client as client`, `from socket import create_connection`) and a
  negative control over three non-network imports — and was checked against the
  real tree once by hand: all seven modules clean, and a copy of the verifier
  with `import urllib.request` inserted at line 1 reports `['urllib.request']`.
  Also corrected two stale counts found while editing: the modules are seven,
  not six (`release_semver` was added in Stage B), and the orientation's job
  list said `release` creates the draft when it is read-only and runs only for
  a dry run; `publish-release` creates it at `release.yml:589`. Prose and one
  test; no workflow body, Rust source, or gate configuration changed.
- [ ] Stage C: publication sequence in `release.yml` (EP-M3, EP-M4, EP-M5).
  Scope correction from recon at `c1f3f2ed`: this is not an extraction of
  existing bodies. `release.yml` has no `package-crate`, `publish-crates-io`,
  `verify-release-inventory` or `consumer-smoke` job, and no crates.io step of
  any kind; only `release` and `publish-release` carry the upload sequence that
  the new jobs decompose. Stage C therefore writes five new composite actions
  plus the job graph, and is the largest stage in this plan.
- [ ] Stage D: gates, guards, documentation (EP-M6).
- [x] (2026-10-17) Ran the full eleven-target batch over the review-closure
  change and committed it as `d604b0cd`. Nine targets pass outright:
  `check-fmt` (265 files formatted, 184 Markdown unchanged), `spelling`,
  `markdownlint`, `nixie`, `lint` (second run), `typecheck`, `doc-coverage`
  (98.89% against the 80.00% threshold), `test-workflow-contracts` (1299 plus
  27 passed) and `test-release-publication` (212 passed). The first `lint`
  attempt failed at `Makefile:343` on
  `R9101: Type dispatch on 'node' would be clearer as a match statement` in the
  new `_imported_modules`; Ruff at the pinned 0.16.4 and Pylint at 10.00/10 had
  both passed it, because the df12 house lints are a separate stage that a plain
  `ruff check` never runs. The dispatch is now a `match` over `ast.Import()`
  and `ast.ImportFrom(module=str() as module)`, and the single permitted re-run
  is green.
- [x] (2026-10-17) `make test` has **no passing verdict**, and the failure is
  environmental rather than a defect in this branch. `test-nextest` reached
  3959 passed, 6 skipped and one `TIMEOUT [300.009s]` in
  `netsuke-build::build_module_slice_ui_tests::production_build_module_slice_has_expected_boundary`;
  because `make test` is `test: test-nextest doctest`, that failure meant the
  doctest half never ran. The 300 s is the product `period = "60s"` ×
  `terminate-after = 5`, and the test awaits a nested `cargo build` inside its
  own body, so compile-slot waits are charged to it; the pool was
  `capacity 4, held 4` with the deepest queue at 51 and 18 slot-wait messages,
  and the test's own file is absent from the branch diff. Do **not** widen the
  budget: `.config/nextest.toml` deliberately leaves that override at the
  profile default, and `docs/developers-guide.md` §"The per-test budget is a
  product, not a period" records that a wider budget "would cover the delay,
  and would not remove the contention that causes it". The doctest half was
  then run on its own with `make doctest` and passed — 95 passed/26 ignored, 2
  passed and 39 passed/6 ignored, zero failures — so that half is now green and
  only the nextest half lacks a clean run.
- [x] (2026-10-17) Re-ran `make test-nextest` at the same head and **it
  reproduced**: the same test timed out again at `300.011s`, with 3959 passed
  and 6 skipped, from position 3960 of 3976. This is the more informative of
  the two runs rather than a repeat of the first, because the pool was *worse*:
  449 slot-wait messages with a deepest queue of 238, against 18 and 51. A
  status check taken immediately afterwards showed
  `capacity 4, held 4, healthy, sccache unreachable` with 252 processes queued
  from other agents' worktrees. Contention rising between two runs of an
  unchanged tree, on a test whose file is absent from the branch diff, is the
  diagnostic that separates a host-capacity timeout from a defect. The verdict
  recorded here is therefore **timeout, environmental and reproduced**, not
  pass: this half of `make test` has no green run, and none should be claimed.
  Two remedies exist and neither belongs in this change — wait for a quiet
  host, or accept that a nested `cargo build` under a shared four-slot pool
  cannot be gated in the same interval it is measured in. Widening the budget
  stays excluded for the reason given above.
- [x] (2026-10-19) Began Stage C with the first of its five seams:
  `scripts/release_candidate_cli.py` records the identity the packaging job is
  authorized to publish, and `scripts/tests/test_release_candidate_cli.py` pins
  it in five cases. The red step was observed before the module existed — the
  suite failed on importing a module that was not there — and the green step
  then passed. The CLI is a *thin* seam by construction: argument parsing, one
  call to `release_candidate.resolve_identity`, one write. A rejected identity
  returns `EXIT_IDENTITY_REJECTED` **before** the write is attempted, which the
  third test checks as an absence (`not destination.exists()`) rather than by
  trusting the branch's shape, so a future reordering that wrote first and
  validated second would fail it. The doctest uses `/nonexistent/identity.json`
  for the same reason: an example that wrongly accepted the tag would raise
  `FileNotFoundError` instead of quietly recording an identity, and one that
  wrongly refused would still return `1` — so the destination is chosen to make
  the wrong path observable. The diagnostic itself goes to standard error,
  which a doctest does not compare; that is stated in the docstring rather than
  left for a reader to infer from why the expected block has one line.

  Wiring landed in the same commit, as the orphan guard requires: the recipe
  `test-release-publication` names both the suite and the source module, so the
  target collects the CLI's doctest as well. Measured: **218 passed**, up from
  212, the difference being five suite tests and one doctest. Lint was not
  assumed. `make lint-python` was run to completion, and it took four
  iterations to reach green, each a distinct stage that the previous one had
  masked — exactly the lower-bound behaviour recorded on 2026-10-15:
  `missing-return-type-private-function` (Ruff), then
  `typing-only-standard-library-import` (Ruff, one stage further on the same
  file), then `C9112` plus eight `C9102` and one `R9108` (df12 house lints),
  then two `line-too-long` (Ruff, after the house-lint repair rewrote the
  assertions). The final state exits 0 through all five stages at 10.00/10
  Pylint and 100.0% interrogate. `local_reference_coverage_test.py` also
  passes, which it would not have if the CLI were unreferenced.
- [x] (2026-10-19) Completed the four remaining CLI siblings and committed the
  milestone at `177c5f7d`: `package_crate_cli`, `publish_crates_io_cli`,
  `verify_release_inventory_cli`, and `finalize_github_release_cli`, each with
  its suite registered in `release_publication_test.py`. The batch of six gates
  ran against the staged tree at `bb9829de` and reported five verdicts:
  `check-fmt` exit 0 (275 Ruff files, 184 mdtablefix files), `lint` exit 0 in
  60 minutes, `test-workflow-contracts` exit 0 (`1314 passed, 3 skipped`;
  `27 passed`), and `test-release-publication` was not reached because the
  scrutineer died on a provider 503. `typecheck` exited 2 with four `ty`
  diagnostics, and `test` exited 2 on one timeout; both are treated below.

  The four `ty` diagnostics were real narrowing gaps in the new suites, not
  typos, and were repaired at source rather than suppressed. Three were in
  `test_finalize_github_release_cli.py`, where `_plan` declared
  `dict[str, object]` and so made `plan["actions"]` and `plan["conflicts"]`
  opaque `object` values that could not be iterated or sorted. The helper now
  returns a `_PlanRecord` TypedDict naming both fields, which is the document's
  actual shape and removes the casts a narrower fix would have needed at three
  call sites. The fourth was in `test_package_crate_cli.py`, where the
  prerelease assertion subscripted the identity mapping without the
  `isinstance(identity, dict)` guard its sibling already carried; the guard is
  now there too. Measured after the repair: `ty check` on both files reports
  `All checks passed!`, the two suites report **21 passed**, Ruff format and
  check are clean, Pylint is 10.00/10 and the df12 house lints are 10.00/10.

  `make test`'s failure was **not** a defect in this change. It was
  `build_module_slice_ui_tests::production_build_module_slice_has_expected_boundary`
  hitting its 300 s allowance at position 3960 of 3976, with 3959 passed and 6
  skipped — the third reproduction of the documented host-capacity timeout, on
  a test whose own file is absent from the branch diff. The host at the time
  held 196 Cargo processes with `capacity 4, held 4` and waiters from another
  worktree. The verdict recorded is therefore **timeout, environmental**, and
  this half of `make test` still has no green run at this head. The doctest
  half is unaffected by any change in this milestone: the diff is Python and
  Markdown only. Widening the budget stays excluded for the reason recorded on
  2026-10-17.

- [x] (2026-10-19) Resolved the staging-layout ambiguity the three asset-name
  findings opened, **before** writing the declared-name producer, so Stage C's
  remaining work has one settled target rather than a choice deferred into
  code. The Decision log entry records the outcome — a flat twenty-name
  consumer set — and the two release-path consequences that follow from it.
  Also corrected two docstrings that still asserted the falsified namespace
  reason (`scripts/hoist_binstall_archives.py`,
  `tests/workflow_contracts/conftest.py`) and the `Idempotence and recovery`
  sentence that credited the finalizer with an upload it never performs. `grep`
  over `tests/` confirms no assertion pinned either claim, so both corrections
  are documentation-only and change no behaviour.
- [x] (2026-10-19) Established the exact contract coupling that governs the
  remaining Stage C commits, before writing any of them, so each dependency is
  an obligation recorded in advance rather than a red gate discovered after the
  fact. Two couplings are hard and both are same-commit edits:

  1. `release_publish_path_publication.py:_publication_needs` compares
     `set(publish-release.needs)` against `REQUIRED_NEEDS` by **exact set
     equality**, so growing `publish-release.needs` with `package-crate`
     obliges the matching edit to `REQUIRED_NEEDS` in
     `release_publish_path_scenarios.py`. Growing one without the other fails
     with "expected metadata, build and smoke dependencies", which names the
     symptom but not the second file.
  2. The same module anchors the publication step list on four exact
     identifiers — `Ensure release exists (draft)`,
     `actions/download-artifact@`, `hoist_binstall_archives.py`, and
     `id: upload_assets` — and `_check_publication_step_order` pins their
     order. `check_upload_plan` additionally requires the `with.dry-run` key on
     the step named `Upload artefacts to release` to resolve `False` on a
     publication, and `tests/workflow_release.rs:77` pins that expression as
     raw text. A per-name `gh release upload` rewrite therefore has to keep the
     step's id, its name, its position, and a `dry-run` key — or edit those
     four assertions in the same commit, which would move the contract away
     from the property it states.

  A third coupling is informational: `release_publish_path_mutations.py`
  mutates the release job's upload step through
  `named_step(…, "Upload artefacts to release")` and `with.dry-run`, so the
  same anchors recur in the mutation suite.
  `release_operation_observability_test.py` reads
  `steps.upload_assets.outputs.upload-error`, which means the upload step must
  continue to publish that output however its body changes.
- [x] (2026-10-19) Built the declared-name producer and wired it into the
  gates in the same commit, so Stage C's remaining work has one settled target
  rather than a naming rule invented inside the upload step. The producer is
  two modules because `scripts/release_inventory.py` is pinned at the 400-line
  module cap and cannot carry it:

  - `scripts/release_declaration.py` states the flat consumer set. The archive
    and sidecar half comes from configuration through
    `release_inventory.binstall_asset_names`, which delegates to the hoist's
    own authority, so it needs no staged file and a build that shipped no
    archive still declares all twelve. The operating-system half is read from
    the staged tree — three external shared actions own those names — and is
    held to `target_count × len(formats)` per platform, so a package that
    failed to build refuses the declaration rather than shrinking it.
    `PUBLISHED_CHARACTERS = {"~": "."}` is the only normalization, and its
    docstring says why a general sanitizer would be wrong.
  - `scripts/release_declaration_cli.py` is the thin seam the workflow
    invokes. It separates `EXIT_MISSING_ASSETS` (the tree disagrees with the
    build's promise) from `EXIT_INPUT_UNUSABLE` (the configuration or manifest
    was wrong before any asset was considered) and writes the file only on
    success, because a partial declaration would let a later stage verify a
    shorter release against itself.
  - `scripts/hoist_binstall_discovery.py` promoted `_walk_files` to public
    `walk_files` rather than duplicating its traversal. A second walk would
    have had to re-earn the `Path.rglob` error propagation, or silently differ
    from it.
  - The refusals are classified by exception family:
    `ConfigurationError` (no targets, malformed target, unpackageable
    platform) and `StagedTreeError` (missing package, duplicate published
    name), both under `DeclarationError`. The CLI's two exit codes are that
    distinction, not a re-derivation of it.

  Wired in the same commit: `SECTION_MODULES` and `SOURCE_MODULES` in
  `tests/workflow_contracts/release_publication_test.py` and the
  `test-release-publication` recipe gained the same four paths in the same
  order, since the contract compares them by exact list equality. Verified
  green: `make lint-python`, `make typecheck-python`,
  `make test-release-publication` (300 passed), `make test-workflow-contracts`
  (1320 passed, 3 skipped), and `make check-fmt`.
- [x] (2026-10-19) Repaired four lint families at source while wiring the
  producer, none of them by suppression. Ruff's
  `typing-only-standard-library-import` moved `pathlib` under `TYPE_CHECKING`;
  `docstring-missing-exception` and `docstring-extraneous-exception` required
  the `Raises` blocks to name only what the function body raises itself, with
  propagated helpers documented in `Notes`; `escape-sequence-in-docstring` was
  cleared by building the doctest's TOML with `chr(10).join` rather than a
  doubled backslash, because an `r"""` prefix would have changed the example's
  own escapes. The df12 house lints required a failure message on the last bare
  `assert`, and `typing.Iterable` gave way to `collections.abc.Iterable`.
- [x] (2026-10-20) Closed the two Stage C unknowns that could not be settled
  from the plan: how `verify-release-inventory` reads a draft release at all,
  and whether the Stage D composite-action lint target can reuse
  `.yamllint.yml`. The first is a confirmed blocker with a recorded deviation
  (`contents: write` on the verifier); the second is a config incompatibility
  with a one-line repair. Both are recorded in `Risks`,
  `Surprises & discoveries`, and `Decision log` before any code was written, so
  the deviation is an obligation recorded in advance rather than a red gate
  discovered afterwards.
- [x] (2026-10-20) Re-measured the publication graph contract's Red state at
  the current head, using the Makefile's own pinned module set, and got the
  expected **18 failed, 3 passed**. The three that pass are the two
  self-consistency readers (`test_the_workflow_path_is_the_release_workflow`,
  `test_the_workflow_job_helper_returns_the_same_job`) and
  `test_the_workflow_still_declares_no_job_without_permissions`, which is
  already true of the seven existing jobs. This is the Red baseline Stage C
  must turn Green; it is the plan's stated evidence line quoted from
  `Plan of work`.
- [x] (2026-10-20) Began Stage C by probing the packaging seam before authoring
  the action that calls it, and found the seam could not have worked:
  `cargo package --target-dir` writes to `<target-dir>/package/`, while
  `package_crate` globbed the target directory itself. Repaired with a named
  `PACKAGE_SUBDIR` constant, a `Notes` paragraph recording the layout and the
  probe, and a new test that pins the nested read — the stub had been writing
  to the target root, which made the suite green while the bug stood. Suite
  re-measured at **11 passed**, up from 10. The finding is recorded under
  `Surprises & discoveries` with the observed layout.
- [x] (2026-10-21) Rebased onto `main` at `37fc364a` from the exclusive replay
  boundary `fa5a4ffa` (27 commits replayed, zero conflicts, `range-diff` fully
  `=`). Re-validated the post-rebase candidate at HEAD `b74f6571`, tree
  `a0c32ca3`: `check-fmt`, `typecheck`, `test`, `lint`,
  `test-workflow-contracts` and `test-release-publication` all exit zero.
  Corrected four line-number citations this plan carried and recorded two
  measurements of `main`'s conventions that Stage C must now follow.
- [x] (2026-10-21) Corrected two of those four citations in the pass that
  followed, and left the other two alone after re-deriving them — the first is
  rebase-induced (`docs/developers-guide.md:1361-1362` → `:1415-1416`) and the
  second is a pre-existing inaccuracy in a branch-owned file
  (`release_inventory.py:319` → `:318`), while `verify_crates_io_bytes` and
  `nextest_durations` are correct as written. The DOC502 citation at
  `release_reconcile_crates.py:161` was challenged and **survived**: probing
  `ruff check --preview --select DOC502` against the named commit reports
  `:161:5`, the docstring's opening line, so the number was right and the gap
  was provenance — the rule is Ruff's own preview rule, not a df12 Pylint
  plugin rule, which rates the same blob 10.00/10. Committed at `958fdf68c`,
  force-pushed with the lease bound to `75fd9195f`.
- [x] (2026-10-21) Repaired the two latent composite-action lint defects the
  Stage D target would trip on, in `windows-msi-upgrade-validation/action.yml`
  (added the `---` document start) and `windows-gate-cache/action.yml` (removed
  the trailing blank line). Both pre-exist on `main` and are outside every
  existing target's reach, which is why they were invisible. Measured both
  directions rather than assumed: the pre-repair content still yields exactly
  the two findings, the repaired content yields none, and all five manifests
  still parse with their names and step counts. Committed at `b07de6038`, tree
  `f8cf6c91`, after `check-fmt`, `markdownlint`, `lint`, `typecheck`, `test`,
  and `test-workflow-contracts` all exited zero. Also recorded in this plan
  that its two "a blanket sweep is green today" statements concern
  **different** invariants — the expression-site rule (green: 68 sites, 0 in a
  `run` block) and the yamllint sweep (red before this commit) — because
  conflating them is how the Stage D target gets written with no repairs and
  then acquires an exemption.
- [x] (2026-10-21) Settled the last Stage C unknown that blocked authoring the
  composite actions: where `--package-name` comes from. The answer is
  `steps.ensure_version.outputs['crate-name']`, read from a new `package_name`
  output on the `metadata` job. The reasoning is in the Decision log; the short
  form is that `ensure-cargo-version` at the pin `release.yml` already carries
  resolves `[package].name` and writes `crate-name` unconditionally, so the
  true registry identity is already in the job that runs first, and the two
  alternatives each cost a contract-test edit for no behavioural gain. Probed
  rather than assumed: the pinned `export-cargo-metadata@a5765019` does declare
  a `name` output and does accept `name` in `_SUPPORTED_FIELDS` (so the
  alternative was *viable*, just worse), `tests/workflow_release.rs:35` does pin
  `fields: bin-name` by exact substring, and no test pins `metadata.outputs`
  by exact set equality — the three facts that decide between the candidates.
  Recorded before any action was written, so the wiring follows the decision
  rather than the decision being reconstructed from it.
- [x] (2026-10-21) Corrected a working assumption that had been carried since
  the last compaction and would have misdirected the whole of Stage C. Against
  `main` — the stale local ref — six files appeared modified, which made their
  contract pins look overridable. Against the branch's **actual** base,
  `37fc364a`, which is also `origin/main`, the branch touches none of
  `.github/workflows/release.yml`, `tests/workflow_release.rs`,
  `scripts/cargo_package_metadata.py`, or
  `tests/workflow_contracts/linux_package_metadata_test.py`. Their pins
  therefore bind from `main` unmodified, and the `fields: bin-name` substring
  is a constraint to design around rather than one this branch may edit away.
  The stale local `main` is 107 commits behind `origin/main`; every provenance
  check must name `origin/main` and not the local ref.
- [x] (2026-10-21) Settled the other Stage C unknown blocking the composite
  actions: where `--commit` comes from. Every action and job that needs the
  candidate commit reads `${{ github.sha }}` into an `env:` key and passes it
  through a declared `commit` input, so the value crosses the action boundary
  as an ordinary declared input rather than as a hidden ambient read. Rationale
  and the rejected alternative (resolving the tag to a commit) are in the
  Decision log. Gate batch over this plan-only delta: `check-fmt`, `lint`,
  `typecheck`, `markdownlint`, `nixie`, and `test` all exited zero, covering
  plan content hash `eff285ff`. Committed at `f07fcaa38`.
- [x] (2026-10-21) Corrected a now-stale Stage D bullet. It read as a
  prediction — the composite-action lint target "is green only after two
  pre-existing defects … are repaired" — but both repairs landed in
  `b07de6038`, so the target's first run is green on the threshold and this
  branch's own manifests are what any red is attributable to. Re-read the
  source rather than trusting the note: `head -1 | cat -A` on
  `windows-msi-upgrade-validation/action.yml` shows `---$`, and
  `tail -c 60 | cat -A` on `windows-gate-cache/action.yml` ends at the cache
  key with no trailing newline beyond it.
- [x] (2026-10-21) Closed the hole Stage C would have opened in the rule it
  was written to satisfy, and settled the last design question before the
  audits. Extracted the run-interpolation predicate and its two containers into
  `tests/workflow_contracts/action_run_sources.py` so one rule serves workflow
  jobs and composite-action manifests alike, and made
  `release_run_interpolation_test` import it. This is a prerequisite rather
  than a tidy-up: the stage moves five job bodies out of `release.yml` and into
  `.github/actions/`, and the contract's own step container skipped every job
  carrying `uses`, so extraction would have removed those shells from the
  contract's reach. `require_action_step` returns a declaration index so a
  mutation restores an unsafe body where it came from instead of asserting
  against an absent step; its terminal `pytest.fail` is written as
  `return pytest.fail(...)` because the bare form trips pylint R1710 after
  `return` arms — the failure that made the previous lint batch red. Full
  seven-gate batch at HEAD `cd85a1a1c` over candidate digests `980be42b…` and
  `2761f735…`: `check-fmt`, `lint`, `typecheck`, `markdownlint`, `nixie`,
  `test` (4300 nextest, 113 doctests) and `test-workflow-contracts` all exited
  zero. Committed at `374da419d` and pushed.
- [x] (2026-10-21) Settled how the upload step carries the declared names,
  the last Stage C unknown that blocked authoring the actions. The step keeps
  its `id`, name, position and `with.dry-run` and becomes a `uses:` call to a
  new local action `.github/actions/upload-declared-assets/`, because a `run:`
  step has no `with` and so cannot satisfy the coupling the plan already
  states. The rehearsal lane keeps the shared action, so
  `tests/workflow_release.rs:25`'s raw `upload-release-assets@` pin survives on
  one occurrence with no Rust edit. Rationale, the measured two-occurrence
  count, and the rehearsal-lane gap left for Stage D are in the Decision log.

- [x] (2026-10-23) Committed the packaging job's own slice at `7527111ae`
  after `check-fmt`, `lint`, `typecheck` and `test` all exited zero on exactly
  those bytes. It adds `.github/actions/package-crate/` and the `package-crate`
  job that calls it, which is what discharges the coverage obligation measured
  two entries above, and it sets `package_name` on `metadata` from
  `steps.ensure_version.outputs['crate-name']` so neither the packaging seam
  nor the registry exchange is fed a binary name in the package-name slot.
- [x] (2026-10-23) Rebased the whole branch onto `origin/main` at `6572fb956`
  from its current base `37fc364a` (37 commits replayed, `range-diff` reports
  36 identical and one changed). One conflict, predicted before the replay by a
  `git merge-tree` rehearsal: the `Makefile` `.PHONY` line, where `main` had
  added `test-default-features` and `lint-default-features` for its new
  default-feature lane and this branch had added `test-release-publication` for
  its script suites. Resolved as the union of the two sides, verified token by
  token rather than by eye — the result contains every token from both sides
  and adds exactly one — because each side's addition is load-bearing and
  dropping either would silently unschedule a real target. Every other branch
  commit replayed byte-identical, and the only delta `range-diff` shows is that
  line. `main`'s work survives intact: the default-feature lane, its workflow
  file, and all 49 renamed ADRs are present, and the branch deletes no file
  that exists on the target. Gates: see the next entry.
- [x] (2026-10-23) Repaired the one defect the rebase introduced into this
  plan, and only that one. `main` moved every ADR to `docs/adrs/adr-0NNN-` with
  a longer slug; this plan cited seven times the two paths that move mattered
  for. Rather than trust a rename map, resolved **every** doc path the plan
  cites against the filesystem — 11 distinct paths, exactly 2 missing — and
  confirmed the new files carry the expected serials and subjects, since that
  rename had already collided serials elsewhere. Also confirmed the other
  stale-looking `.md` paths the repository still contains are `main`'s own
  deliberate historical quotations in other ExecPlans, not defects to inherit.
  `main` performed no equivalent rewrite of those, so this branch carries no
  obligation to invent one.

- [x] (2026-10-23) Re-derived every line-number and file-path claim this plan
  makes against the rebased base, and corrected the ones `main`'s four new
  commits had invalidated. The drift was mechanical and traceable: #939 changed
  `release.yml`'s Windows build to `--workspace` and added three comment lines
  above it, which shifts every citation below that point by three, so
  `release.yml:412/545/586/537` are now `415/548/589/540`. The file itself also
  grew — 685 lines at the old base, 688 at the target, 733 in this branch after
  the `package-crate` job — so the five live "685 lines on `main`" statements
  now read 688 and the `ci.yml` comparison reads 710 (was 642). The
  developers'-guide citation for the composite-action convention moved from
  `:1415-1416` to `:1574-1575`. Deliberately **not** touched: the `685` in the
  dated `Revision note` and the two `1415-1416` mentions that record a previous
  rebase — those are historical statements about what was true on the date they
  name, and rewriting them would falsify the record rather than correct it.
  Verified each new number by printing the target's line at that offset, not by
  trusting an arithmetic shift alone.

- [x] (2026-10-23) Repaired the first defect the new packaging lane showed on
  its first real execution, which CI surfaced and no local suite could. The
  hosted `release / package-crate` job failed on every pull-request run with
  `tag '903/merge' is not a release tag: expected 'v' followed by a semantic
  version such as 'v1.2.3'`
  and exit `2`: on a `pull_request` event `github.ref_name` is the synthetic
  merge ref, not a tag, and the packaging step forwarded it to a validator that
  exists precisely to refuse a tag that does not name the commit the bytes came
  from. The repair carries the mode explicitly instead of loosening that
  validator: `release.yml` now passes
  `dry-run: ${{ needs.metadata.outputs.should_publish != 'true' }}` — the same
  expression `upload-release-assets` already receives — the composite action
  selects exactly one of `--tag` and `--dry-run` before invoking the CLI, and
  the CLI makes the two mutually exclusive so a *publishing* call site that
  forgot its tag is an argument error rather than a silently tagless identity.
  A rehearsal's record now stores `tag: null`, and
  `_require_tag_version_agreement` accepts an absent tag as the recorded
  absence of one. `tests/workflow_contracts/release_package_tag_test.py` pins
  the wiring across the workflow and the action manifest. That repair pushed
  `release_candidate.py` from 394 to 426 lines, past the 400-line cap
  `pyproject.toml` sets for `too-many-lines`, and the module had only six lines
  of headroom before it, so the overage was not avoidable by tightening the new
  prose. The record *reader* — `identity_from_mapping`, its two error classes,
  the field list, and `_recorded_tag` — moved to a new
  `scripts/release_identity_record.py` (154 lines), leaving
  `release_candidate.py` at 304. The seam is not arbitrary: a resolver sees
  facts the workflow has just gathered, while a reader sees whatever is on
  disk, which may be hand-edited, truncated, or stale, and that difference is
  exactly why the stored flag is checked rather than trusted. The split follows
  the precedent `release_semver.py` set when the same module was divided
  before. Both files are in the `test-release-publication` recipe and its
  `SOURCE_MODULES` contract, so the new module's doctests run and an unwired
  module would fail the suite. Gates: see the next entry.
- [x] (2026-10-23) Cleared every CodeScene finding on the pull-request delta
  and brought every changed file back under the 400-line module cap, as two
  commits. `cs delta origin/main --include-metadata` now reports
  `status: no-issues-found` across 37 eligible files, and `--error-on-warnings`
  exits `0`. The remediation was decomposition, not metric gaming: the
  asset-name predicates moved to a new `scripts/release_asset_names.py` (152
  lines) and the platform/format policy with its eight refusals moved to a new
  `scripts/release_package_formats.py` (133 lines), leaving
  `release_inventory.py` at 341 and `release_declaration.py` at 386.
  `release_declaration` re-exports the moved names through an explicit
  `__all__`, verified to preserve exception identity
  (`release_declaration.MissingPackageError is
  release_package_formats.MissingPackageError`),
  so `release_declaration_cli.py`'s `except` clauses still catch what the
  moved code raises. `package_crate`, `finalize_github_release_cli`'s record
  reading, and `classify_registry_state` were decomposed into named helpers,
  and the workflow-contract scans split their path predicates from their
  comment/escape handling. Both new modules are named in the
  `test-release-publication` recipe and its `SOURCE_MODULES` contract. The
  first gate batch caught a genuine `ruff format` failure in the declaration
  suite's `_workspace` signature, fixed separately; the second caught an
  `mdtablefix` rewrap of the entry above. Both are recorded here rather than
  smoothed away, because each is the kind of mechanical defect the freeze rule
  exists to force into the open instead of under a running gate.
- [x] (2026-10-23) Repaired the two gate failures the module split exposed, and
  a third the repair itself unmasked. The third batch of gates ran at
  `a461c427` and returned two failures, both in tests the split had touched but
  not caused. `make typecheck` reported four `invalid-argument-type`
  diagnostics in `scripts/tests/test_release_candidate.py`: the split's own
  `_facts` helper unpacked `{**base, **changes}` into `CandidateFacts`, and a
  `dict[str, str | None]` widens *every* field to nullable, so the four
  non-nullable fields no longer typechecked. Replaced with a fully tagged
  `TAGGED_FACTS` value varied through `dataclasses.replace`, which checks each
  replacement against the field it replaces and keeps the per-field agreement
  the mapping form threw away. `make test-workflow-contracts` reported a
  Hypothesis `DeadlineExceeded` at 296.71 ms against the 200 ms default in
  `tests/workflow_contracts/local_reference_property_test.py`, whose examples
  write, scan, and remove two synthetic workspaces on disk; the same example
  re-ran in 5.95 ms, so the deadline measured scheduler contention rather than
  the property. Disabled with `@settings(deadline=None)`, matching the 30 of 57
  sibling `@given` tests in that suite that already do so. Both fixes were
  verified in isolation — `make typecheck` exited 0 and the focused suite
  reported `61 passed` — before being committed together as `880e98f82`. The
  fourth batch then ran all ten gates at `880e98f82` and returned nine green;
  `make lint` was the sole failure, and it failed only on the Python side, with
  `cargo doc`, Clippy under `-D warnings`, and both Whitaker invocations
  passing. Its Ruff stage reported 22 findings in the modules the split had
  created or rewritten: 6 `docstring-extraneous-exception`, 9
  `docstring-missing-returns`, and 4 `escape-sequence-in-docstring`, plus 3
  auto-fixable import/`__all__` orderings. Every one is a defect the split
  introduced, which is why the branch carries the fix rather than main. The
  remedies were structural, never suppressive — there is no `noqa` precedent
  for the `DOC` rules anywhere in the repository, and the repository's own
  instruction is that lints are silenced only as a last resort. A `Raises`
  entry naming an exception the function delegates to a helper now lives in
  `Notes` prose, because Ruff's `DOC501` requires the `raise` to appear in the
  documented function itself; genuine `Returns` sections were added where the
  split lost them; and four docstrings carrying backslash escapes became raw
  strings, with their doctest bodies adjusted so the examples still see a
  newline rather than a literal `\n`. All eight files remain under the 400-line
  cap, with `release_declaration.py` the largest at 384. Because `make lint`
  stops at the first failing sub-target, the four stages behind Ruff — Pylint,
  the df12 house lints, `ambrleaks`, and Interrogate — had never run at this
  revision and are therefore unverified rather than passing; the next batch
  establishes them, and their finding count is a lower bound on the first run.
  Running that cascade directly unmasked one further finding: the df12 house
  lint `R9109` (`prefer-snapshot-substring`) reported that
  `release_package_tag_test.py` probed one subject — the action's `run` body —
  with three substring assertions. The remedy was the one the rule asks for, a
  table: the three fragments and their reasons became `SELECTOR_FRAGMENTS`, and
  the test is now a `pytest.mark.parametrize` row per fragment, so the row that
  fails names itself. That raises the case count from three to five — the `ids`
  list keeps the two rewritten selectors readable — without weakening any
  assertion.

- Repair: **the trusted digests are now anchored before the upload, not read
  back from the download.** The hosted completeness assessment of `f7168a6f2`
  rejected the flow the plan had recorded: with
  `verify_release_inventory_cli --output` writing `{"assets": {…}}` and
  `finalize_github_release_cli` reading that same file, the expected digests
  were a function of the bytes they were meant to check, so the comparison
  could detect a missing asset and nothing else. The decision log's **Amended
  2026-10-23** entry records the reversal. The implemented flow adds two
  modules: `release_digest_record` owns the record's shape, its reader and
  writer, and both comparisons made against it; `release_staged_digests` hashes
  the staged tree and refuses a declared name that is unstaged or staged twice
  under one published spelling. `release_declaration_cli` gained
  `--digest-record` and now produces the trusted record from the staged bytes,
  keyed by `published_name` so a prerelease package is recorded under the dot
  spelling GitHub actually publishes; `verify_release_inventory_cli` gained a
  required `--trusted` and compares every download against it through
  `require_downloads_match_record`; `finalize_github_release_cli` reads the
  build's record rather than the verifier's output. The two substitution cases
  the assessment prescribed are
  `test_verify_rejects_a_replaced_operating_system_package` and
  `test_verify_rejects_a_replaced_archive_and_sidecar_pair` — the second
  rewrites the sidecar to describe the substituted archive, so both older
  agreements hold over the download and only the trusted record can refuse it.
  `RecordedDigestMismatchError` derives from both `DigestRecordError` and
  `release_digest.DigestMismatchError`, so a caller guarding either hierarchy
  catches it. The 400-line Pylint cap fired on
  `test_verify_release_inventory_cli.py` at 424 lines once the new cases
  landed; rather than delete a case or suppress the lint, the duplicated
  staging fixtures moved to `scripts/tests/release_staging_support.py`, which
  both staging suites now import. That extraction is a genuine deduplication —
  the two suites had carried near-identical five-function fixture sets — and it
  removed exactly the overage. `release_staging_support.trusted_digests` hashes
  the staged files with `release_digest.digest_of_file` directly instead of
  reading back what the producer wrote, so a substitution test compares the
  download against an independent account of the build's bytes; a record
  obtained from the module under test would let a fault in that module vouch
  for itself. `release_staging_support` is registered in neither the Makefile
  recipe nor the contract tuples because it is not a test module and carries no
  doctest, which is the same treatment `release_admission_test_support.py`
  receives. Both new modules are registered in both contract tuples, in
  Makefile-recipe order, because `SECTION_MODULES` is compared by list equality
  and `SOURCE_MODULES` parameterizes the no-network and docstring-example
  contracts. At this tree, `make test-release-publication` reports 377 passed,
  `make lint-python` clears all five stages, and `make test-workflow-contracts`
  reports 1419 passed with 3 skipped. Date/Author: 2026-10-23, implementation
  agent.
- Repair: the CodeScene "Overall Code Complexity" advisory fired on
  `release_digest_record.py` at 9.38, the only one of the six release modules
  below 10.00, and it reproduced locally with `cs check`. The metric is the
  mean cyclomatic complexity across the file, so the reported score names no
  function; `require_record_matches_declaration` was carrying two independent
  obligations, the empty-declaration premise and the set-coverage comparison.
  The comparison and its diagnostic now live in `_require_complete_coverage`,
  which is a cohesive decomposition rather than a dilution: each function
  states one requirement and owns one error path, and the caller no longer
  interleaves a premise guard with a coverage decision. Measured with
  `cs check`, the file is 10.00 at 362 lines; the other five release modules
  were already 10.00 and were not touched. The affected suites pass: 91 tests
  across the five release test modules and the five modules' own doctests.
  Date/Author: 2026-10-23, implementation agent.
- Repair: CodeRabbit's follow-up (comment `6360956632`) found one LOW defect
  the `173c6ff4a`/`fc9aa9ea9` revisions still carried.
  `verify_release_inventory_cli` reported an absent `--trusted` record as
  `EXIT_INVENTORY_REJECTED`, because `read_digest_record` raised the single
  `DigestRecordError` for both "this path does not exist" and "this file parses
  and is not a record". The CLI's own docstring at lines 199-203 already
  promised `EXIT_INPUT_UNREADABLE` for an unreadable trusted record, so the
  code, not the contract, was wrong. `UnreadableDigestRecordError` now derives
  from `DigestRecordError` and is raised only when the file cannot be read at
  all; the verifier maps it to exit 2 and keeps exit 1 for a record that parses
  but is malformed. Deriving from the base class means every existing catcher
  is unchanged. The first attempt at this fix scored 9.09 on CodeScene, because
  putting the read and the parse in one `_record_object` pushed it to `cc = 9`;
  reading moved into `_parsed_record`, which is a cohesive split (get the
  bytes; parse them as JSON) rather than a dilution, and
  `release_digest_record.py` is back to 10.00. Two files landed at exactly the
  400-line cap, so the test module's duplicated rationale and the record
  module's repeated prose were consolidated rather than any coverage dropped.
  `scripts/tests/` gained
  `test_the_cli_reports_an_unreadable_trusted_record_as_unreadable`, whose
  declaration is readable on purpose: the pre-existing unreadable-input test
  makes the declaration absent too and returns before the trusted record is
  opened, so it could not have reached this branch. Verified by probe under the
  3.14 baseline: absent path gives `UnreadableDigestRecordError`; non-UTF-8
  bytes, non-JSON text, a JSON array, and an empty `assets` mapping all give
  `DigestRecordError`. Date/Author: 2026-10-23, implementation agent.

## Surprises & discoveries

- Observation: **`cargo package --target-dir` nests its output one level down,
  so the packager's directory assumption was wrong and every packaging run
  would have failed.** `package_crate_cli.package_crate` globbed
  `destination.glob("*.crate")` non-recursively and raised unless it found
  exactly one, while cargo writes the finished archive to
  `<target-dir>/package/<crate>.crate` beside the `tmp-crate/` scratch tree its
  verification unpacks into. Evidence: probed twice on the pinned toolchain at
  the current head, once with `--no-verify` and once with the default verifying
  run. Both left exactly two hard links of the archive —

  ```plaintext
  <target-dir>/package/netsuke-build-0.1.0-beta4.crate       (inode 13632987)
  <target-dir>/package/tmp-crate/netsuke-build-0.1.0-beta4.crate (inode 13631912)
  ```

  — and a bare `glob("*.crate")` over the target directory returned `[]` in
  both cases. The two entries are separate inodes holding identical bytes, so
  the hazard is not "the same file twice": each is size 1126402, so a recursive
  search would find two and refuse. Impact: the fix is a named `PACKAGE_SUBDIR`
  constant and a glob beneath it, and the verification pack now stages the
  crate where cargo stages it — the stub previously wrote to the target root,
  which made the bug unobservable from the suite, so the suite was green while
  the seam could not work. This is the first Stage C defect found by execution
  rather than by reading, and it was found before any action was written, which
  is the point of probing a seam's assumptions instead of trusting a `--help`
  synopsis.

- Observation: **a linter can demand two mutually exclusive fixes, so the
  finding to act on is the one that names a real defect.** Ruff reported both
  `escape-sequence-in-docstring` ("use an `r"""` prefix") and `line-too-long`
  on the same doctest. Adding `r` would have kept `\\n` as two literal
  characters and broken the example's TOML; the actual defect was the doubled
  backslash, and rewriting the fixture through `chr(10).join` cleared both
  findings without a suppression. Take the finding that describes the code's
  fault, not the one that offers the cheapest edit.
- Observation: **`docstring-missing-exception` and
  `docstring-extraneous-exception` divide on "raised here" versus "raised by a
  helper".** Documenting a propagated helper exception in `Raises` produced
  `extraneous-exception`, because the function body never names it. The house
  style is `Raises` for what the body raises and `Notes` for what propagates;
  the two findings are a style guide expressed as two checks, not a
  contradiction.
- Observation: **a failing Make prerequisite can hide a downstream gate
  entirely, so a gate's silence is not a gate's pass.** `make markdownlint`
  depends on `spelling` (`Makefile:448`), and when `spelling` exited 2 the
  recipe never ran: the log read `make: *** [Makefile:452: spelling] Error 2`
  and stopped. The batch's report therefore listed three failures where the
  third had produced *no verdict at all* — and the real MD040 errors beneath it
  stayed invisible until `spelling` was fixed and the target re-run on its own.
  Evidence: `docs/execplans/use-trusted-packaging.md` carried three bare fences
  (`1261`, `1310`, `1578` at that revision), all introduced by my own
  `e2b9794b` — the pre-edit `0440bfaa` had none — and `markdownlint` reported
  `0 issues in 0 files` once it finally ran. Impact: **re-run a
  prerequisite-aborted target after fixing its prerequisite; do not record the
  abort as that target's result.** A gate that never executed is missing
  evidence, not a pass and not even a known failure.

- Observation: **DOC502 follows the public function's own body only**, so a
  `Raises` section naming an exception raised solely by a private helper is
  itself a lint error. `classify_registry_state` documented
  `RegistryResponseError` while only `_version_records` and `_checksums` raised
  it, which was correct to a human reader and wrong to the linter. Evidence:
  two probes. A synthetic module at `/tmp/doc502probe/probe.py` reproduced the
  finding, and the real tree reports `docstring-extraneous-exception` at
  `release_reconcile_crates.py:161` in the revision the finding was measured on,
  `f6ba95f1b` — the docstring's opening quote line, which is where Ruff
  anchors the report: the rule fires on the docstring, not on the `Raises`
  block, whose heading is at `:177`. Re-verified 2026-10-21 at the rebased head
  by running `ruff check --preview --select DOC502` against that revision's
  blob from Ruff `0.16.4`, which prints `release_reconcile_crates.py:161:5` and
  the message
  `Raised exception is not explicitly raised: RegistryResponseError`. The line
  is deliberately *not* renumbered to the rebased file: the fix this
  observation describes deleted that `Raises` block, so no post-rebase head
  carries the finding, and the revision named is the only head on which it
  existed. Note that the rule is Ruff's, not a df12 Pylint plugin rule — the
  plugin returns a clean 10.00/10 on the same blob — so a reader re-deriving
  this must use `ruff check --preview`, not `pylint`. The
  `release_inventory.py` precedent settles the convention: `Raises` names what
  the function's own body raises — `validate_inventory` lists only
  `EscapingAssetNameError` and `MalformedAssetNameError` for that reason — and
  helper-raised exceptions are named in `Notes`. Impact: the four exceptions
  `validate_inventory` inherits from `_require_usable_entries` and
  `_require_declared_agreement` are documented in its `Notes` prose, not in its
  `Raises` section.
- Observation: **the backslash line-continuation form in a doctest does not
  work in a raw docstring**, so it is unavailable wherever the docstring
  discusses backslashes. `release_inventory.py:318` and
  `verify_crates_io_bytes.py:279` use `\` at column 0 to fit a long expected
  exception line under Ruff's 88-column limit, which `make lint` enforces and
  `ruff format` does not. Python splices the physical lines into one logical
  line before `doctest` sees them, so the continuation must sit at **column
  0**: indent it and its leading spaces are spliced into the expected message.
  `parse_sidecar`'s docstring is raw — it explains the GNU escaped-path form —
  so its backslash stayed literal and `doctest` rejected the example with
  "inconsistent leading whitespace". Evidence:
  `ValueError: line 55 of the docstring for
  release_digest.parse_sidecar has inconsistent leading whitespace`
  from `make test-release-publication`, resolved by `# doctest: +ELLIPSIS`
  with the message truncated to `...`, which has a repository precedent at
  `tests/workflow_contracts/nextest_durations.py:243`. Impact: a raw docstring
  needs ELLIPSIS, a non-raw one may use the continuation.
- Observation: **the df12 house lints require a failure message on every
  `assert`** (C9102, an enable-only rule). Evidence: the four Stage B suites
  carried 53 bare asserts and reported 53 findings, while the three sibling
  suites that predate this work pass because every assert already carries one.
  Impact: new `scripts/tests` modules must write a message per assert from the
  outset; the requirement was invisible to Ruff and to Pylint's default set.
- Observation: the shared `upload-release-assets` action rejects `.tar.gz`
  outright, because `Path("x.tar.gz").suffix` is `.gz`. Evidence: read from its
  `scripts/upload_release_assets.py` at pin
  `a5765019912a8ab6882b12db049c7cde635f3a85`; the rule is unchanged at the newer
  `0606ca899ce2995a0c4bc78e660e80832e6e8855` that `publish-release` uses.
  Impact: binstall archives reach the release root only through
  `scripts/hoist_binstall_archives.py`, and any new asset type (`.crate`, a
  provenance bundle) will need the same treatment or a different uploader.

- Observation: the action's `clobber` defaults to `"true"`, so the current
  upload path already overwrites same-named assets on a re-run. Evidence: same
  source, `clobber: str = "true"`. Impact: the reconciliation design must pass
  `clobber: false` explicitly and verify existing assets itself; leaving the
  default is the "clobber alone is not reconciliation" failure the mission
  forbids.

- Observation: `runner_placement_invariants.REQUIRED_RUNNER_ASSIGNMENTS` is
  compared for exact equality, but it enumerates named jobs, so adding a job
  does not break it — while renaming `release` would break three suites.
  Evidence: `has_required_runner_assignments` compares
  `dict(assignments) == REQUIRED_RUNNER_ASSIGNMENTS` over a hand-built dict.
  Impact: the existing `release` job keeps its name; new jobs are additive.

- Observation: `actionlint` does not lint composite action files at all.
  Evidence: `rhysd/actionlint` issue 401; `make github-actions-lint` passes
  `.github/workflows` only, and the existing contract test pins that recipe.
  Impact: Cuprum #487 cannot be closed here by adding a flag to actionlint. A
  separate check is required, and the plan must not claim actionlint validates
  composite actions.

- Observation: `release.yml` never un-drafts the release it creates.
  Evidence: the only `gh release` invocations in the repository are
  `gh release view` and `gh release create --draft`. Impact: "publish GitHub
  Release" is new behaviour, not a preserved one, and the runbook must say so.

- Observation: **the repository root holds a tracked `cyclopts/__init__.pyi`
  stub, and a namespace-package `cyclopts/` directory with no `__init__.py` wins
  `import cyclopts` over no package at all but loses to an installed one** —
  so a source module that uses Cyclopts at import time fails to import under the
  `test-release-publication` recipe but succeeds under
  `test-workflow-contracts`'s. Evidence: four probes at the rebased head.
  `cyclopts.__file__` is `None` and `hasattr(cyclopts, "App")` is `False` under
  `uv run --no-project --python 3.14` (no Cyclopts installed), and a probe
  module doing `from cyclopts import App` at module scope fails collection with
  `ImportError: cannot import name 'App' from 'cyclopts' (unknown location)`
  under the recipe's exact flags. Adding `--with 'cyclopts>=4.25.3,<5'` — which
  `test-workflow-contracts` already carries — makes the same probe pass, and
  `cyclopts.__file__` then resolves to the installed package. The stub is
  tracked since `d91ebb49c` (#848) and is an ancestor of the rebase boundary,
  so this predates the branch. Impact: **Stage C's composite actions must not
  add a Cyclopts module-scope `App` to any `SOURCE_MODULES` entry**, because
  `test-release-publication` pins its `--with` list exactly and does not
  install Cyclopts. The two viable shapes are a stdlib-only `argparse` CLI (the
  convention the six sibling `*_cli.py` modules already follow) or adding
  Cyclopts to the recipe's `--with` list *and* to `REQUIRED_RECIPE_FRAGMENTS` in
  `release_publication_test.py`, which is a deliberate contract change rather
  than a free choice. The `--no-project` flag is what makes this reachable:
  `uv run --script` on a PEP 723 file runs with the *script's* directory first
  and installs the declared dependencies, so a script invoked that way resolves
  the real Cyclopts either way.

- Observation: **a PEP 723 inline-metadata block still applies under
  `uv run --no-project`, so the two invocation shapes are not distinguished by
  the absence of a project.** Evidence: a probe script carrying
  `#!/usr/bin/env -S uv run --script` plus a `# /// script` block and importing
  Cyclopts ran green — `Installed 8 packages in 8ms`, then the expected output
  — under `uv run --no-project --python 3.14 probe.py`, and exited `0`. The
  same file with the block stripped (`sed '1,6d'`) failed with
  `ModuleNotFoundError: No module named 'cyclopts'` under the identical
  command, so the block, not the project state, is what supplied the
  dependency. Impact: Stage C may invoke a stdlib-only CLI under
  `uv run --no-project --python 3.14`, matching `release.yml`'s existing hoist
  steps and the six sibling CLI modules, and that form is unaffected by the
  preceding observation; the PEP 723 form is the alternative and needs the
  contract change described above if the module is ever doctested.

- Observation: PR #780 is open, draft, and `CHANGES_REQUESTED`, and it adds
  jobs named `release-candidate`, `downstream-canaries`, and
  `downstream-canaries-windows`, plus a `candidate-ref` workflow input.
  Evidence: `gh pr view 780`. Impact: this plan must not reuse those job names,
  must not depend on that branch, and should say so in the PR so a reviewer is
  not surprised by the collision on merge.

- Observation: `cargo install` is forbidden anywhere in a workflow **or any
  composite action**, by
  `tests/workflow_contracts/cache_ownership_test.py:test_workflows_do_not_reintroduce_source_tool_builds_or_stale_providers`,
  which scans `.github/actions/**/action.yml` as well as the workflows. The
  detector is a tokenizer, not a substring search, so it survives
  `cargo  install`, `cargo +nightly install`, and a folded line continuation,
  and it resolves ambiguity toward detection on purpose. Evidence: probed
  directly —
  `source_build_commands("cargo install netsuke-build --version 0.1.0-beta3")`
  returns `["netsuke-build --version 0.1.0-beta3"]`, a non-empty result, which
  is the failure condition. `cargo publish` and a curl-pipe-tar are not
  matched. Impact: a "consumer smoke" cannot be a `cargo install`. It must
  instead fetch the published archive by its public URL, verify the digest,
  extract, and run the binary — which is also the stronger test, because it
  exercises the exact bytes and the exact download path a `cargo-binstall` user
  takes. This is why the plan specifies digest-checked archive extraction
  rather than an install.

- Observation: three existing tests glob every composite action —
  `cache_write_policy_test.py` (only those *with* a cache save),
  `cache_ownership_test.py` (the source-build policy above), and
  `ci_mdtablefix_installer_test.py` (searches for a retired path string) — so
  each new action under `.github/actions/` is subject to those policies
  automatically, with no inventory edit needed. Evidence:
  `grep -rn 'rglob("action.yml")' tests/` returns exactly those three files.
  Impact: new composite actions inherit the cache and source-build policies by
  construction. That is the desired direction, and it means the plan's Stage D
  does not need to register new actions in those suites. It does, however, mean
  a new action must not *save* a cache (the key step would then need an
  `IS_TRUNK_PUSH` env entry), which the plan already respects.

- Observation: #834 (`24293a1b`, "Exercise release staging in dry runs")
  landed on `main` on 2026-10-10 and built much of the scaffolding this plan
  designs — under different names, in a different shape, and without any
  registry publication. It adds 27 files and 3554 lines, including
  `tests/workflow_contracts/release_publish_path_permissions_test.py`,
  `release_publish_path_mutations.py` and its test,
  `release_publish_path_ contract.py`, an `actions_expression_evaluator.py`, and
  `scripts/release_operation_observability.py`. It also splits API-bound
  publication out of staging into a new `publish-release` job holding
  `contents: write`. Evidence: `git show --stat 24293a1b`; the permissive job
  header is at `release.yml:540` on `main`. Impact: Stage B must extend these
  modules rather than introduce parallel ones. **Corrected 2026-10-12:** this
  entry first concluded that Stage C must add the registry exchange *inside*
  the existing `publish-release` job. That job holds `contents: write`, so the
  exchange would have put a registry credential and GitHub release-write
  authority in one job — the split LEM-5 exists to forbid. The exchange is a
  separate `publish-crates-io` job (see `Plan of work`), and `publish-release`
  keeps its name, its scope, and its steps unchanged. The plan's own
  permission-separation obligation (EP-M3) is therefore partly already
  discharged by a test that exists; the work is to make the crates.io exchange
  satisfy it, not to write the separation from scratch.

- Observation: `docs/terms-of-reference.md` (Draft v0.2) was added by #786
  after this plan was drafted, and does state release-adjacent criteria —
  crates.io downloads as the first external adoption signal, and the three
  release-admission canaries as a dependency. Evidence: ToR sections 7.3 and
  8.3, and open question Q12. Impact: this plan's earlier claim that no ToR
  covers release is now false and has been corrected in `Conformance basis`.
  The ToR governs the publication *outcome*; it still does not prescribe
  publication mechanics, which remains this plan's own design.

- Observation: `make test` is red on this host at the frozen head `6483aa7c`,
  but by nextest's per-test allowance rather than by an assertion, and the
  victim changes between runs. Evidence: run 1 timed out on
  `netsuke-build::build_module_slice_ui_tests`
  `production_build_module_slice_has_expected_boundary` at 300.013 s; run 2
  timed out on `netsuke-build::command_env_ui_tests`
  `cli_configuration_fixture_compiles` at 300.014 s — and run 2's victim is run
  1's *pass*, in 197.755 s. Run standalone outside nextest's cap,
  `cli_configuration_fixture_compiles` passes in 230.79 s with exit 0. Both
  tests are already serialized in `.config/nextest.toml`'s
  `nested-cargo-builds` group (`max-threads = 1`, lines 56 and 66), so the
  contention is not intra-run: the shared `build-limits` pool reported
  `capacity 4, held 4` with waiters from another repository's worktree and
  `the managed sccache server is unreachable; compiling without the cache`
  throughout. Impact: the reds are a property of this host's compile
  contention, not of the branch. The branch's entire diff against `main` is one
  Markdown file, which cannot influence a Rust test's runtime; continuous
  integration runs the same suite on a warm cache with its own build budget.
  The residual risk is that a real regression in one of these two tests would
  be indistinguishable from contention here, so the diff-scope argument — not
  the timeout message — is what establishes the red as environmental.

- Observation: **RETRACTED — new `release.yml` jobs need NO runner
  registration.** An earlier revision of this entry asserted that every added
  job must be registered in two places or the gate reds. That was an inference
  from a contract's *shape*, and reading the object the contract actually
  constrains falsifies it. `has_required_runner_assignments` does compare for
  exact equality, but the gatherer `_checked_in_runner_assignments` collects
  only the **eleven** `DIRECT_RUNNER_SOURCES` entries plus matrix-derived and
  reusable-workflow entries. A job named in neither table is never collected,
  so it can never appear in the gathered map — and the equality is still
  satisfied, because it is only ever compared against what was gathered. An
  unregistered job is INVISIBLE to this contract, not failing it. The
  repository's own precedent settles the question rather than leaving it to
  argument: `publish-release`, added by PR #834 and live on `main`, appears in
  neither table, and `make test-workflow-contracts` is green.

  ```plaintext
  grep -c 'publish-release' tests/workflow_contracts/runner_placement_invariants.py
    -> 0
  grep -c 'publish-release' tests/workflow_contracts/runner_placement_test.py
    -> 0
  ```

  No suite enumerates the `release.yml` job set:
  `grep -rn 'required_actions\|JOB_NAMES\|expected_jobs\|job_names'
  tests/workflow_contracts/*.py`
  returns no matches. The plan's `Risks` entry has been corrected to match.

  What **does** apply to a new job is per-job `permissions:` —
  `release_publish_path_permissions_test.py:13-24` loops over every job
  asserting `"permissions" in job` and separately requires the workflow-level
  `permissions` to be `{}`. That is the one real registration step Stage C owes.
  `timeout-minutes` is not required of these jobs: the timeout suite is
  parameterized over named Ubicloud jobs, none of them in `release.yml`.

- Observation:
  `github_actions_validation_test.test_makefile_runs_both_github_actions_linters`
  pins the `github-actions-lint` recipe's **first line** to
  `$(YAMLLINT) --config-file .yamllint.yml .github/workflows` and its **last
  line** to `$(ACTIONLINT)`, and models the target with strict CmdMox mocks
  that call `--with_args(...)` exactly once per tool. Evidence: read from
  `tests/workflow_contracts/github_actions_validation_test.py:79-90` and
  `:230-235`. Impact: Stage D's requirement that `github-actions-lint` also lint
  `.github/actions/*/action.yml` with yamllint **cannot** be satisfied by
  appending a second `yamllint` line — `recipe[-1]` would become the new line
  and the exact-args mock would see an unexpected second call. It cannot be
  satisfied by extending the first line's path list either, because that string
  is asserted verbatim. The composite-action lint therefore needs its own
  Makefile target, wired into `lint` alongside `github-actions-lint`, with
  `github_actions_validation_test.py` updated in the same change-set to
  describe the new shape. This is a real constraint discovered now rather than
  at the first red gate.

- Observation: Stage C must be authored **after** the rebase onto `main`, and
  the rebase is now done. The `local_reference_*` family — eleven contract
  modules — exists only on `main`, and it constrains exactly the artefact Stage
  C adds (every local composite action must be referenced from a workflow,
  Makefile, or test source, or carry a reasoned exemption). Authoring the
  actions before the rebase would have meant writing against a contract that
  was not present and then being judged by it afterwards. Evidence: before the
  rebase the probe below reported one uncovered item,
  `.github/actions/cross-build-macos`, a dead action that `main` had already
  deleted in PR #859 while this branch still carried it; after the rebase the
  count is zero.

  ```plaintext
  before rebase:  inventory size 71,  uncovered total 1
                  ['.github/actions/cross-build-macos']
  after  rebase:  inventory size 85,  uncovered total 0
                  all 7 new release scripts inventoried=True covered=True
                  control (a script that cannot exist): False
  ```

  Impact for Stage C, measured from `local_reference_coverage_test.py`:
  `EXEMPTIONS` is **empty on main**, so the contract is satisfied entirely by
  real references and an exemption is the wrong tool —
  `test_exemptions_are_not_stale` deletes one the moment the item gains a real
  reference. Coverage resolves **indirectly**: `covered_items` makes a covered
  action's own manifest a source, so the intended chain is `release.yml` →
  `uses: ./.github/actions/<name>` → that action's `action.yml` naming
  `scripts/<script>.py`. Each new action must therefore be referenced from
  `release.yml`, and any script it invokes must be named in its own manifest.
  `test_expected_indirect_repository_items_are_covered` pins a known set of
  indirectly-covered items, so the mechanism is load-bearing rather than
  incidental.

- Observation: test collection differs by directory, and the difference decides
  where a new guard may live. `make test-workflow-contracts` passes
  `tests/workflow_contracts` as a **directory**, so a new contract module there
  is auto-collected with no registration — which is why the LEM-5 suite belongs
  there. The same recipe names `scripts/tests/test_*.py` as a **hand-enumerated
  list**, so a new module under `scripts/tests/` must be named in a recipe or
  `release_publication_test.py`'s orphan check reds. Evidence:
  `Makefile:247-262`. Impact: if the composite-action guard is written as a
  `scripts/tests/` module it must be registered; the LEM-5 suite and the
  mutation module need not be.

- Observation: the existing publish-path contract suite is a **scenario
  evaluator**, not a name-matching assertion set.
  `release_publish_path_scenarios.py` declares eleven `Scenario` rows over the
  mode/event/needs matrix, `release_publish_path_contract.py` renders the
  `metadata` outputs and job guards through the repository's own expression
  evaluator, and `release_publish_path_publication.py` asserts
  `publish-release`'s reachability per row. Evidence: read in full. Impact: two
  consequences for Stage B. First, the new `release_publication_test.py` must
  *extend* this scenario table rather than duplicate it — a parallel table
  would drift and the two would disagree about what `should_publish` means.
  Second, adding a job that `publish-release` needs, or changing `metadata`'s
  outputs, will change these rows' expected values and must be done
  deliberately.

- Observation: the leading-zero rule was written too broadly in the first
  attempt, and the mistake was in the *documentation* as well as the test rows.
  The module claimed `0.1.0-beta04` "is not a semantic version" and that this
  was "the rule that matters". Cargo parses versions with the `semver` crate,
  which pins `1.0.28` in this repository's `Cargo.lock`, and its identifier
  rule refuses a leading zero only when the identifier is *entirely* numeric —
  the `!segment_has_nondigit` guard at
  `~/.cargo/registry/src/index.crates.io-*/semver-1.0.28/src/parse.rs:242-248`.
  So `0.1.0-01` is invalid but `0.1.0-beta04` is valid: the padding sits inside
  an alphanumeric identifier, which the grammar permits. Evidence: the reference
  `semver` npm package agrees on a ten-case corpus, and it is the stricter
  oracle for the tag-prefix and whitespace rows because it trims and accepts
  `v0.1.0`, which this grammar refuses on purpose. Impact: the module docstring
  and three test rows were corrected. The defensible claim is the one the plan
  needs anyway — `0.1.0-beta04` and `0.1.0-beta4` are *different identifiers*
  that no ordering comparison unifies (`semver.eq` is false, `compare` is -1),
  so the tag must spell whichever the manifest declares. The byte-equality rule
  was never wrong; one of its justifications was. The general lesson is that a
  rule stated as "the leading-zero rule" must be stated with its exact guard,
  because the intuitive version refuses versions the registry accepts.

- Observation: `ambrleaks`, the last stage of `make lint-python`, reports
  findings for an explicitly named `.py` path while passing over the same file
  reached through a directory walk. Evidence: `ambrleaks scripts/` exits 0 and
  prints nothing; `ambrleaks scripts/tests/test_release_candidate.py` exits 1
  with four `snapshot-hex` findings against that file's 40-character commit
  fixtures. The Makefile passes directories
  (`PYTHON_SOURCES = .github/scripts scripts tests/workflow_contracts`), so the
  gate sees only `.ambr` files and the findings are invisible to it. Impact:
  this is the same explicit-path hazard as `mdtablefix`'s in the Markdown gates
  — naming a path bypasses the tool's extension filter — and it means a
  per-file invocation of a directory-oriented tool proves nothing about what
  the gate will say. The four findings are in test fixtures whose values are
  public commit hashes, not secrets; the gate is nonetheless silent about them,
  and this is recorded rather than papered over.

- Observation: the 400-line cap and `too-many-locals` pull against each other
  when a function is decomposed by extraction, and satisfying one naively
  breaks the other. Evidence: `scripts/release_inventory.py` carried 11 locals
  in `validate_inventory` against the configured limit of 10. The
  house-preferred fix is extraction rather than shaving a variable, so two
  stage helpers were extracted — and the file went from 361 to 421 lines,
  breaching the 400-line cap the plan's constraint 13 requires. Trimming
  docstrings back to 397 lines then produced a *second*, previously absent
  finding: `docstring-extraneous-exception` (DOC502) on `validate_inventory`,
  because four of the six exception classes it had documented are now raised by
  its helpers rather than in its own body. Impact: a decomposition fix is not
  complete until the linter has been re-run on the decomposed shape, since
  moving code across a function boundary changes what each docstring may
  legitimately claim. The resolution keeps the two locally-raised classes in the
  `Raises` block and names the remaining four in prose, with the helper that
  raises each — which is what a caller needs, since none is caught here.

- Observation: **the released binstall archives do not exist.** All six
  `netsuke-build-{version}-{target}.tar.gz` archives are absent from every
  published release, while their `.sha256` sidecars are present. Evidence, all
  read on 2026-10-12:
  `gh release view v0.1.0-beta4 --repo leynos/netsuke --json assets` lists 29
  assets; six end in `.tar.gz.sha256` (119–125 bytes each) and **zero** end in
  `.tar.gz`. `v0.1.0-beta3` and `v0.1.0-beta2` show the identical shape, and
  `v0.1.0-beta1` has neither. The mechanism is read from the action's own
  source at the pin `release.yml` uses: in
  `.github/actions/upload-release-assets/scripts/upload_release_assets.py` (pin
  `0606ca89…`), `_is_candidate` accepts a file named `{bin_name}`,
  `{bin_name}.exe`, or `{bin_name}.1`, anything ending `.sha256`, or any path
  whose `path.suffix` is `.deb`, `.rpm`, `.pkg`, or `.msi` — and
  `Path("x.tar.gz").suffix` is `.gz`, so an archive is never a candidate while
  its sidecar always is. `scripts/hoist_binstall_archives.py` therefore moves
  the archives to the `dist/` root correctly, and the uploader then discards
  them by extension. Impact: the user-visible consequence is confirmed by
  request, not inferred — `curl -L` on the exact URL the
  `[package.metadata.binstall]` template in `Cargo.toml` resolves returns
  **404**, while the same URL with `.sha256` appended returns **200**. So
  `cargo binstall netsuke-build`, documented in `README.md:72` and
  `docs/users-guide.md:29`, cannot install any released version, and the
  sidecars that do upload direct a user to bytes that are not there. This is a
  pre-existing defect on `main`, not one this branch introduces, and it is
  exactly what LEM-2 and the `verify-release-inventory` job exist to catch: the
  previous plan statement that "binstall archives reach the release root only
  through `scripts/hoist_binstall_archives.py`" was true and insufficient —
  reaching the root does not make them uploadable. Three suites pin the hoist,
  the staging config, and the binstall metadata; none of them observes the
  published asset list, which is where the loss happens.

- Observation: **the uploaded asset names are not the names the action
  computes, so 86 candidates collapse onto 29 bare basenames.** This is a
  second, independent fault stacked under the `.gz` one, found on 2026-10-19 by
  reconstructing the beta4 `dist/` from its six workflow artefacts and
  replaying the pinned uploader over it. Evidence: the reconstruction holds
  **86** candidate paths *excluding* the six archives and **92** *including*
  them, which reproduces the run's own `Successfully processed 86 asset(s)`
  (`0_release.txt`, `2026-09-27T00:17:33.3478165Z`) and confirms the `.gz`
  exclusion independently. Those 86 paths yield 86 *distinct* values from
  `_resolve_asset_name`, and **zero** collisions, so `_register_asset` never
  fires — yet their 86 computed names collapse onto only **29** distinct bare
  basenames, which is exactly the number `gh release view` reports. The
  mechanism is in `gh` itself: at `v2.97.0`, `pkg/cmd/release/shared/upload.go`
  splits `path#label` at the first `#`, uses the left side as the file to read
  and `Name: fi.Name()` — the source file's basename — as the asset name,
  carrying the right side as a display `Label` only. So `_resolve_asset_name`'s
  `linux-amd64__netsuke_linux_x86_64/netsuke` descriptor uploads as `netsuke`.
  With `clobber` defaulting to `"true"`, the last source in `sorted()` path
  order wins each name. Impact: the damage is not limited to the missing
  archives, and it is worse than a missing asset because it is silent. The
  published `netsuke` at beta4 is not the Linux binary but
  `netsuke_windows_aarch64/.../completions/bash/netsuke`, and the published
  `netsuke.exe` (`7f8606b6682ddfcd…`) is the **aarch64** build rather than the
  distinct amd64 bytes (`b6f4cfda6e341e5c…`) staged under `windows-x86_64`.
  `netsuke.sha256` records `0b4da8d4…`, the digest of the *published* file, so
  the sidecar verifies and the release is internally consistent while shipping
  the wrong bytes — exactly the failure a digest-only check cannot see, and the
  one INV-1 exists to prevent. The archive sidecars (`a852c3a5…` for the x86_64
  Linux target) do publish correctly and point at archives that return **404**.
  Two consequences for this plan: the statement corrected above, that the
  action "namespaces any file not already at the root", is false, and the
  declared-name producer must mirror `_is_candidate` **and** the basename
  semantics — modelling `_resolve_asset_name` would reproduce the defect in the
  oracle. The uploader is byte-identical at both pins (`diff` of
  `a5765019912a8ab6882b12db049c7cde635f3a85` and
  `0606ca899ce2995a0c4bc78e660e80832e6e8855` printed nothing), so the dry-run
  lane shares the defect.

  A **third** renaming happens after that, inside GitHub rather than in the
  uploader: asset filenames containing special or non-alphanumeric characters,
  and leading or trailing periods, are rewritten by GitHub itself. The staged
  `netsuke-0.1.0~beta4-1.x86_64.rpm` (digest
  `4a94f0be3cafb8a0621514b7c046a41a8eb959ed3d50be73bbf5a8ad34501d90`) publishes
  as `netsuke-0.1.0.beta4-1.x86_64.rpm` — the tilde becomes a dot, the bytes do
  not change. The same substitution applies to all four RPM/DEB assets:
  `netsuke_0.1.0~beta4-1_amd64.deb` (`1a5e283f…`) publishes as
  `netsuke_0.1.0.beta4-1_amd64.deb`, and the aarch64/arm64 pair (`c68ff1fe…`,
  `17a4d115…`) likewise; each published digest equals its staged digest. The
  payload keeps the tilde (`Version: 0.1.0~beta4-1` in the extracted `.deb`
  control file), so only the asset name is transformed, and the MSI and PKG
  names contain no such character and pass through unchanged. Consequence:
  staging directly into `dist/` does not make staged and published names agree,
  so the declared-name producer must model this normalization as a third
  transformation on top of `_is_candidate` and `fi.Name()`. An oracle that
  compared staged names to published names would report a `MissingAssetError`
  for each RPM/DEB beside an `UnexpectedAssetError` for its published twin — a
  false alarm on a correct release. The `[package.metadata.binstall]` `pkg-url`
  template is unaffected, because the archive names it interpolates contain no
  character GitHub rewrites. Date/Author: 2026-10-19, implementation agent.

  The replacement is **narrower than the documentation's wording suggests**.
  GitHub's sentence covers "special characters, non-alphanumeric characters,
  and leading or trailing periods"; over the beta4 reconstruction the only
  character outside `[A-Za-z0-9._-]` anywhere in a staged name is the `~` in
  the four RPM/DEB names, and each of the four published names is exactly its
  staged basename with `~` mapped to `.`. So the producer's rule is stated as
  the observed mapping over the staged names this repository produces, not as a
  general `gh` sanitizer: a broader "strip everything unusual" rule would
  invent names for characters that never occur, and an oracle that predicts a
  name no upload produces is as wrong as one that predicts the staged name.

  One more measurement from the same reconstruction, because it constrains what
  the inventory can honestly verify: **44 distinct normalized basenames carry
  130 staged files, and 29 distinct names carry all 86 candidates.** Under the
  uploader's real naming rule the release therefore presents 29 names where the
  tree holds 130 files, and the collisions are not marginal — `netsuke` has 12
  sources, `netsuke.sha256` 10, and `netsuke.1` 4. Replaying `clobber: true` in
  sorted path order gives the published set exactly, and it is the same set
  `gh release view` reports. LEM-2's exact set equality is therefore
  satisfiable **only after** the declared-name producer applies this reduction:
  a declaration built from the staged tree would name 130 assets and be
  rejected against the 29 GitHub holds. This is the same defect the producer
  exists to prevent, one layer up, and it is why the producer is a separate
  module with its own suite rather than a comprehension inside the verifier.

  **That reduction is a defect, not a contract, and modelling it would make the
  oracle certify the wrong release.** The beta4 release is not merely missing
  its archives: `netsuke` publishes as 11 132 bytes of bash completion with
  digest `0b4da8d4…`, `netsuke.exe` as the **aarch64** binary (`7f8606b6…`), and
  `netsuke.1` as the macOS x86_64 man page — each the last source in sorted
  path order. So the verifier must not adopt the uploader's basename rule as
  its declaration; doing so would produce a green check over exactly the
  release INV-1 exists to fail. The producer therefore declares the names a
  **correct** upload would publish, and the inventory check's job is to
  disagree with beta4's 29 names rather than to reproduce them. The separate
  decision — which staging layout the release should present so that a correct
  upload is possible at all — is recorded in the Decision log and is a change
  to the release path, not to the oracle.

- Observation: the docstring examples this plan counts as LEM-1 and LEM-2
  evidence were **inert** when they were first written. The seven collected
  doctest **items** in `release_candidate.py` and `release_semver.py` (four and
  three) existed, read correctly, and passed whenever they were run by hand —
  but no target collected them, so the "7 doctests" reported above was a
  measurement of nothing. Evidence: the `test-release-publication` recipe named
  only the three test modules and carried no `--doctest-modules`, so pytest
  collected test functions alone. Impact: this is Cuprum #499's orphaned-suite
  hazard one layer down. A suite nothing runs is the failure the wiring
  contract exists to catch; a *doctest* nothing runs is the same failure inside
  a file that is already wired, and it is harder to see because the module is
  present in the recipe and green in every gate. The repair is the recipe's
  source-module arguments plus `--doctest-modules`, pinned by `SOURCE_MODULES`
  in `tests/workflow_contracts/release_publication_test.py`, and proved live by
  mutation: deleting the flag turns **one** contract test red —
  `test_the_target_recipe_pins_its_interpreter_and_modules`, on its single
  `REQUIRED_RECIPE_FRAGMENTS` assertion. Observed, not derived: with the line
  `-p no:cacheprovider --doctest-modules` reduced to `-p no:cacheprovider` in
  that recipe alone, the module reports `1 failed, 11 passed`; restored,
  `12 passed`. The other candidates do not move — the source-module check reads
  the *source* text for `>>>`, which deleting a recipe fragment does not
  change, and the module-existence check reads the test-module directory. The
  same mutation exposed a second, narrower gap — `release_inventory.py` carried
  no example at all, so naming it on the command line would have been vacuous;
  `validate_inventory` now has one, which is why that module grew from 397 to
  400 lines. A third finding, deliberately **out of scope** for this branch:
  fifteen further inert doctest **lines** pre-exist on `main` in
  `validate_coverage_artifact.py` (6 lines, 4 items), `kani_proof_scope.py` (4
  lines, 2 items), `cargo_package_metadata.py` (2 lines, 2 items), and
  `.github/scripts/verify_python_baseline.py` (3 lines, 2 items). The counts are
  `>>>` lines, not collected items; the two units differ and this bullet
  states which is which on both figures. They are recorded here so they are not
  lost, but this deliverable owns only its own new modules, and repairing
  modules it did not touch would enlarge the diff past its scope. Date/Author:
  2026-10-15, implementation agent.
- Observation: **`.yamllint.yml` cannot be reused unchanged to lint
  `.github/actions`.** Evidence: the config is `extends: default` with
  `document-start: {level: error, present: true}`, and four of the five
  existing manifests begin with `---` while
  `windows-msi-upgrade-validation/action.yml` begins with `name:` and carries
  no document start. `github-actions-lint` lints only `.github/workflows`, so
  that action is currently unlinted and the omission is invisible. Impact: the
  Stage D composite-action target either supplies its own policy or the
  existing action gains the marker; the marker is the smaller change and is
  done in this branch, because a target whose first act is an exemption is the
  shape the mission forbids. Date/Author: 2026-10-20, implementation agent.

- Observation: **the same sweep has a second, separate blocker in
  `windows-gate-cache/action.yml`.** Evidence: running the Stage D command
  directly — `yamllint --config-file .yamllint.yml .github/actions` — exits `1`
  today with two findings, not one:
  `windows-gate-cache/action.yml:233 too many blank lines (1 > 0)` alongside the
  `document-start` finding recorded above. The file ends at line 232 and
  carries a trailing empty line; under `extends: default` the
  `empty-lines: max-end: 0` rule rejects it. Both findings are pre-existing on
  `main` — neither file is modified by this branch — so this is a latent defect
  in the repository, not drift introduced here. Impact: repairing only the
  missing marker would leave the new target red on its first run, and the
  obvious next move from there is an exemption for the file that trips it,
  which is exactly the weakening the mission forbids. Both fixes are therefore
  in scope: add `---` to `windows-msi-upgrade-validation/action.yml`, and strip
  the trailing blank line from `windows-gate-cache/action.yml`. Verified by
  probe rather than assumed: applying both edits to a scratch copy of
  `.github/actions` makes the sweep exit `0`. Date/Author: 2026-10-21,
  implementation agent.

- Observation: **this plan carries two different "a blanket sweep is green"
  claims about two different invariants, and they must not be conflated.**
  Evidence: the Stage C paragraph beginning "Earlier revisions of this plan
  justified a path-scoped rule" concerns the **expression-site** rule — no
  `${{ }}` inside a `run` block — and its claim is correct: re-measured after
  the rebase, the five existing manifests hold **68** expression sites (7, 17,
  0, 26, 18 for `kani-cache`, `linux-gate-cache`, `memory-sampler`,
  `windows-gate-cache`, `windows-msi-upgrade-validation`), matching the plan's
  per-file figures exactly, and **0** of them sit in a `run` block. The
  **yamllint** sweep over the same directory is a separate invariant and is
  **not** green — it exits `1` on the two defects above. Impact: a reader who
  carries "green today" from the first claim to the second concludes the Stage
  D target needs no repairs, implements it, watches it red, and reaches for an
  exemption on its first run. The rule to apply is that each sweep's colour is
  established by running that sweep. Date/Author: 2026-10-21, implementation
  agent.
- Observation: `runner_placement_invariants.REQUIRED_RUNNER_ASSIGNMENTS` is
  an explicit table of named jobs, so the five new jobs in `release.yml` are
  unconstrained by it and may declare `ubuntu-latest`; the same holds for
  `cache_ownership_test.test_each_job_calls_only_its_own_lane_cache_action`,
  which is table-driven. Date/Author: 2026-10-20, implementation agent.

- Observation: **the packaging lane's only rehearsal defect was invisible to
  every local suite and appeared on the first hosted pull-request run.**
  Evidence: `release / package-crate` failed with a refusal quoting the merge
  ref —
  ``tag '903/merge' is not a release tag: expected 'v' followed by a
  semantic version such as 'v1.2.3'`` —
  and exit `2`. The mechanism is that `release.yml` is a reusable workflow
  invoked both from `push: tags: v*.*.*` and from `pull_request`, and on the
  latter `github.ref_name` resolves to the synthetic merge ref rather than a
  tag; the packaging step forwarded it unexamined. The existing jobs tolerate
  the same event because `upload-release-assets` receives
  `dry-run: needs.metadata.outputs.should_publish != 'true'`, so the mode is
  already computed in this workflow — the packaging step simply did not receive
  it. Every local suite passed while the defect was live, including both the
  packaging CLI's unit tests and the workflow contracts, because all of them
  supply a literal tag: the failing value comes from the hosting event and
  cannot be produced by a local string literal. The rule to carry forward is
  that a workflow expression whose value depends on the *event* needs a
  contract that evaluates it in each trigger's context, not a test that
  supplies the happy value. Date/Author: 2026-10-23, implementation agent.

- Observation: **`cs delta` does not analyse untracked files, so a clean delta
  over an unstaged new module is not evidence about it.** The delta reported
  `total-modified-file-count: 42` / `checked-file-count: 35` both before and
  after `release_asset_names.py` and `release_package_formats.py` were created,
  which reads as "clean" but actually means "not looked at":
  `git diff origin/main` cannot see a path Git is not tracking. After
  `git add`, the same command reported `44` modified and `37` checked — exactly
  the two new files. Impact: a CodeScene verdict obtained before staging is a
  verdict about a different file set. `cs check <file>` also accepts only its
  first path argument, so `cs check a.py b.py` silently scores `a.py` alone;
  score new files one at a time, then stage them and re-run the delta as the
  real check. Date/Author: 2026-10-23, implementation agent.
- Observation: **the 400-line module cap is enforced by a lint that a targeted
  test run will not surface.** `pyproject.toml` disables nearly all of Pylint
  and then explicitly re-enables `too-many-lines` at `max-module-lines = 400`,
  so a file of 464 or 460 lines fails `make lint`'s Pylint stage while every
  behavioural suite over the same file passes. Confirmed by running the pinned
  Pylint directly (`C0302: Too many lines in module (454/400)`), which is the
  cheap way to check a suspected overage without waiting for the full gate. The
  cap forced genuine decomposition here rather than docstring trimming: the
  first attempt at trimming prose moved 464 to 454 and no further, because the
  modules genuinely owned two responsibilities. Date/Author: 2026-10-23,
  implementation agent.

## Decision log

- Decision: keep the existing `release` job's name and identity.
  Rationale: `runner_placement_test.py`, `release_dry_run_smoke_test.py`, and
  `release_workflow_hoist_test.py` each name it; renaming buys nothing and
  churns three suites. Date/Author: 2026-09-27, planning agent.

- Decision: extract every new step body into a composite action rather than
  growing `release.yml` inline. Rationale: the repository's own convention,
  stated in `docs/developers-guide.md:1574-1575` for the lane cache steps, and
  the only way to keep the job graph readable while making the sequence
  explicit. **Not** a 400-line cap: no contract enforces one on workflow YAML,
  and `release.yml` is 688 lines on `main`. A local `action.yml` resolves from
  the checked-out commit, so it carries no pin — `action_references.py:66-69`
  in fact refuses a `./`-prefixed action for exactly that reason, meaning
  `uses: ./.github/actions/<name>` needs no SHA. Date/Author: 2026-09-27,
  planning agent; rationale corrected 2026-10-15.

- Decision: do not adopt Cuprum's filename-first identity rule.
  Rationale: crates.io keys the trusted-publisher registration on the Cargo
  package name, not on a distribution filename; copying the PyPI rule would be
  an unverified transplant. Date/Author: 2026-09-27, planning agent.

- Decision: no Cuprum runtime dependency.
  Rationale: the reusable part is a decomposition, not code. Python here is
  stdlib-only and runs under `uv run --no-project`. Date/Author: 2026-09-27,
  planning agent.

- Decision: skip the `typos.toml` refresh commit on rebase rather than
  replay it as an empty commit. Rationale: `main` reached the identical blob
  (`d32137d0`) through #892 and moved `TYPOS_CONFIG_BUILDER_VERSION` to v0.1.3,
  so the commit's content is already present and the branch adds nothing by
  keeping it. The commit was non-code generated churn whose own message
  anticipated being dropped. Verified by blob comparison against `origin/main`,
  not by the rebase's own "empty" verdict alone. Date/Author: 2026-10-11,
  planning agent.

- **SUPERSEDED 2026-10-12** by the decision recorded further down this list
  that begins "SUPERSEDES the 2026-10-11 decision above": the crates.io OIDC
  exchange is declared in its own job, not inside `publish-release`, because
  LEM-5 forbids one job holding both a registry credential path and GitHub
  release-write authority. Retained for the history of why the placement was
  first made and why that reason was overridden. Decision: build the crates.io
  exchange into #834's existing `publish-release` job instead of adding a new
  publication job. Rationale: #834 already isolated API-bound publication there
  and wrote `release_publish_path_permissions_test.py` to hold the separation.
  A second publication job would duplicate that boundary and split the
  permission story across two places. This supersedes the plan's earlier
  job-layout sketch; the `release` job keeps its name, as previously decided,
  and no third job is introduced. Date/Author: 2026-10-11, planning agent.

- Decision: record the `make test` timeouts as environmental and proceed,
  rather than re-running until green. Rationale: the two failures land on
  different tests, neither reproduces outside nextest's 300 s allowance, the
  pool is saturated by another repository, and the branch's whole diff is one
  Markdown file. Re-running until it happens to pass would produce a green tick
  whose value cannot be distinguished from the one already recorded, and would
  hide that this host cannot run these two tests reliably under contention. The
  honest record is the two-run evidence above, including the residual risk.
  Date/Author: 2026-10-11, planning agent.

- Decision: **SUPERSEDES the 2026-10-11 decision above**, the one beginning
  "build the crates.io exchange into #834's existing `publish-release` job".
  That entry is now annotated SUPERSEDED in place. Declare the crates.io OIDC
  exchange in its own job in `release.yml`, not inside #834's existing
  `publish-release` job. Rationale: LEM-5 requires that no job holds both a
  registry credential path and GitHub release-write authority, and the plan's
  own Verification plan states that the registry-exchange job "is the only one
  carrying `id-token: write` **and** `environment: crates-io`". #834's
  `publish-release` holds `contents: write` and runs the draft creation, the
  asset download, the hoist, and the upload — granting it `id-token: write` as
  well would put a publisher credential and release-write authority in one job,
  which is exactly the split LEM-5 exists to prevent, and would contradict
  `release_publish_path_permissions_test.py`'s stated intent that only
  publication holds `contents: write`. **Qualified 2026-10-16:** that intent
  governs which job may hold `contents: write` *for the GitHub release
  publication path*, and it is what forbids the registry-exchange job from
  acquiring it; it is not a claim that no other job may ever hold it. The test
  itself asserts per-job scopes and does not perform set-equality over the
  holders, so the later decision to grant a second, post-publication writer
  (`finalize-github-release`) does not conflict with it — but the test's name
  and docstring would outlive their truth once that job lands, so Stage C
  extends or renames it and the two-writer set is asserted by
  `release_credentials_separation_test.py`. The 2026-10-11 decision was made to
  avoid a competing *publication* job and that concern still stands; the
  resolution is that the new job is a *registry exchange and upload* job, not a
  second GitHub-publication job. `publish-release` keeps its name, its
  `contents: write`, and its steps unchanged. Escalated under the plan's
  `Ambiguity` tolerance on 2026-10-12 (choice between two materially different
  credential-separation semantics) and resolved in favour of LEM-5, which is
  the security invariant rather than a layout preference. Date/Author:
  2026-10-12, implementation agent.

- Decision: record acceptance criterion 5 as a **deviation** rather than
  silently redefining it or restructuring five existing job bodies to chase it.
  Rationale: the criterion's premise is false. `release.yml` is 688 lines on
  `main`, `ci.yml` is 710 and `build-and-package.yml` is 441, and no contract
  test enforces a 400-line cap on workflow YAML — the 400-line rule reaches
  Python through Pylint's `max-module-lines` (`pyproject.toml:175`) and Rust
  through Whitaker's `module_max_lines`, and the developers' guide applies it
  to workflows by convention only. Meeting the number would require extracting
  `metadata`, all three `build-*` jobs, `windows-native-recipe-smoke`,
  `release-admission-canaries`, `release` and `publish-release` into composite
  actions, which is far outside this plan's scope and would churn six contract
  suites for no security or correctness benefit. Because the deviation is not a
  workaround of a constraint — it is a false premise in an acceptance criterion
  — it is recorded here and reported rather than escalated as `BLOCKED`, per
  the skill's instruction to record a proposed deviation from approved scope.
  Date/Author: 2026-10-12, implementation agent.

- Decision: record the **scope tolerance breach** as an escalated deviation in
  prose, rather than silently rewriting approved scope to fit. Rationale: the
  `Tolerances` section allows up to 30 changed files and 1 500 net changed
  lines. The figure that decides this is the work **excluding this plan**,
  since the plan is itself a changed path and its size churns with every
  revision to it. Measured against the current candidate
  (`git diff --shortstat fa5a4ffa..565d5572`):

  ```plaintext
  excluding this plan:  17 files changed, 3996 insertions(+), 1 deletion(-)
  all paths:            18 files changed, 5679 insertions(+), 1 deletion(-)
  ```

  The file count is inside tolerance (17 ≤ 30) but the line count is not: 3 995
  net excluding the plan, 2.7× the 1 500 budget. The **excluding** figure is
  the one quoted because it is the only one that does not move when this plan
  is revised to record it. Holding the base fixed at `486b876c` and varying
  only the head across this branch's own successive revisions, the two figures
  behave differently:

  ```plaintext
                excluding this plan            all paths
  c09e9e99      3996 insertions(+)             5495 insertions(+)
  e2b9794b      3996 insertions(+)             5651 insertions(+)
  7339ed3c      3996 insertions(+)             5679 insertions(+)
  ```

  The excluding figure is **constant** at `3996 insertions(+)` across all three
  — it does not depend on the plan's length at all. The all-paths figure drifts
  `5495 → 5651 → 5679` over the same head range, entirely from growth in the
  file that reports it. (Quoted against the current base `fa5a4ffa` the
  excluding figure is still `3996 insertions(+)`, so the constancy is not an
  artefact of the old base.) A breach measured with the number that moves when
  you record it is not a measurement. Stage C is still to come and adds five
  composite actions, new `release.yml` jobs, the LEM-5 suite, a
  composite-action guard and a mutation harness — all growth, with no
  offsetting deletions. The breach is structural, not incidental: the required
  six-section design (candidate binding, package verification, short-lived
  auth, recoverable sequence, immutable-state reconciliation, genuine dry runs)
  cannot be delivered as pure Python modules plus contract suites inside 1 500
  lines. Raising the tolerance would be redefining the measurement to fit the
  result, which is why this is recorded as a breach and reported to the user.
  The `AskUserQuestion` seam has failed before (PR #860: six of six attempts
  aborted), so this is escalated in prose rather than through that mechanism.
  Date/Author: 2026-10-15, implementation agent.

- Decision: state the version grammar against the reference implementation
  rather than against the informal SemVer description, and keep byte equality
  as the tag/version rule. Rationale: the informal "no leading zeros" phrasing
  is too broad. Cargo's `semver` crate refuses a leading zero only when an
  identifier is entirely numeric (`!segment_has_nondigit`, `parse.rs:242-248`
  in the pinned `1.0.28`), so a grammar written from the informal rule would
  refuse `0.1.0-beta04` — a version crates.io accepts — and turn the LEM-1
  validator into an obstacle for a legitimate release. The rule that actually
  protects the release is unaffected: `0.1.0-beta04` and `0.1.0-beta4` are
  distinct identifiers that no ordering comparison equates, so a tag that
  spells one must not be satisfied by a manifest declaring the other. Byte
  equality is what refuses that; a parsed comparison would accept
  `0.1.0-beta4+meta` against the tag `v0.1.0-beta4`, because semantic-version
  ordering ignores build metadata. Evidence for both directions is recorded in
  `Surprises & discoveries`. Date/Author: 2026-10-12, implementation agent.

- Decision: name the four remaining jobs `package-crate`, `publish-crates-io`,
  `verify-release-inventory` and `finalize-github-release`, avoiding the four
  names PR #780 already claims. Rationale: the plan forbids reusing
  `release-candidate`, `downstream-canaries`, `downstream-canaries-windows` and
  the workflow input `candidate-ref`, because #780 owns that vocabulary and a
  second workflow reusing it would make the two suites ambiguous about which
  file they constrain. Every new job name also matches the composite action it
  calls, and the fifth action was renamed from `github-release-finalize/` to
  `finalize-github-release/` so that parity holds in both directions rather
  than only one. Date/Author: 2026-10-15, implementation agent.

- Decision: land `release_permissions_test.py` in the **same commit** as the
  `publish-crates-io` job, and do not write `release_modules_wired_test.py` at
  all. Rationale: the first would otherwise fail the orphan guard the plan
  requires, and the second is superseded by `release_publication_test.py`,
  which already pins the module inventory. This reverses an earlier reading of
  mine that the LEM-5 suite was redundant with
  `release_publish_path_permissions_test.py`: that suite asserts what each
  *named* job holds, while LEM-5 asserts that **no** job can hold both
  authorities, that the token scope and the protected environment appear
  together in exactly one place, that the check is non-vacuous, and that a
  synthetic offending job is reported. All four were absent. The lesson, which
  generalizes past this plan: "an existing file covers this" is a claim about
  the *property*, not about the filename. Date/Author: 2026-10-15,
  implementation agent.

- Decision: extend the `${{`-in-`run` prohibition to **every** composite action
  rather than scoping it by path to the five release-path actions, reversing
  the scoping stated in Stage C of earlier revisions. Rationale: the scoping
  rested on a measurement that does not hold. The five existing actions carry
  68 expression sites, not the 84 the plan recorded (`cross-build-macos` was
  deleted by `9e1bf27e`, #798), and **none** of the 68 is inside a `run` block,
  so a blanket extension is green today rather than red. With the premise gone,
  path-scoping buys nothing and costs a manual extension each time an action is
  added. It is also strictly stricter with no allowlist, which is what the
  mission requires. The deliberate mutation that restores an expression to a
  named step and requires that step alone to fail discharges non-vacuity, so
  the sweep cannot pass by having nothing to sweep. Date/Author: 2026-10-15,
  implementation agent.

- Decision: hold `contents: write` on **two** jobs, `publish-release` and
  `finalize-github-release`, and assert that exact set rather than a subset.
  Rationale: `publish-release` creates the draft release and uploads the assets
  to it, while `finalize-github-release` un-drafts it only once the whole
  release has been verified against the identity record. Un-drafting is itself
  a write, and jobs do not share a token, so a single-holder design would have
  to delete the finalizer's write scope — which is exactly the separation
  between "upload" and "make public" that makes the ordering meaningful. An
  implementation draft of mine asserted the single-holder form and would have
  forced that deletion; recorded because the failure is instructive. "An
  assertion that sounds stricter" is not evidence that it is correct, and a
  stricter-sounding claim needs the same check against the design as a weaker
  one. Date/Author: 2026-10-15, implementation agent.

  **Amended 2026-10-20:** `verify-release-inventory` also holds
  `contents: write`, so the writer set is three jobs rather than two. The
  reason is in `Risks`: a draft release is invisible to `contents: read`, and
  the verifier's entire purpose is to compare the *draft* against the
  declaration before anything is un-drafted. The amended form is therefore "two
  mutating writers plus one read-only holder that needs write scope to see a
  draft", and the LEM-5 suite asserts the difference directly — the verifier
  job's composite action is required to contain no `gh release` call at all, so
  the extra scope cannot quietly become a second way to write to the release.
  LEM-5's own text (disjointness, and `id-token`/`environment` uniqueness) is
  unchanged and remains what the contract proves. Date/Author: 2026-10-20,
  implementation agent.

- Decision: resolve the verifier's network boundary as **orchestration
  downloads, the module decides**, and enforce EP-M2's no-network claim with a
  test rather than leaving it as conformance prose. Rationale: EP-M2 says the
  six — now seven, and more once Stage C's modules joined `SOURCE_MODULES` —
  modules perform no network I/O, while Stage B described
  `verify_crates_io_bytes.py` as a "token-free download". The two cannot both
  be true, and the choice that keeps the property is the one that makes the
  policy testable without a socket: the workflow step issues the request and
  writes the bytes, and the module compares a path's digest against the
  identity record. The module's names still say "download" (`download_url`,
  `required_headers`, `verify_downloaded_archive`) because they describe the
  orchestration it serves; none of them opens a connection. Rather than fix the
  wording alone, `test_policy_modules_import_no_network_client` now parses every
  `SOURCE_MODULES` entry and fails on an import of `urllib`, `http`, `socket`,
  `ssl`, `requests`, `httpx`, `ftplib`, or `urllib3`, with a positive control
  over four spellings and a negative control over three. A claim in a
  Conformance check that nothing executes is the Cuprum #499 failure this plan
  exists to avoid, one layer up. Date/Author: 2026-10-17, implementation agent.

- Decision: give each workflow-facing job a **thin CLI sibling** rather than
  adding `main()` to a policy module or building one multi-command dispatcher.
  Rationale: all seven EP-M2 modules are pure libraries — none defines `main`,
  `__main__`, or an `argparse` surface, and nothing outside their own tests
  imports them — so Stage C has no callable entry point to invoke and
  `release.yml` cannot reach them as written. Reversing the earlier plan text's
  implicit assumption here is unavoidable: the plan named the modules and the
  jobs but never named the seam between them. The sibling keeps EP-M2's claimed
  property intact — `test_policy_modules_import_no_network_client` constrains
  `SOURCE_MODULES`, and a CLI that performed no I/O could not satisfy its own
  job, so putting the argument parsing and any file reads in a separate module
  leaves the policy modules holding the same no-I/O contract they were proved
  against. A single dispatcher was rejected because five commands with distinct
  arguments would push one file toward the 400-line cap and make the dispatch
  table the thing under test. Date/Author: 2026-10-19, implementation agent.

- Decision: add a new contract module,
  `tests/workflow_contracts/release_publication_graph_test.py`, rather than
  extending `release_publish_path_permissions_test.py` or
  `release_publish_path_contract.py`. Rationale: the two existing files are
  imported by other contract modules and pin the *current* two-job shape by
  exact assertion; the new jobs need LEM-5 permission separation and
  skip-propagation reachability proved, which is a different claim from "the
  staging job is read-only". Editing the pinned files would couple an unchanged
  invariant to a changing one and make a failure ambiguous between them.
  Date/Author: 2026-10-19, implementation agent.

- Decision: name the packaging composite action `package-crate/`, matching its
  job, and give `consumer-smoke` its own action under that name. Rationale:
  Stage C's bullet list and its numbered job list disagree — the bullets name
  `crate-package/` and `release-candidate/` against jobs called
  `package-crate`, and no action is named at all for the fifth job — while §601
  and the 2026-10-15 Decision entry both require every job's name to match the
  action it calls and record a rename performed solely to restore that parity.
  Two of those three statements agree, and the third is a list written before
  the jobs were named; parity is therefore the settled reading.
  `release-candidate/` survives as the name of the *script* seam
  (`scripts/release_candidate_cli.py`), which is a different artefact with a
  different naming convention, so nothing is lost by the action being called
  `package-crate`. The graph contract derives the action path from the job name
  rather than tabulating it, so the convention is enforced rather than
  documented. `consumer-smoke` is the one job whose body calls no policy
  module: it is a download-verify-extract-run sequence, so there is no
  `scripts/consumer_smoke.py` for it to invoke and no Python seam to test. It
  is still a composite action under its own name —
  `.github/actions/consumer-smoke/` — because the job/action name parity above
  applies to all five jobs without exception. Date/Author: 2026-10-19,
  implementation agent.

- Decision: `ReleaseIdentity` keeps its six fields and the digest record
  travels **beside** it, rather than gaining the `assets` and `digests` fields
  the `Interfaces and dependencies` sketch shows. Rationale: the sketch is not
  describing one record but two. The identity is what the packaging job is
  authorized to publish — tag, commit, package name, version, binary name,
  prerelease flag — and it is settled before any artefact exists, which is why
  it can refuse a disagreeing tag *before* spending build time. The digests
  cannot exist until the crate and the release assets have been produced, so
  putting them in the same record would either delay the identity past the
  point it is useful or make the dataclass's own validity conditional on a
  later stage. The implemented flow is therefore `package_crate_cli --output`
  writing the identity and the crate digest, and two separate
  `{"assets": {...}}` records written at two different points. Adding the
  fields later is a schema change to a record the plan's own criterion compares
  by exact set equality, so the separation is recorded now rather than left as
  an apparent gap. Date/Author: 2026-10-19, implementation agent.

  **Amended 2026-10-23**: the second half of that flow was backwards, and the
  reversal is recorded here rather than left to be inferred from the code. It
  read `verify_release_inventory_cli --output` writing `{"assets": {...}}` and
  `finalize_github_release_cli._expected()` reading that record. That makes the
  expected digests a function of the download: the verifier hashes what it was
  handed, the finalizer compares GitHub against that hash, and the two agree
  however the bytes were altered — because there is only one copy of the
  evidence. It detects an incomplete upload and nothing else.

  The flow is now anchored before the upload.
  `release_declaration_cli --digest-record` writes the **trusted record** —
  `release_staged_digests.staged_digests` hashes each declared asset in the
  staged tree, keyed by `release_declaration.published_name` so a prerelease
  package is recorded under the spelling GitHub publishes rather than the `~`
  it is staged under. `verify_release_inventory_cli --trusted` reads that
  record and `release_digest_record.require_downloads_match_record` compares
  every download against it, which refuses a replaced operating-system package
  and, because the record's archive digest does not move when both files are
  rewritten, a replaced archive/sidecar pair.
  `finalize_github_release_cli --identity` reads the same trusted record, not
  the verifier's output. The record's shape and its refusals live in
  `release_digest_record` so the two consumers cannot disagree about what a
  usable record is, and both files are written by the declaration stage only on
  success. The assessed head `f7168a6f2` had the old flow; this amendment
  describes the code as it stands after the repair. Date/Author: 2026-10-23,
  implementation agent.

- Decision: the release publishes a **flat consumer set of twenty names** —
  the six binstall archives, their six `.sha256` sidecars, and the eight
  operating-system packages — and the declared-name producer emits exactly that
  set. Everything else the build stages (per-platform binaries, completions,
  manual pages, PowerShell help, `LICENSE`) ships **inside** the packages and
  is not a release asset. Rationale: a GitHub release is a flat namespace and
  the build produces six distinct `netsuke` binaries, so no staging layout that
  publishes the binaries separately can be correct without renaming every one
  of them; the beta4 release is the measurement, with 12 `netsuke` sources
  collapsing onto a single name. The twenty-name set is the only one whose
  members are already unique and already consumed by a documented install path
  (`cargo binstall`, `.deb`, `.rpm`, `.pkg`, `.msi`), so it is the smallest
  declaration that is both correct and checkable.

  The producer's rule, stated as three parts so that no part can drift into a
  model of the uploader's defect:

  1. Archives and sidecars come from
     `hoist_binstall_discovery.expected_archive_names`, already the single
     authority the hoist and the inventory share — not from the staged tree.
  2. Operating-system packages come from the staged tree's `.deb`, `.rpm`,
     `.pkg` and `.msi` entries, because three external shared actions own
     their naming and reimplementing those rules here would be an unverifiable
     guess. `~` is mapped to `.` in each name, which is the only normalization
     GitHub performs over a name this repository produces.
  3. Nothing else is declared. In particular the producer never applies
     `_is_candidate`'s basename rule, because that rule is the defect: it is
     what turns a bash completion into `netsuke` and the aarch64 build into
     `netsuke.exe`.

  Because parts 1 and 2 have different sources, the declaration is not vacuous
  when a package fails to build: part 1 fixes twelve names from configuration,
  and part 2 is held to a per-platform format count derived from
  `.github/release-staging.toml`'s `platform` fields (linux yields `.deb` and
  `.rpm`, macos `.pkg`, windows `.msi`, two targets each). A missing package
  therefore shrinks neither half silently.

  Two consequences for the release path follow from the set, not from taste.

  First, `publish-release` can no longer hand `dist/` wholesale to
  `upload-release-assets`: that action's `_is_candidate` rejects `.tar.gz` at
  both pins, so the six archives — the only asset `cargo binstall` fetches —
  are unreachable through it however the tree is arranged, and any selection
  rule that admits them is the same rule that admits all 86 basenames. The
  upload step therefore hands `gh release upload` exactly the declared names,
  one call per name, and the working directory it uploads from must hold them
  under their **published** spelling. That is what makes the `~` → `.` mapping
  load-bearing rather than cosmetic: `gh` names an asset from the source file's
  basename, so an RPM still called `netsuke-0.1.0~beta4-1.x86_64.rpm` on disk
  publishes as `netsuke-0.1.0.beta4-1.x86_64.rpm` and would disagree with the
  declaration by one character. The step materializes each name before
  uploading it. This supersedes the 2026-10-12 statement that `publish-release`
  keeps "its steps unchanged"; the supersession is forced by the uploader's
  rule, not by a preference for a different layout, and it is the smallest
  change that publishes an archive at all.

  Second, the ordering does not move. `publish-release` uploads, then
  `verify-release-inventory` compares the whole release against the
  declaration, then `finalize-github-release` un-drafts — and the finalizer
  continues to **refuse** rather than upload: `finalize_github_release_cli`
  reports `EXIT_UPLOAD_REQUIRED` when an asset is missing. That corrects the
  `Idempotence and recovery` sentence "The GitHub finalizer inspects the draft
  and uploads only missing assets", which the implemented CLI never did and
  could not do after the verifier it depends on. A re-run that finds an asset
  missing fails closed at the verifier, which is the honest outcome: the
  missing asset is a defect in the upload step, and repairing it by having a
  second job write to the same release is the competing-writer shape LEM-5
  exists to keep out.

  User-visible consequence: `README.md` and `docs/users-guide.md` currently say
  SHA-256 sidecars "accompany standalone binaries and staged help and licence
  files". That sentence describes files this change stops publishing. Stage D
  corrects it rather than leaving a documented asset that the release no longer
  carries. Escalated under the `Ambiguity` tolerance on 2026-10-19 (two
  materially different asset-inventory semantics: publish the consumer set, or
  namespace the full staged tree) and resolved in favour of the consumer set
  because the namespaced alternative multiplies the published surface by
  roughly six and contradicts the plan's own scope tolerance for a release
  layout the users' guide does not document. Date/Author: 2026-10-19,
  implementation agent.

- Decision: Stage C's workflow-facing CLIs keep **stdlib-only `argparse`**,
  matching the six sibling `*_cli.py` modules, rather than adopting the
  Cyclopts style `main` has been using for its standalone scripts. Rationale:
  both styles are live on the rebased tree and neither is deprecated — main's
  `uv run --script` + PEP 723 +
  `App(config=cyclopts.config.Env("INPUT_", command=False))` shape appears in
  six scripts, and the six sibling CLIs this branch already wrote use
  `argparse` with no third-party import at all. The discriminating constraint
  is not style but the gate that reads these modules:
  `test-release-publication` names every `SOURCE_MODULES` entry on a
  `uv run --no-project` command line whose `--with` list is pinned by
  `REQUIRED_RECIPE_FRAGMENTS` and installs only pytest and hypothesis, while
  the repository root's tracked `cyclopts/__init__.pyi` makes `import cyclopts`
  resolve to a namespace package with no `App` unless Cyclopts is installed —
  so a module-scope `App` would make six of the fourteen `SOURCE_MODULES`
  entries unimportable and red the target. Adopting Cyclopts would therefore
  require editing the recipe's `--with` list and `REQUIRED_RECIPE_FRAGMENTS`,
  coupling a registry-publication change to a test-framework contract for no
  behavioural gain. Recorded because this is the kind of rebase-induced
  convention divergence that otherwise gets re-litigated at every new CLI; the
  two observations under `Surprises & discoveries` carry the probe evidence.
  Date/Author: 2026-10-21, implementation agent.

- Decision: correct two line-number citations this plan carried; leave three
  others alone after re-deriving them. Rationale: only the first is
  rebase-induced, and separating the two cases matters because an over-eager
  "refresh every citation" sweep is how a correct citation gets broken.
  `docs/developers-guide.md:1361-1362` — cited three times, for the
  composite-action convention — moved to `:1415-1416`, because `main` grew that
  file by 317 lines and the sentence it names is unchanged.
  `release_inventory.py:319` → `:318` is **not** rebase-induced: the file is
  the branch's own, byte-identical at `OLD_HEAD` and at the rebased head, and
  `:318` is the `\`-continuation line the sentence describes while `:319` is
  that continuation's second physical line. The two `verify_crates_io_bytes` and
  `nextest_durations` citations, at `:279` and `:243`, are correct as written
  and were not touched. The DOC502 citation `release_reconcile_crates.py:161`
  was also challenged and **survived**: probing
  `ruff check --preview --select DOC502` against the commit it names reports
  `:161:5`, the docstring's opening line, so the observation's original number
  was right and its missing provenance — that the rule is Ruff's `--preview`
  rule and not a df12 Pylint plugin rule, which rates the same blob 10.00/10 —
  was the actual gap. That provenance is now recorded in the observation
  instead of a renumbering. Date/Author: 2026-10-21, implementation agent.

- Decision: the five Stage C composite actions take the Cargo **package name**
  as an ordinary declared `input`, and the `metadata` job supplies it from
  `steps.ensure_version.outputs['crate-name']`. The job therefore gains one
  more output, `package_name`. Rationale: the value has to come from the
  manifest rather than from a workflow literal, because `Cargo.toml` declares
  `package.name = "netsuke-build"` while `[lib]`/`[[bin]]` are both `netsuke`
  (the divergence
  `docs/adrs/adr-0007-publish-the-crates-io-package-as-netsuke-build.md`
  records), and this job's only manifest-derived name today is the *binary*
  name — passing that as `--package-name` would make the packaging seam reject
  the candidate, and `publish-crates-io`'s authenticated exchange request the
  wrong crate. `ensure-cargo-version` at the pin `release.yml` already carries
  (`ff1dd759dfffc0db3459e30e833f52437ee62b57`) declares a `crate-name` output
  and writes it unconditionally from `manifest_versions[0].name`, independent of
  `check-tag`, so the name is already resolved in this job and needs no new
  action, no `cargo install`, and no new cache. Two alternatives were rejected
  on evidence rather than preference. Widening the existing
  `export-cargo-metadata` step to `fields: name,bin-name` would work — the pin
  `a5765019912a8ab6882b12db049c7cde635f3a85` declares a `name` output and its
  `_SUPPORTED_FIELDS` accepts `name` — but `tests/workflow_release.rs:35` pins
  the exact substring `fields: bin-name`, so it buys nothing `crate-name` does
  not already provide while forcing an edit to a Rust contract test. Reading
  `[package].name` through `scripts/cargo_package_metadata.py` would grow a
  four-field reader whose output block
  `scripts/tests/test_cargo_package_metadata.py` pins by exact equality, and
  whose readers (`validate_linux_package_metadata.py`) index
  `cargo_meta.PackageMetadata` by key, so the new key would owe a
  `PackageMetadata` TypedDict entry and an edit to the equality-pinned
  expectation for no gain. The lane this runs in is a dry run as well as a tag
  push, and `crate-name` is emitted in both. Date/Author: 2026-10-21,
  implementation agent.

- Decision: every action and job that needs the candidate commit reads
  **`${{ github.sha }}`** into an `env:` key named for its use, rather than
  reading an output from a preceding step. Rationale: `github.sha` is the
  revision the workflow run was triggered on, which on a `push` tag event is
  the tag's own commit, and it needs no seam to carry it — the composite action
  declares a `commit` input and the job supplies it from the context, so the
  value crosses the boundary as an ordinary declared input like `package-name`
  and `version`. The repository already reads the revision from the ambient
  environment in both shapes this branch needs:
  `REVISION_VARIABLE = "GITHUB_SHA"` in
  `.github/scripts/_release_admission/configuration.py:46` resolves it through
  `_required`, which refuses an unset or empty value rather than substituting
  one, and `.github/actions/windows-gate-cache/action.yml:97` reads
  `${GITHUB_SHA}` directly into its cache key. The alternative — resolving the
  tag to a commit and publishing that — was rejected because
  `ReleaseIdentity.commit` is a record of *which bytes were packaged*, and the
  only commit those bytes can come from is the one the workflow checked out.
  Recording a separately resolved SHA would make the identity a claim about a
  revision the packaging job never saw. `https://github.com/actions/checkout`
  defaults to `github.sha` when neither `ref` nor `sha` is given, and the
  release path's checkouts set neither, so the two agree by construction rather
  than by a later comparison. Date/Author: 2026-10-21, implementation agent.

- Decision: the declared-name producer runs inside **`publish-release`**,
  inserted after the hoist step and before the upload step, and the declaration
  it writes travels to the verifier through a new `publish-release` job output.
  Rationale: `release_declaration_cli` requires a `--dist-dir` holding the
  **staged** tree, and `publish-release` is the only job that has one —
  `package-crate` never sees `dist/`, `publish-crates-io` is skipped on a dry
  run, and placing the step there would leave the declaration unproduced
  exactly when the verifier needs it. The insertion is legal because
  `release_publish_path_publication._check_publication_step_order` pins only
  the *relative* order of its four anchor steps
  (`Ensure release exists (draft)`, `actions/download-artifact@`,
  `hoist_binstall_archives.py`, `id: upload_assets`), not adjacency, so a new
  step may be placed among them without editing that contract. The declaration
  is produced *after* the hoist because the hoist is what puts the archives
  under their published spelling, and it is produced *before* the upload
  because the upload is what consumes the names. Date/Author: 2026-10-21,
  implementation agent.

- Decision: **no new module reads GitHub's asset listing.** The
  `finalize-github-release` action reshapes `gh release view --json assets`
  into its own `--assets` input with `--jq`:
  `[.assets[] | {key: .name, value: .digest}] | from_entries`. Rationale: a
  first draft of this branch added `scripts/github_release_assets.py` to parse
  the listing into a record, and that draft was **deleted rather than
  repaired**, for three independent reasons. First, it produced the wrong shape
  twice over: it stripped the `sha256:` prefix, while
  `release_reconcile_github.normalise_digest` returns `None` for anything
  without that prefix — so every asset the release already held would have been
  planned as a `CONFLICT`, and `finalize_github_release_cli` would have refused
  a correct release — and it nested the mapping under `"assets"`, which is the
  *identity* record's shape, whereas `--assets` reads a top-level mapping. It
  therefore matched neither consumer. Second, its private `_bare_digest`
  re-implemented the already-exported `normalise_digest`, and its "an absent
  digest is refused rather than treated as unverified" policy is already
  `plan_asset_actions`' behaviour, so the module owed the
  abstraction/port/helper policy a justification for a second helper whose only
  caller was itself. Third, the plan's `Interfaces and dependencies` section
  names no listing reader, so the module was implementing a seam the approved
  plan does not describe — the correct response to which is to surface the
  divergence, not to add a consumer for it. The `--jq` reshape was probed
  against the live `v0.1.0-beta4` release and returns the expected 29-entry
  mapping with prefixed digests intact, which is exactly what
  `finalize_github_release_cli --assets` consumes.

  The listing is taken **in the finalizer, not in the verifier and not in
  `publish-release`**. Three constraints decide it. The listing must be read
  after the upload, which rules out `publish-release`: its own listing would
  predate the very upload the verifier is checking. It must not be read by
  `verify-release-inventory`, because the LEM-5 amendment above requires that
  job's composite action to contain **no `gh release` call at all** — that is
  what stops its extra `contents: write` scope from becoming a second way to
  write to the release, and `gh api` without a method flag is a GET, so the
  verifier downloads through `gh api` and the rule holds mechanically. And it
  need not be plumbed from either job as an artefact: the finalizer runs
  immediately after the verifier with no writer in between, so re-reading the
  release cannot see a set the verifier did not. The finalizer is also the only
  one of the three that already holds `contents: write` and is permitted
  `gh release` calls, so the reshape introduces no new credential surface.
  Date/Author: 2026-10-21, implementation agent.

- Decision: the per-name upload is a **new local composite action**,
  `.github/actions/upload-declared-assets/`, and `publish-release`'s step keeps
  its `id: upload_assets`, its name `Upload artefacts to release`, its
  position, and its `with.dry-run` expression — only its `uses:` target
  changes. Rationale: the 2026-10-19 coupling note states the choice exactly —
  a per-name rewrite "has to keep the step's id, its name, its position, and a
  `dry-run` key — or edit those four assertions in the same commit, which would
  move the contract away from the property it states". A `run:` step cannot
  satisfy the first branch, because `check_upload_plan` reads `with.dry-run`
  through `require_mapping`, which fails on an absent `with`; and `with` is not
  a `run`-step key at all. Keeping the step a `uses:` step is therefore the
  only way to hold the stated property rather than restate it. The action is a
  composite action rather than an inline `run:` block because the repository's
  own convention is to extract a step body into `.github/actions/`
  (`docs/developers-guide.md:1574-1575`), and because the composite-action
  contract then applies the run-interpolation rule to it automatically. It is a
  sixth action rather than a sixth *job*: the five-action rule in Stage C names
  the actions the five new jobs call, and this one is called by an existing
  job, so the job/action name-parity rule that produced those five names does
  not reach it. The name states what it does — it uploads exactly the declared
  names — rather than repeating the job it serves.

  Two couplings follow, both measured. First, `tests/workflow_release.rs:25`
  pins the raw substring `upload-release-assets@`, and `release.yml` carries
  that substring twice — once in `release` at line 493 (pin `a5765019…`) and
  once in `publish-release` at line 634 (pin `0606ca89…`). Changing only
  `publish-release`'s `uses:` leaves exactly one occurrence, so the Rust pin
  stays satisfied with no edit to the Rust suite. Substituting **both** lanes
  would delete the substring and turn that assertion red, which is a second
  reason the rehearsal lane is out of scope for this change. Second, the
  orientation sentence "the repository now invokes one shared action at two
  revisions, chosen by job" describes the current tree and goes stale in the
  same commit that lands this change; it must be corrected there rather than
  left as a description of a file that no longer has that shape.

  The rehearsal lane is left on the shared action deliberately, and the gap is
  recorded rather than closed: `release` runs with `dry-run: true` and uploads
  nothing, so the defect it retains is that its *plan* omits the archives, not
  that it drops bytes a user fetches. Its dry run therefore cannot catch a
  declaration-versus-release disagreement that `publish-release` would hit.
  Closing that would mean giving `release` the declaration producer too, which
  would put a `publish-release`-owned output on a mutually exclusive job and
  widen this change beyond the upload path. Flagged for Stage D rather than
  decided here. Date/Author: 2026-10-21, implementation agent.

- Decision: give the packaging action an explicit rehearsal mode rather than
  loosening the tag validator or inventing a tag for a pull-request run.
  Rationale: `release_candidate._require_tag_version_agreement` exists to
  refuse a tag that does not point at the commit the bytes were built from, and
  a merge commit is exactly such a case — recording `v0.1.0-beta4` for a PR run
  would name a tag that points at the release commit while the bytes were built
  from the merge commit, which is the near-match that module was written to
  reject. Weakening the check would trade the publication guard for a rehearsal
  convenience, so the mode is carried instead, and the mode output already
  exists in the same job: `ensure-cargo-version` has taken
  `check-tag: fromJSON(steps.release_modes.outputs['should-publish'])` since
  before this branch, so tag/version agreement as a *publication-time*
  invariant is an established pattern here rather than an invention. The two
  selectors are made mutually exclusive at the CLI rather than reconciled in
  it, so a publishing call site that lost its tag reaches an argument error
  instead of a record that names no release. `_recorded_tag` was added beside
  the reader for the same reason in reverse: `str(None)` would read a
  rehearsal's `null` back as the four-character tag `"None"`, so absence is
  passed through as absence. Date/Author: 2026-10-23, implementation agent.

## Outcomes & retrospective

Not yet applicable. To be completed at EP-M6, at which point every discovery
above must be reconciled against the named upstream artefacts.

## Revision note

Draft 1, 2026-09-27: initial plan, written after reconnaissance of
`release.yml` and the contract suites that constrain it. Rows in
`Verification plan` map to milestones in `Milestones and plateaus`; the
one-line placeholder in `Purpose / big picture` must be replaced before the
plan can leave `DRAFT`.

Draft 2, 2026-10-11: rebased onto `main` at `557966c8` (56 commits on from the
draft's base) and corrected against the rebased tree. `release.yml` is 685
lines, not 438; the version is `0.1.0-beta4`, not beta3; the dry run no longer
inherits secrets. #786 added a Terms of Reference, so the "no ToR covers
release" claim is corrected in `Conformance basis`. #834 landed a
staging/publication split and much of the contract scaffolding, recorded as a
discovery and as a decision to build within it. Still `DRAFT`, still not
approved, and still not implemented: EP-M1 through EP-M6 remain open.

Draft 3, 2026-10-21: rebased onto `main` at `37fc364a` from the exclusive
replay boundary `fa5a4ffa` (27 commits, zero conflicts, `range-diff` fully
`=`), and re-validated at HEAD `b74f6571` against all six gates. Unlike Draft
2, this rebase introduced no new external requirement into the plan's scope:
`main`'s delta touches no `scripts/` file this branch owns, no `Makefile`
target, and no `.github/actions/` manifest, so the branch's file set is
unchanged by the replay. It did surface one convention question — Cyclopts
versus `argparse` for the workflow-facing CLIs — which the Decision log settles
in favour of the branch's existing `argparse` shape on the strength of three
probes rather than on style, and it moved three citations that `main`'s growth
had displaced. Both are recorded above. EP-M1 through EP-M6 remain open; Stage
C is under way, with the packaging seam repaired and the identity CLI landed.
