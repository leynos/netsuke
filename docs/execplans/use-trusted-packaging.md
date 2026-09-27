# Publish `netsuke-build` through crates.io Trusted Publishing

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

The plan is a draft for review. No implementation has begun, and no release,
tag, registry publication, GitHub Release, credential, or account setting is
changed by this document.

## Purpose / big picture

Netsuke currently ships a binary on six platform targets through
`.github/workflows/release.yml`, which stages a **draft** GitHub Release and
leaves a human to publish it. Nothing in that path touches a registry, so a
user who runs `cargo install netsuke-build` today gets whatever bytes a
maintainer uploaded by hand, with no release job able to say which commit they
came from. After this change, a single tagged run resolves one candidate
commit, builds and verifies the platform artefacts and the Cargo package from
it, publishes the package to crates.io through Trusted Publishing, verifies
the registry bytes, and only then publishes the GitHub Release. A user can
`cargo install netsuke-build --version 0.1.0-beta3` and get a crate the
release run can account for.

The registry upload itself stops depending on a long-lived secret. GitHub
mints a short-lived, repository-scoped token from an OIDC identity; the token
exists only inside the one step that needs it and is revoked by the action's
own post-step. No maintainer rotates it, and no log, cache, or job output can
leak it.

A maintainer watching a tagged release sees one ordered sequence, and can
re-run any part of it without double-publishing or corrupting an
already-published version. A reviewer can read the release run and see, for
every uploaded file, which commit it came from and which digest was expected.
A pull request still rehearses the whole release without a token, without an
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

**The release today.** `.github/workflows/release.yml` is 438 lines. It runs
on a `v*.*.*` tag push, and as a reusable workflow (`workflow_call`) with
inputs `publish`, `dry-run`, and `wix-extension-version`, and outputs
`bin_name`, `version`, `should_publish`, `dry_run`,
`should_upload_workflow_artifacts`, `wix_extension_version`.

Its jobs are:

1. `metadata` — resolves the repository name, the release modes through the
   `determine-release-modes` shared action, the crate version through
   `ensure-cargo-version` (with `check-tag` bound to `should-publish`), the
   WiX extension version, and the binary name through
   `export-cargo-metadata`.
2. `build-linux`, `build-windows`, `build-macos` — each a two-leg matrix
   calling `./.github/workflows/build-and-package.yml`, which compiles,
   packages (`.deb`, `.rpm`, `.msi`, `.pkg`), runs the release-help tooling,
   and uploads workflow artefacts.
3. `windows-native-recipe-smoke` — a native Windows debug build plus
   `./scripts/windows-recipe-smoke.ps1`, skipped on a dry run except for the
   `ready_for_review` event.
4. `release-admission-canaries` — the ADR-020 admission-evidence gate.
5. `release` — creates a **draft** GitHub Release, downloads the workflow
   artefacts into `dist/`, hoists the cargo-binstall archives to the release
   root, and calls the shared `upload-release-assets` action.

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

**The upload action's real selection rule.** Read from
`leynos/shared-actions` at pin `a5765019912a8ab6882b12db049c7cde635f3a85`, its
`_is_candidate` accepts exactly: a file named `{bin-name}`, `{bin-name}.exe`,
or `{bin-name}.1`; any file whose name ends `.sha256`; and any file whose
suffix is `.deb`, `.rpm`, `.pkg`, or `.msi`. It therefore **rejects**
`.tar.gz`, `.crate`, `.json`, and `.sigstore` bundles. `clobber` defaults to
`"true"`, and the action never fails its own step: a `gh` failure sets
`upload_error=true` and `uploaded_count=0`, which `release.yml`'s
`Check asset upload errors` step converts into an exit 1.

**The dry run.** `.github/workflows/release-dry-run.yml` calls `release.yml`
with `dry-run: true` on `pull_request` events `opened`, `synchronize`,
`reopened`, and `ready_for_review`, with `secrets: inherit`.

**How contract tests read these files.** `tests/workflow_contracts/` is a
PyYAML-based suite run by `make test-workflow-contracts`. It parses workflows
with a YAML 1.2 boolean resolver and duplicate-key rejection
(`workflow_loading.py`), follows reusable-workflow calls transitively
(`workflow_call_closure.py`), and drives both parsed values and raw text. A
second, Rust-side suite (`tests/workflow_release.rs`,
`tests/workflow_shared_actions_pins.rs`) asserts raw substrings and full-commit
pins.

