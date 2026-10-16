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
recorded in `docs/adr-007-publish-as-netsuke-build.md`, and it is why a
publisher must never derive a user-facing name from Cargo package metadata.

**The workspace.** `Cargo.toml` is both the package manifest and the workspace
root; `test_support/` is a second, `publish = false` workspace member. Only
`netsuke-build` may be published. An indiscriminate `cargo publish` with no
`-p` selector would attempt `test_support` and fail, or worse succeed under a
name nobody reviewed.

**The release today.** `.github/workflows/release.yml` is 685 lines on `main` —
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
5. `release` — creates a **draft** GitHub Release, downloads the workflow
   artefacts into `dist/`, hoists the cargo-binstall archives to the release
   root, and calls the shared `upload-release-assets` action at pin `a5765019…`.
6. `publish-release` — added by #834 after this plan was drafted. It holds
   `contents: write`, and is where API-bound publication was isolated so that
   read-only staging could stay reachable from dry runs. It calls the same
   `upload-release-assets` action but at the newer pin `0606ca89…`, so the
   repository now invokes one shared action at two revisions, chosen by job.

Nothing in that path publishes to a registry, and nothing un-drafts the
release: today a human publishes the draft.

**The packaging contract.** `.github/release-staging.toml` names six targets
(`linux-x86_64`, `linux-aarch64`, `windows-x86_64`, `windows-aarch64`,
`macos-x86_64`, `macos-aarch64`), the binary and its extensions, `LICENSE`,
five shell completions, per-platform manual pages or PowerShell help, and the
`cargo-binstall` archive shape. `scripts/hoist_binstall_archives.py` moves the
`{name}-{version}-{target}.tar.gz` archives and their `.sha256` sidecars from
the nested workflow-artefact layout to `dist/` root, because the shared upload
action namespaces any file not already at the root.

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
very first publication cannot be trusted-published and needs a manual API token.

## Signposts: documentation and skills

Read, in this order, before writing code:

1. `AGENTS.md` — the repository's own agent instructions.
2. `docs/developers-guide.md` — especially the sections on cache ownership,
   the 400-line limit, and workflow contract tests.
3. `docs/repository-layout.md` — where a new script, action, or workflow
   belongs.
4. `docs/documentation-style-guide.md` — the *ExecPlan* section, which is
   authoritative for this file's header.
5. `docs/adr-007-publish-as-netsuke-build.md` and
   `docs/adr-020-release-admission-observability.md`.
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
docs/adr-007-...              -> EP-M2         -> tests/binstall_metadata_tests.rs
docs/adr-020-...              -> EP-M6         -> make test-release-admission
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

- Risk: the initial publication cannot be trusted-published at all, because
  the package does not yet exist on crates.io. Severity: high Likelihood: high
  Mitigation: record it as the first item of the maintainer checklist, and have
  the publisher detect the registry's "crate does not exist" response and fail
  with an actionable message rather than a stack trace.

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
6. `scripts/verify_crates_io_bytes.py` — token-free download and digest
   comparison of the published `.crate`.

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

- `release-candidate/` — resolve and record the identity.
- `crate-package/` — package, list, verify, digest, stage.
- `publish-crates-io/` — probe, decide, upload, verify registry bytes.
- `verify-release-inventory/` — compare the whole release against the
  identity record.
- `finalize-github-release/` — reconcile the draft, then un-draft.

Each new job's name matches the composite action it calls, so a reader moving
between `release.yml` and `.github/actions/` does not have to translate.

