# RFC 0001: Harden release integrity and admission

## Preamble

- **RFC number:** 0001
- **Status:** Proposed
- **Created:** 2026-08-11

## Summary

This RFC proposes release-mode arithmetic checks, continuous secret scanning,
dependency and licence policy, and a release-admission gate for Netsuke. The
gate would publish artefacts only when deterministic quality, security, and
integrity evidence exists for the exact commit being released. The proposal is
inspired by the actual upstream
[VTCode repository](https://github.com/vinhnx/VTCode), but retains Netsuke's
stronger existing pinning, verification, and release contracts.

## Problem

Netsuke's [Cargo manifest](../../Cargo.toml) has no explicit
`[profile.release]`. Cargo therefore builds release binaries with overflow
checks and debug assertions disabled, even though six production `debug_assert`
sites express assumptions about valid state. A release-only arithmetic defect
or violated assumption can consequently become an undiagnosed correctness or
safety problem.

The repository also has no Gitleaks, `cargo-audit`, `cargo-deny`,
`cargo-about`, or `cargo-unmaintained` gate. Existing quality checks are useful
but are not assembled into one admission decision, and no machine-readable
record currently proves that the commit, checks, security scans, and uploaded
artefacts are the same release candidate. A failed check can therefore be
noticed without being mechanically connected to publication.

Secret exposure is especially difficult to contain after a commit has entered
history. A working-tree scan catches the candidate currently under review, but
only a history scan can find a credential introduced and later removed. A
scheduled history scan also provides a recurring check for secrets that were
missed by an earlier review.

## Current state

- `Cargo.toml` has no `[profile.release]`; release overflow checks and debug
  assertions therefore use Cargo's defaults.
- Six production `debug_assert` sites exist. The current test and lint gates
  do not establish that every release path is safe when those assertions are
  enabled.
- The [CI workflow](../../.github/workflows/ci.yml) already uses SHA-pinned
  GitHub Actions, the pinned nightly toolchain and Polonius,
  all-target/all-feature Clippy and rustdoc checks, Whitaker, Kani, Proptest,
  and the repository's other documented quality gates.
- The [release staging policy](../../.github/release-staging.toml) already
  requires a target archive and matching `.sha256` sidecar. The release
  workflow hoists those pairs before upload, and the `cargo-binstall` metadata
  depends on their names and locations.
- There is no repository-wide policy that combines quality evidence, security
  evidence, release-profile measurements, and checksum verification into a
  publication decision.

The relevant VTCode precedent is pinned to commit
`f188bcb0e47d7386886ab0c3db7e338b297a3d07`. It enables release overflow checks
and debug assertions, scans secrets, and runs licence and unmaintained-
dependency checks. Its secret-scan workflow at that commit uses an unpinned
checkout, and its workflow-policy rules reject that pattern. Netsuke must take
the useful policy intent without copying that weakness.

## Goals and non-goals

- Goals:
  - Enable release overflow checks and debug assertions, with measured and
    reviewable performance and size budgets.
  - Block changes with secrets in the working tree and make a scheduled scan of
    reachable history part of release admission.
  - Define advisory, licence, notice, and unmaintained-dependency policy using
    `cargo-audit`, `cargo-deny`, `cargo-about`, and a
    `cargo-unmaintained` advisory.
  - Make release publication depend on deterministic evidence for the exact
    tag commit, including existing checksum sidecars.
  - Preserve SHA-pinned actions, the pinned Polonius toolchain, Kani,
    all-target/all-feature gates, and the current release naming and checksum
    contracts.
- Non-goals:
  - Replacing Proptest, Kani, Clippy, rustdoc, Whitaker, or existing test gates.
  - Changing Netsuke's public command-line or manifest behaviour deliberately.
  - Copying VTCode's action pinning, workflow layout, thresholds, or download
    mechanisms without verifying them against Netsuke's contracts.
  - Introducing a new package manager, signing service, or runtime telemetry
    system.

## Proposed design

### Release-mode invariants

Add an explicit release profile with `overflow-checks = true` and
`debug-assertions = true`. The profile must apply to every release target and
must not be silently overridden by a packaging job. Release builds continue to
use the pinned nightly toolchain and `-Zpolonius=next` wherever the existing
release workflow supplies that requirement.

The release test set must exercise every supported command path that can reach
the six production assertions, including boundary inputs for arithmetic that
can overflow. A valid supported input must not fail because a newly enabled
assertion detects a violated internal invariant. An intentional rejection must
remain a documented, deterministic error rather than an accidental assertion
failure.

Before enabling the profile as a blocking requirement, CI must record a
baseline from the current profile and a candidate measurement from the new
profile. Both measurements use the same pinned toolchain, target, runner class,
inputs, and build flags. The baseline and candidate are retained with the
release evidence so a later comparison is reproducible.

Property tests must cover every range-based release invariant. Proptest
generators must produce arithmetic inputs at and around each valid boundary,
measurement pairs from zero through twice the baseline, archive sets from zero
through one beyond the permitted target count, and evidence timestamps at and
around the freshness boundary. Each property must assert the corresponding
acceptance or rejection decision, including deterministic rejection for invalid
inputs, exact digest and commit binding, and the performance and size limits.
Bound generated command inputs to 64 KiB and staged archive bytes to 64 MiB;
bound each run to 256 cases and the documented evidence limits. Promote every
discovered counterexample to a checked-in regression case, replay it in the
blocking property suite, and use cargo-fuzz only as a supplement for longer
hostile-input exploration.

### Secret scanning

Use a SHA-pinned Gitleaks action or pinned executable. The working-tree scan is
blocking on pull requests, pushes, and release candidates. It covers tracked,
staged, and untracked candidate files, subject only to a small, reviewed list
of generated paths. A finding fails the job; suppressions must identify the
false-positive pattern, path, reviewer, and expiry, and must not disable the
whole repository scan.

A scheduled job scans the full reachable Git history rather than a shallow
checkout and records both the scanned ref and its resolved commit SHA. Its
result is a blocking security signal: release admission requires a successful
history scan for the repository within the configured freshness window whose
resolved SHA is the exact release-tag commit. A new finding fails admission
until it is removed or explicitly dispositioned. The schedule and freshness
window must be recorded in the workflow contract so a missed or stale scan
cannot be mistaken for a passing scan.

All checkout and scanning actions, including any newly introduced action, must
remain pinned to immutable commit SHAs. No release-hardening workflow may use
an unpinned download URL or an unpinned action tag.

### Dependency and licence policy

The repository will maintain versioned policy files and generate their outputs
in CI:

- `cargo-audit` fails on every vulnerability or unsoundness advisory unless a
  narrowly scoped waiver names the advisory, affected dependency, reason,
  owner, and expiry.
- `cargo-deny` fails on denied advisories, banned crates or sources, and
  licences outside the explicitly permitted set. Unknown licence metadata is a
  failure, not an implicit approval.
- `cargo-about` generates the notices and licence inventory shipped or retained
  for each release. Missing or unrenderable metadata fails the gate.
- `cargo-unmaintained` supplies an advisory report. A newly introduced direct
  dependency reported as unmaintained blocks admission until an owner records
  an approved, expiring exception or the dependency is replaced. Transitive
  findings are reported for review and do not silently become permanent
  approvals.

Tool versions, registry/index state, policy files, and lockfile are inputs to
the evidence record. Waivers are exceptional, auditable, and time-limited; they
cannot turn a failed release job green by editing generated output.

### Release-admission evidence

Add a read-only admission job before any job with permission to publish release
assets. It verifies provenance rather than trusting declared fields and
consumes a machine-readable evidence manifest containing at least:

- the tag, commit SHA, repository, Rust toolchain, and versions of each policy
  tool;
- the approved workflow run identifier, immutable producer job identifiers or
  digests, and the exact release SHA checked out by every producer;
- the result and log identity for formatting, lint, tests, rustdoc, Whitaker,
  Kani, and all existing all-target/all-feature gates;
- release-profile performance and size measurements against their baseline;
- working-tree and sufficiently recent scheduled history scan results;
- advisory, licence, notice, and unmaintained-dependency results; and
- every staged archive name, target, byte size, and SHA-256 digest.

Admission succeeds only when every required result is successful, the evidence
manifest names the exact release commit, and the results and log identities are
bound to the approved workflow run and that SHA. Every producer job must have
checked out the release SHA; admission rejects a manifest with a mismatched or
missing provenance binding. The declared archive set must preserve its required
filenames and cardinality, and every archive must have exactly one matching
`.sha256` sidecar. Before comparison, admission recomputes each archive's
SHA-256 from its actual bytes and compares it with both the sidecar and the
digest recorded in the evidence manifest; any mismatch rejects admission. The
publication job must depend on admission and must not run on a missing, stale,
malformed, or mismatched manifest. A dry run may build and inspect artefacts,
but it must not bypass admission for a real publication.

### Failure modes and mitigations

- A release assertion or overflow check fails: the release is rejected; the
  failing input and assertion are reported, and the profile is not disabled to
  make the job pass.
- A performance or size budget is exceeded: admission fails and retains the
  measurements for review. The budget can change only through a reviewed
  decision, not through an environment variable or an ad hoc workflow edit.
- A scan or advisory tool is unavailable: the result is `unknown`, not
  `passed`, so publication waits for a deterministic rerun.
- A scheduled history scan is late: its evidence expires and admission fails
  until the scan completes within the freshness window.
- A secret, disallowed licence, or unwaived advisory is found: publication is
  blocked, and the failure identifies the finding without printing secret
  material.
- An archive or sidecar is missing, duplicated, or mismatched: staging and
  admission fail before any release asset is uploaded.
- A policy waiver expires during a release: the gate fails closed and names the
  expired waiver for renewal or remediation.

## Requirements

### Functional requirements

- Release binaries must compile with overflow checks and debug assertions
  enabled for every supported target.
- Working-tree secret scanning must block pull requests, pushes, and release
  candidates; scheduled full-history scanning must produce fresh evidence that
  release admission requires.
- `cargo-audit`, `cargo-deny`, `cargo-about`, and the
  `cargo-unmaintained` advisory must run against the committed lockfile and
  versioned policy.
- Release publication must be impossible unless the admission job has passed
  for the exact tag commit.
- Existing archive names, target coverage, `.sha256` sidecars, and
  `cargo-binstall` resolution must remain compatible.

### Technical requirements

- Existing action SHA pins, the pinned nightly toolchain, Polonius,
  all-target/all-feature quality gates, Kani, and checksum contracts must not
  be weakened or replaced.
- New actions and downloaded executables must be pinned immutably and verified
  by the repository's established supply-chain policy.
- Evidence must be deterministic, machine-readable, retained with the workflow
  run, and linked to the exact commit and tool versions that produced it.
- Secret findings must be redacted from logs; suppressions must be narrow,
  reviewed, and expiring.
- The policy must fail closed on unknown, stale, missing, or contradictory
  evidence.

### Measurable acceptance criteria

- Across five repeated runs on the same runner class, the candidate's median
  representative-workload time must be no more than 5% slower than the baseline
  for any supported target.
- The stripped release binary must be no more than 10% larger than the
  baseline for any supported target. The report must include unstripped size as
  a diagnostic, but the stripped-size limit is the admission criterion.
- The release-profile test suite must pass for all supported targets and must
  cover every production `debug_assert` site and selected arithmetic boundary
  cases without assertion or overflow failures.
- A clean candidate must produce zero unwaived Gitleaks findings, zero denied
  or unknown licences, zero unwaived dependency advisories, a complete notice
  inventory, and a current history-scan result.
- The admission manifest must identify one passing result for every required
  gate, the exact tag commit, every release target, and every archive/sidecar
  digest. Any missing field fails admission.
- Existing release workflow contract tests and checksum tests must continue to
  pass without changing their expected archive or sidecar names.

## Compatibility and migration

The profile change may expose latent arithmetic defects or assertions that were
previously compiled out. That is an intentional compatibility check: supported
inputs must be repaired or covered before the profile becomes blocking. Users
should not observe a command-line change for valid inputs, but release binaries
may be modestly slower or larger within the stated budgets.

Migration is phased so each new failure has a useful baseline:

### Phase 1: Measure and inventory

Record the current release profile, assertion call sites, representative
workloads, target sizes, release archive layout, and existing gate outputs. Add
policy-file and evidence-manifest schemas without making publication depend on
them.

### Phase 2: Enable release invariants

Add the explicit release profile, boundary tests, and performance/size
measurements. Run the new checks in required CI with a short baseline period;
fix defects and tune only the documented workload and budgets. Keep Polonius,
Kani, and all-target/all-feature gates unchanged.

### Phase 3: Enforce security and dependency policy

Introduce the pinned working-tree scan, scheduled history scan, dependency
advisories, licence checks, notices, and unmaintained-dependency review. Triage
existing findings with expiring waivers before making the checks blocking.

### Phase 4: Turn on admission

Require the evidence manifest and admission job in the release workflow. Make
the publishing job depend on admission, verify the tag-to-commit binding, and
retain the existing archive and checksum-sidecar tests. Exercise the workflow
with dry runs and intentionally incomplete evidence before enabling real
publication.

Rollback is limited to disabling publication while retaining the checks and
evidence. The release profile must not be removed merely to bypass a failed
admission; if a threshold is unsafe, record a revised threshold through a
reviewed decision and repeat the measurement phase.

## Alternatives considered

### Option A: Keep Cargo defaults and rely on tests

This avoids release overhead and configuration changes, but leaves six
production assertions disabled and does not detect release-only overflow. It
does not address secret history, dependency policy, or the absence of a single
publication decision, so it is rejected.

### Option B: Copy VTCode's workflows verbatim

The commit-pinned [VTCode release profile][vtcode-release-profile],
[secret scan][vtcode-secret-scan], and
[licence and unmaintained checks][vtcode-dependency-checks] are useful
precedents. Copying them would import assumptions about action pinning,
workflow permissions, thresholds, and artefact layout. In particular, the
secret-scan workflow at that commit uses an unpinned checkout, which is not
acceptable for Netsuke. This option is rejected in favour of adapting the
intent to Netsuke's contracts.

### Option C: Run checks but leave publication independent

This provides diagnostic information but permits a release job to publish when
a check is missing, stale, or failed. It makes the evidence advisory rather
than an invariant and is rejected because release integrity depends on a
fail-closed admission decision.

## Open questions

- Should the history scan freshness window be 24 hours, seven days, or another
  interval that matches the release cadence?
- Which representative workloads and runner classes best predict user-visible
  performance across all supported targets?
- Should a transitive dependency reported by `cargo-unmaintained` ever become a
  hard blocker, and who owns that escalation?
- Where should the generated `cargo-about` notice inventory be retained, and
  which release formats must embed it rather than publish it as a sidecar?
- Should release evidence be signed or attested by an external service, or is
  commit-bound workflow retention sufficient for the first implementation?
- What approval authority and maximum duration should apply to each waiver
  category?

## References

- [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html).
- [Gitleaks](https://github.com/gitleaks/gitleaks).
- [`cargo-about`](https://github.com/EmbarkStudios/cargo-about).
- [`cargo-audit`](https://github.com/RustSec/rustsec/tree/main/cargo-audit).
- [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny).
- [`cargo-unmaintained`](https://github.com/trailofbits/cargo-unmaintained).
- [VTCode upstream repository](https://github.com/vinhnx/VTCode).
- [VTCode release profile at commit `f188bcb0`][vtcode-release-profile].
- [VTCode secret scan at commit `f188bcb0`][vtcode-secret-scan].
- [VTCode dependency checks at commit `f188bcb0`][vtcode-dependency-checks].

[vtcode-dependency-checks]: https://github.com/vinhnx/VTCode/blob/f188bcb0e47d7386886ab0c3db7e338b297a3d07/.github/workflows/ci.yml#L165-L230
[vtcode-release-profile]: https://github.com/vinhnx/VTCode/blob/f188bcb0e47d7386886ab0c3db7e338b297a3d07/Cargo.toml#L415-L430
[vtcode-secret-scan]: https://github.com/vinhnx/VTCode/blob/f188bcb0e47d7386886ab0c3db7e338b297a3d07/.github/workflows/secret-scan.yml#L1-L65

## Recommendation

Adopt this RFC as the release-hardening direction: enable the explicit release
invariants, add pinned and fail-closed security and dependency checks, measure
their cost, and make exact-commit admission evidence a prerequisite for
publication. This gives Netsuke the useful release-health lessons from VTCode
while preserving Netsuke's existing SHA pins, Polonius, Kani, all-target and
all-feature quality gates, and checksum contracts.