**The file-size ceiling.** No source file may exceed 400 lines
(`AGENTS.md`, enforced for Python by Pylint's `max-module-lines = 400`, for
Rust by Whitaker's `module_max_lines`, and by convention for
`.github/workflows/*.yml` — see `docs/developers-guide.md`, which states that
each lane's cache steps live in a composite action *because* the workflow
files must stay inside that limit).

**Trusted Publishing.** crates.io Trusted Publishing exchanges a GitHub OIDC
token for a short-lived registry token. Its registration fields are the
repository owner, the repository name, the **workflow filename** (a bare file
name), and an **optional environment**. Two properties of that design drive
this plan: the exchange must therefore happen in a job that lives in
`release.yml` itself, not in a reusable workflow (whose filename would be
presented instead); and the crate must already exist on crates.io, because the
very first publication cannot be trusted-published and needs a manual API
token.

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

No Terms of Reference or technical-design document covers release
publication, so none is cited for it; this plan does not invent one. The
governing artefacts are:

```plaintext
AGENTS.md                      -> EP-M1..EP-M6  -> every gate named below
docs/adr-007-...              -> EP-M2         -> tests/binstall_metadata_tests.rs
docs/adr-020-...              -> EP-M6         -> make test-release-admission
docs/documentation-style-guide.md -> EP-M1      -> tests/execplan_status_contract_tests.rs
docs/developers-guide.md      -> EP-M2, EP-M6   -> docs updates
docs/repository-layout.md     -> EP-M2         -> docs updates
```

External dependencies, cited by revision and treated as axioms:

```plaintext
rust-lang/crates-io-auth-action  c6f97d42243bad5fab37ca0427f495c86d5b1a18  (v1.0.5)
leynos/shared-actions            a5765019912a8ab6882b12db049c7cde635f3a85
```

Cuprum (`leynos/cuprum`) is a motivating context only. Its release design —
a pure-policy module, a thin CLI, and workflow `curl`/`gh` steps as the sole
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
  registration.
  Severity: high
  Likelihood: medium
  Mitigation: make the triple an explicit, tested value
  (`crates-io` environment, workflow filename `release.yml`, owner `leynos`,
  repository `netsuke`); assert it in a contract test; and state plainly in
  the PR and the runbook that no live exchange was validated.

- Risk: the initial publication cannot be trusted-published at all, because
  the package does not yet exist on crates.io.
  Severity: high
  Likelihood: high
  Mitigation: record it as the first item of the maintainer checklist, and
  have the publisher detect the registry's "crate does not exist" response
  and fail with an actionable message rather than a stack trace.

- Risk: `cargo publish --dry-run` is slow on the release lane and blows the
  job timeout, as it already does on Windows for the existing packaging test
  (`tests/packaging_smoke_tests.rs` cites 240.8 s median, 265.6 s max).
  Severity: medium
  Likelihood: medium
  Mitigation: run the package job on a Linux lane with a measured timeout,
  and allow `--no-verify` only where a prior verified build exists for the
  same digest.

- Risk: a re-run of a tagged workflow double-uploads an asset or attempts to
  re-publish an existing version.
  Severity: high
  Likelihood: high
  Mitigation: `concurrency` with `cancel-in-progress: false` for the tag
  event, reconciliation that inspects remote state before acting, and a
  digest comparison before any skip.

- Risk: expanding `make github-actions-lint` to cover composite actions
  breaks the existing contract test that pins its recipe, or breaks the
  `lint` target for an unrelated reason.
  Severity: medium
  Likelihood: medium
  Mitigation: keep yamllint's existing invocation and actionlint's position
  unchanged; add composite-action validation as a separate, additional
  recipe line, and update
  `tests/workflow_contracts/github_actions_validation_test.py` in the same
  change-set.

- Risk: a new composite action under `.github/actions/` silently violates the
  repository-wide source-build or cache policy, because three existing suites
  glob `**/action.yml` and apply those policies to anything added.
  Severity: medium
  Likelihood: medium
  Mitigation: the consumer smoke is specified as a digest-checked archive
  extraction rather than a `cargo install` (see `Surprises & discoveries`),
  and no new action saves a cache. Both are checked by running
  `make test-workflow-contracts` after the first new action lands, before
  more are added.

- Risk: adding jobs to `release.yml` pushes it past 400 lines.
  Severity: medium
  Likelihood: high
  Mitigation: move every extracted step body into a composite action under
  `.github/actions/`, leaving only the job graph inline.