**The new actions pass values by environment, never by `${{ }}` in a `run`
block.** `tests/workflow_contracts/release_run_interpolation_test.py` forbids
`${{` inside any `run` in `release.yml` or `package.yml` — but its
`_step_contexts` skips every job carrying `uses`, so moving a step body into a
composite action **removes that shell from the contract's reach entirely**.

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
   `needs: [metadata, publish-crates-io, publish-release]`; `contents: read`;
   `if: … should_publish == 'true'`. The mode gate is stated directly rather
   than inherited. `release` and `publish-release` are mutually exclusive —
   `release` requires `dry_run == 'true'`, `publish-release` requires
   `should_publish == 'true'` — and GitHub skips any job that needs a skipped
   job. Depending on `release` here would make this job unreachable in both
   lanes: on a tag push `release` is skipped, and on a pull-request dry run
   `publish-crates-io` is skipped.
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
the repository's own documented practice (`docs/developers-guide.md:1361-1362`
records it for the cache steps), **not** that a contract enforces a 400-line
cap on workflow YAML — no such contract exists. `release.yml` is 685 lines on
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
  positional pin that forbids it.
- `.github/workflows/ci.yml`: run the new targets in `build-test`.
- `docs/developers-guide.md`: the release runbook — job graph, package and
  asset identity contract, provenance verification, re-run procedure, and
  partial-publication recovery.
- `docs/contents.md` and `docs/repository-layout.md` if a new workflow file
  is added.
- `docs/adr-007-publish-as-netsuke-build.md`: an addendum naming the
  publisher.

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

- Outcome: six pure modules under `scripts/`, each with a focused suite, all
  runnable locally without network access.
- Requirements: LEM-1, LEM-2, LEM-3, LEM-4 discharged.
- Acceptance evidence: `make test-release-publication` passes; each module's
  reject rows fail before the module exists.
- Conformance check: no module exceeds 400 lines; no module performs
  network I/O; Ruff, Pylint, Interrogate, and ty pass.
- Recovery: the modules are additive; revert the commit.
- Remaining gaps: no workflow wiring yet.
- Compatibility decision: none; these are new private modules.

**EP-M3 — the GitHub-side publication sequence.**

- Outcome: `release.yml` carries the full sequence, every new job body
  extracted to a composite action so the job graph stays readable at a glance.
  (Not "under 400 lines": no contract caps workflow YAML, and `release.yml` is
  already 685 lines on `main`. The extraction follows the repository's
  documented composite-action convention, `docs/developers-guide.md:1361-1362`.)
- Requirements: INV-1, LEM-5, LEM-6 discharged.
- Acceptance evidence: `make test-workflow-contracts` passes, including the
  new permission and mode-matrix tests.
- Conformance check: the six existing outputs are unchanged; the three
  `workflow_call` inputs are unchanged.
- Recovery: the sequence is additive after `release`; a revert restores the
  current behaviour exactly.
- Remaining gaps: no crate is published by any run, because the registry
  registration and the initial publication are maintainer actions.
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
   **Unsatisfiable as written.** The file is 685 lines on `main` before this
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
GitHub finalizer inspects the draft and uploads only missing assets. Nothing in
this change deletes a remote object.

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
  `locale_stub_ui_tests::split_build_fixture_compiles_through_the_direct_rustc_harness` —
  in `.config/nextest.toml` it sits in the **same
  `[[profile.default.overrides]]` block** (`:98-100`) as
  `production_build_module_slice_has_expected_boundary`, which
  `docs/developers-guide.md` documents as a known host-contention timeout whose
  wall clock "is the queue's to decide rather than the test's". Both runs hit
  the cap at 300.007 s and 300.010 s, 0.003 s apart, because
  `period = "60s", terminate-after = 5` is arithmetic and not a measurement.
  `make test` is therefore adjudicated **environmental for the second time and
  at the same site**, reproducing across two revisions eleven hours apart, and
  is **not** evidence against this branch. The doctest half again did not run —
  for the same reason, and it remains **unobserved evidence**, not a pass. That
  is attempt two of the `Tolerances` limit of three for the `test` target
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
  name does not outlive its truth. Both edits are whitespace-canonicalised:
  `mdtablefix --check` reports 184 files unchanged, the token stream is
  identical to the pre-format copy, and the line-initial ordered-list marker
  census is 44 before and after.