- Risk: a contract test that enumerates release jobs or runner assignments
  fails after a job is added.
  Severity: low
  Likelihood: medium
  Mitigation: the two suites that do enumerate
  (`runner_placement_test.py` through `DIRECT_RUNNER_SOURCES` and
  `REQUIRED_RUNNER_ASSIGNMENTS`) were read before planning. Any added job
  must be registered in `runner_placement_invariants.py` and given a
  `timeout-minutes`; nothing else enumerates the job set.

## Verification plan

The change introduces one central invariant and a small family of lemmas
around it.

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
  records must be accepted. The `ready_for_review` row is reachable and is
  the exact row the current skip condition exempts.

**LEM-1 (tag/version agreement).** A `v`-prefixed tag accepts exactly the
package version it spells, prerelease included, with no lossy normalization.

- Obligation: `v0.1.0-beta3` accepts `0.1.0-beta3` and nothing else.
- Method: parameterized table over accept and reject cases, including
  `0.1.0-beta4`, `0.1.0`, `0.1.0-beta3+meta`, `v0.1.0-beta3` and `0.1.0-beta03`.
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
identical package, conflicting package, yanked version, and indeterminate
error are five distinct outcomes with four distinct actions; only an
identical checksum permits a skip.

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

**LEM-5 (permission separation).** No job holds both a registry credential
path and GitHub release-write authority; the registry-exchange job is the only
one carrying `id-token: write` **and** `environment: crates-io`.

- Obligation: `LEM-5` holds for every job in `release.yml`.
- Method: contract test over parsed jobs.
- Rationale: this is a static property of the file, decidable exactly.
- Artefact: `tests/workflow_contracts/release_permissions_test.py`.
- Evidence: a synthetic job carrying both scopes must be reported.
- Non-vacuity: the check must find at least one job with
  `id-token: write` and at least one job with `contents: write`, or the
  separation assertion would be vacuously true.

**LEM-6 (dry runs publish nothing).** A dry run exchanges no token, creates
no release, uploads no asset, and requires no environment approval.

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

**AXIOM-2.** `crates.io` presents the OIDC `job_workflow_ref` of the file
that declares the job, and requires the registered environment name to match
exactly. Consequence: the exchange job must be declared in `release.yml`.

**AXIOM-3.** `cargo publish` with `CARGO_REGISTRY_TOKEN` set, and no
`--token`, uploads the `.crate` that `cargo package` produced for the same
source tree, and rejects a duplicate version with a distinct error.

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
  (`source_build_data.source_build_commands`) over the concatenated text of
  the changed workflows and actions; it is already a tokenizer rather than a
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
  *detected*. Without that control, a detector that had been broken or
  narrowed to match nothing would make this lemma pass for the wrong reason.

No non-trivial Rust invariant is introduced: this change touches no Rust
source file, so no Kani harness or Verus proof is owed. That conclusion is
recorded here rather than left implicit.

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
   `validate_inventory(...)` implementing LEM-2: non-empty, unique, relative,
   no `..` component, exact set equality against the manifest.
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
- `tests/workflow_contracts/release_permissions_test.py` (LEM-5).
- `tests/workflow_contracts/composite_action_contract_test.py` (new).
- `tests/workflow_contracts/release_modules_wired_test.py` (Cuprum #499).

### Stage C — implementation

`.github/actions/` gains one composite action per extracted body:

- `release-candidate/` — resolve and record the identity.
- `crate-package/` — package, list, verify, digest, stage.
- `publish-crates-io/` — probe, decide, upload, verify registry bytes.
- `verify-release-inventory/` — compare the whole release against the
  identity record.
- `github-release-finalize/` — reconcile the draft, then un-draft.

`.github/workflows/release.yml` gains, after the existing `metadata`,
`build-*`, `windows-native-recipe-smoke`, and `release-admission-canaries`
jobs:

1. `package-crate` — `needs: metadata`; `contents: read`; ubuntu-latest. The
   order is: resolve candidate -> package -> verify.
2. `publish-crates-io` — `needs: [metadata, package-crate, build-linux,
   build-windows, build-macos, windows-native-recipe-smoke,
   release-admission-canaries]`; `if: needs.metadata.outputs.should_publish
   == 'true'`; `environment: crates-io`; `permissions: {contents: read,
   id-token: write}`.
3. `verify-release-inventory` — `needs: [publish-crates-io, release]`;
   `contents: read`.
4. `publish-github-release` — `needs: [verify-release-inventory]`;
   `contents: write`, no `id-token`.
5. `consumer-smoke` — `needs: [publish-github-release]`; verifies the
   published bytes without compiling anything (see `Surprises & discoveries`:
   `cargo install` is forbidden in CI by policy). It fetches
   `https://static.crates.io/crates/netsuke-build/netsuke-build-<version>.crate`,
   checks its SHA-256 against the staged digest, then downloads the published
   cargo-binstall archive by its public release URL, verifies it against its
   checksum sidecar, extracts it, and runs the shipped `netsuke --version`.
   That exercises the real public-consumer download path — the one
   `cargo-binstall` takes for a user — and executes the shipped bytes.

The existing `release` job keeps its name, its `contents: write`, and its
draft-creation and asset-upload steps; its `needs` grows `package-crate`, so
the draft is not created before a package exists. The workflow's
`concurrency.cancel-in-progress` becomes a function of the event: `true` for
`workflow_call`, and `false` for a tag push, so a queued re-run of a tag
reconciles rather than interrupting an upload.

Adding jobs 1–5 to `release.yml` would exceed 400 lines, so each new job's
body is a single `uses: ./.github/actions/...` step.

### Stage D — wiring and documentation

- `Makefile`: a `test-release-publication` target running the new
  `scripts/tests` modules with pinned `pytest` and `hypothesis`, and a
  `test-release-negative-mutation` target.
- `Makefile`: `github-actions-lint` additionally runs yamllint with the same
  policy over `.github/actions/*/action.yml`, and a new
  `bash -n`-based syntax check over each composite action's `run` blocks.
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

- Outcome: `release.yml` carries the full sequence, every body extracted to
  a composite action, the file still under 400 lines.
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
- Conformance check: no long-lived token anywhere; `persist-credentials:
  false` on the publisher's checkout; no cache restore in the publisher.
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
  injected mutation and passes without it; `make github-actions-lint` covers
  `.github/actions`.
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
   actionlint, and `.github/actions/*/action.yml` with yamllint and the
   shell-syntax check.
5. `wc -l .github/workflows/release.yml` reports a value at or below 400.
6. `make lint` and `make test` pass unchanged.
7. A dry-run rehearsal on a pull request runs `package-crate` to completion
   and skips every publishing job, with no environment approval requested.

Red-Green-Refactor evidence: each Stage B module is committed with its test
failing; each Stage C implementation is committed with that test passing; each
Stage D wiring change is committed after the gate that covers it passes.

## Idempotence and recovery

Every step is re-runnable. The publisher inspects registry state before
acting, so a second run over an already-published version with a matching
digest is a no-op success, and a second run with a differing digest fails
closed. The GitHub finalizer inspects the draft and uploads only missing
assets. Nothing in this change deletes a remote object.

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
- [ ] Obtain approval (EP-M1).
- [ ] Stage B: pure modules and their failing suites (EP-M2).
- [ ] Stage C: publication sequence in `release.yml` (EP-M3, EP-M4, EP-M5).
- [ ] Stage D: gates, guards, documentation (EP-M6).

## Surprises & discoveries

- Observation: the shared `upload-release-assets` action rejects `.tar.gz`
  outright, because `Path("x.tar.gz").suffix` is `.gz`.
  Evidence: read from its `scripts/upload_release_assets.py` at pin
  `a5765019912a8ab6882b12db049c7cde635f3a85`.
  Impact: binstall archives reach the release root only through
  `scripts/hoist_binstall_archives.py`, and any new asset type (`.crate`, a
  provenance bundle) will need the same treatment or a different uploader.

- Observation: the action's `clobber` defaults to `"true"`, so the current
  upload path already overwrites same-named assets on a re-run.
  Evidence: same source, `clobber: str = "true"`.
  Impact: the reconciliation design must pass `clobber: false` explicitly
  and verify existing assets itself; leaving the default is the
  "clobber alone is not reconciliation" failure the mission forbids.

- Observation: `runner_placement_invariants.REQUIRED_RUNNER_ASSIGNMENTS` is
  compared for exact equality, but it enumerates named jobs, so adding a job
  does not break it — while renaming `release` would break three suites.
  Evidence: `has_required_runner_assignments` compares
  `dict(assignments) == REQUIRED_RUNNER_ASSIGNMENTS` over a hand-built dict.
  Impact: the existing `release` job keeps its name; new jobs are additive.

- Observation: `actionlint` does not lint composite action files at all.
  Evidence: `rhysd/actionlint` issue 401; `make github-actions-lint` passes
  `.github/workflows` only, and the existing contract test pins that recipe.
  Impact: Cuprum #487 cannot be closed here by adding a flag to actionlint.
  A separate check is required, and the plan must not claim actionlint
  validates composite actions.

- Observation: `release.yml` never un-drafts the release it creates.
  Evidence: the only `gh release` invocations in the repository are
  `gh release view` and `gh release create --draft`.
  Impact: "publish GitHub Release" is new behaviour, not a preserved one,
  and the runbook must say so.

- Observation: PR #780 is open, draft, and `CHANGES_REQUESTED`, and it adds
  jobs named `release-candidate`, `downstream-canaries`, and
  `downstream-canaries-windows`, plus a `candidate-ref` workflow input.
  Evidence: `gh pr view 780`.
  Impact: this plan must not reuse those job names, must not depend on that
  branch, and should say so in the PR so a reviewer is not surprised by the
  collision on merge.

- Observation: `cargo install` is forbidden anywhere in a workflow **or any
  composite action**, by
  `tests/workflow_contracts/cache_ownership_test.py:test_workflows_do_not_reintroduce_source_tool_builds_or_stale_providers`,
  which scans `.github/actions/**/action.yml` as well as the workflows. The
  detector is a tokenizer, not a substring search, so it survives
  `cargo  install`, `cargo +nightly install`, and a folded line continuation,
  and it resolves ambiguity toward detection on purpose.
  Evidence: probed directly —
  `source_build_commands("cargo install netsuke-build --version 0.1.0-beta3")`
  returns `["netsuke-build --version 0.1.0-beta3"]`, a non-empty result, which
  is the failure condition. `cargo publish` and a curl-pipe-tar are not
  matched.
  Impact: a "consumer smoke" cannot be a `cargo install`. It must instead
  fetch the published archive by its public URL, verify the digest, extract,
  and run the binary — which is also the stronger test, because it exercises
  the exact bytes and the exact download path a `cargo-binstall` user takes.
  This is why the plan specifies digest-checked archive extraction rather than
  an install.

- Observation: three existing tests glob every composite action —
  `cache_write_policy_test.py` (only those *with* a cache save),
  `cache_ownership_test.py` (the source-build policy above), and
  `ci_mdtablefix_installer_test.py` (searches for a retired path string) — so
  each new action under `.github/actions/` is subject to those policies
  automatically, with no inventory edit needed.
  Evidence: `grep -rn 'rglob("action.yml")' tests/` returns exactly those
  three files.
  Impact: new composite actions inherit the cache and source-build policies by
  construction. That is the desired direction, and it means the plan's Stage D
  does not need to register new actions in those suites. It does, however,
  mean a new action must not *save* a cache (the key step would then need an
  `IS_TRUNK_PUSH` env entry), which the plan already respects.

## Decision log

- Decision: keep the existing `release` job's name and identity.
  Rationale: `runner_placement_test.py`,
  `release_dry_run_smoke_test.py`, and `release_workflow_hoist_test.py`
  each name it; renaming buys nothing and churns three suites.
  Date/Author: 2026-09-27, planning agent.

- Decision: extract every new step body into a composite action rather than
  growing `release.yml` inline.
  Rationale: the repository's own convention, stated in
  `docs/developers-guide.md`, and the only way to keep the file under 400
  lines while making the sequence explicit.
  Date/Author: 2026-09-27, planning agent.

- Decision: do not adopt Cuprum's filename-first identity rule.
  Rationale: crates.io keys the trusted-publisher registration on the
  Cargo package name, not on a distribution filename; copying the PyPI rule
  would be an unverified transplant.
  Date/Author: 2026-09-27, planning agent.

- Decision: no Cuprum runtime dependency.
  Rationale: the reusable part is a decomposition, not code. Python here is
  stdlib-only and runs under `uv run --no-project`.
  Date/Author: 2026-09-27, planning agent.

## Outcomes & retrospective

Not yet applicable. To be completed at EP-M6, at which point every discovery
above must be reconciled against the named upstream artefacts.

## Revision note

Draft 1, 2026-09-27: initial plan, written after reconnaissance of
`release.yml` and the contract suites that constrain it. Rows in
`Verification plan` map to milestones in `Milestones and plateaus`; the
one-line placeholder in `Purpose / big picture` must be replaced before the
plan can leave `DRAFT`.