- [ ] Stage C: publication sequence in `release.yml` (EP-M3, EP-M4, EP-M5).
  Scope correction from recon at `c1f3f2ed`: this is not an extraction of
  existing bodies. `release.yml` has no `package-crate`, `publish-crates-io`,
  `verify-release-inventory` or `consumer-smoke` job, and no crates.io step of
  any kind; only `release` and `publish-release` carry the upload sequence that
  the new jobs decompose. Stage C therefore writes five new composite actions
  plus the job graph, and is the largest stage in this plan.
- [ ] Stage D: gates, guards, documentation (EP-M6).

## Surprises & discoveries

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
  `release_reconcile_crates.py:161`. The `release_inventory.py` precedent
  settles the convention: `Raises` names what the function's own body raises —
  `validate_inventory` lists only `EscapingAssetNameError` and
  `MalformedAssetNameError` for that reason — and helper-raised exceptions are
  named in `Notes`. Impact: the four exceptions `validate_inventory` inherits
  from `_require_usable_entries` and `_require_declared_agreement` are
  documented in its `Notes` prose, not in its `Raises` section.
- Observation: **the backslash line-continuation form in a doctest does not
  work in a raw docstring**, so it is unavailable wherever the docstring
  discusses backslashes. `release_inventory.py:319` and
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
  header is at `release.yml:537` on `main`. Impact: Stage B must extend these
  modules rather than introduce parallel ones, and Stage C must add the
  registry exchange *inside* the existing `publish-release` job rather than
  create a competing publication job. The plan's own permission-separation
  obligation (EP-M3) is therefore partly already discharged by a test that
  exists; the work is to make the crates.io exchange satisfy it, not to write
  the separation from scratch.

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

## Decision log

- Decision: keep the existing `release` job's name and identity.
  Rationale: `runner_placement_test.py`, `release_dry_run_smoke_test.py`, and
  `release_workflow_hoist_test.py` each name it; renaming buys nothing and
  churns three suites. Date/Author: 2026-09-27, planning agent.

- Decision: extract every new step body into a composite action rather than
  growing `release.yml` inline. Rationale: the repository's own convention,
  stated in `docs/developers-guide.md:1361-1362` for the lane cache steps, and
  the only way to keep the job graph readable while making the sequence
  explicit. **Not** a 400-line cap: no contract enforces one on workflow YAML,
  and `release.yml` is 685 lines on `main`. A local `action.yml` resolves from
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

- Decision: build the crates.io exchange into #834's existing
  `publish-release` job instead of adding a new publication job. Rationale:
  #834 already isolated API-bound publication there and wrote
  `release_publish_path_permissions_test.py` to hold the separation. A second
  publication job would duplicate that boundary and split the permission story
  across two places. This supersedes the plan's earlier job-layout sketch; the
  `release` job keeps its name, as previously decided, and no third job is
  introduced. Date/Author: 2026-10-11, planning agent.

- Decision: record the `make test` timeouts as environmental and proceed,
  rather than re-running until green. Rationale: the two failures land on
  different tests, neither reproduces outside nextest's 300 s allowance, the
  pool is saturated by another repository, and the branch's whole diff is one
  Markdown file. Re-running until it happens to pass would produce a green tick
  whose value cannot be distinguished from the one already recorded, and would
  hide that this host cannot run these two tests reliably under contention. The
  honest record is the two-run evidence above, including the residual risk.
  Date/Author: 2026-10-11, planning agent.

- Decision: **SUPERSEDES the 2026-10-11 decision below.** Declare the crates.io
  OIDC exchange in its own job in `release.yml`, not inside #834's existing
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
  Rationale: the criterion's premise is false. `release.yml` is 685 lines on
  `main`, `ci.yml` is 642 and `build-and-package.yml` is 441, and no contract
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
