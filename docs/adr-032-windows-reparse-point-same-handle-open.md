# Architectural decision record (ADR) 032: Validate the Windows final component through a same-handle reparse-point open

## Status

Accepted.

## Date

2026-09-18

## Context and problem statement

The four file-reading filters — `contents`, `linecount`, `hash`, and `digest` —
share one safety policy, enforced once in
`src/stdlib/path/fs_utils.rs::open_file_checked`. That function decides what
may be opened and then reads only through the handle it opened.

On Unix the decision and the read share one open. `apply_unix_open_flags` adds
`O_NOFOLLOW` to the open itself when the default policy is in force, so the
kernel refuses a symlink final component as part of the same call that produces
the handle. There is no window between the check and the read.

The Windows implementation had no equivalent. `reject_windows_symlink` ran a
`symlink_metadata` call *before* the open and then opened the path as a
separate step. Those two operations are not atomic: a final component that is a
regular file at check time can be replaced before the open, so a caller able to
write to the containing directory can win the race and have the read follow a
prohibited reparse point. The pre-open check was the platform's best available
guard, but it was never race-free.

The gap is a hardening concern rather than a regression, and it does not defeat
the ordinary case. It matters because the same policy is load-bearing for all
four filters, and a check-then-open race is exactly the defect the Unix path
was built to avoid.

## Decision drivers

- The Windows default path must reject a symlink, mount point (junction), and
  any other prohibited reparse point without relying on a separate pre-open
  metadata check.
- The Unix `O_NOFOLLOW` path must be unchanged, and `open_file_checked` must
  remain the single shared entry point for all four filters.
- The open and the policy decision must not be able to diverge.
- The fix should not weaken the capability-based filesystem posture that the
  rest of `src/stdlib/path/` is built on (`cap_std` directory handles rather
  than ambient absolute paths), nor widen the crate's `unsafe` surface.

## Requirements

### Functional requirements

- A final component that is a reparse point is rejected by the default policy on
  Windows, whether it is a file symlink, a directory symlink, a junction, a
  volume mount point, or any other tag such as a deduplication or cloud
  placeholder.
- A final component that is not a regular file is rejected on every platform.
- `follow_symlinks=true` continues to resolve a link to its target and read it.

### Technical requirements

- The reparse-point judgement and the regular-file judgement are both taken from
  the handle that the read will use.
- The `unsafe_code` workspace lint stays at `forbid`; no new direct dependency
  is added for this change.
- Ambient filesystem access stays out of the module, so Whitaker's
  `no_std_fs_operations` policy needs no new exclusion.

## Options considered

### Option A: Raw Win32 FFI in a dedicated Windows submodule

Open the final component with `CreateFileW`, passing
`FILE_FLAG_OPEN_REPARSE_POINT` so the open does not follow the reparse point,
then inspect the handle with `GetFileInformationByHandleEx` and
`FileAttributeTagInfo`, and adopt the handle with `FromRawHandle`.

This closes the race and is the literal reading of the original plan. It was
rejected on three grounds.

- It opens an **ambient absolute path** (`parent.dir_path.join(entry)`), because
  `CreateFileW` has no directory-handle-relative form. That discards the
  `cap_std` capability handle the rest of the module is built on, and it
  requires a `dylint.toml` `excluded_paths` entry to exempt the new module from
  Whitaker's `no_std_fs_operations` lint — an exemption that weakens the policy
  for a check the repository can already perform inside the sandbox.
- It forces a workspace-wide downgrade of `unsafe_code` from `forbid` to
  `deny`, and adds a direct `cfg(windows)` `windows-sys` dependency, for what
  amounts to three `u32` constants and two calls.
- It is unnecessary, because Option B delivers the same guarantee through
  existing public safe APIs.

### Option B: `cap_std` Windows `OpenOptionsExt::custom_flags` and `MetadataExt::file_attributes` (chosen)

`cap_std` re-exports the Windows-only `OpenOptionsExt` trait, whose
`custom_flags` method is OR-ed straight into the `dwFlagsAndAttributes`
argument of the open, and the Windows-only `MetadataExt` trait, whose
`file_attributes` method is populated from `BY_HANDLE_FILE_INFORMATION` read
from **the open handle itself**. Both are safe, public, and already reachable
through the `cap_std::fs_utf8` re-exports the module uses.

The Windows no-follow branch therefore sets
`FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS` on an ordinary
`parent.handle.open_with(...)` call, and then reads `file_attributes()` from
the resulting handle. The open and the judgement share one handle, exactly as
the Unix path does.

This route is notable for what it does *not* need: no `unsafe`, no new
dependency, no lint relaxation, and no dylint exclusion. It is also the same
`OpenOptionsExt` trait the Unix branch of `open_file_checked` already uses to
set `O_NOFOLLOW`, so the two platforms reach their guarantees through parallel
mechanisms rather than one privileged path.

### Option C: Keep the pre-open check and document the race

Take no code change and record the residual risk. Rejected: the repository
would keep a check-then-open window in a security-relevant policy when a
same-handle alternative is available through an already-vendored dependency.

## Decision outcome

Adopt Option B.

`src/stdlib/path/fs_utils.rs::open_file_checked` keeps its shape. Its Windows
no-follow branch now applies `FILE_FLAG_OPEN_REPARSE_POINT` through
`OpenOptionsExt::custom_flags` before a single `open_with` call, and then
rejects the opened handle when either

- the handle's `file_attributes()` carries `FILE_ATTRIBUTE_REPARSE_POINT`, or
- the handle's metadata reports anything other than a regular file.

The first test rejects **every** reparse tag, not only the ones `std` reports
as symlinks. `FileType::is_symlink` is a test on the tag *value*: it is true
only for name-surrogate tags (bit 29 set), which covers file symlinks,
directory symlinks, junctions, and volume mount points — but is false for every
other tag, such as a deduplication or cloud placeholder. For those, an open
that follows the point succeeds and returns the target's handle, while a check
phrased as "is this a symlink" sees nothing to refuse. Testing the attribute
bit asks "is this a reparse point at all", which is the policy the callers
actually want, and it needs no knowledge of which tags a future Windows release
may mint.

`reject_windows_symlink` is deleted; its pre-open `symlink_metadata` call is
gone, and nothing replaces it.

### Why the race is closed by construction

The two facts the policy needs — "is this a reparse point?" and "is this a
regular file?" — are both read from the handle that was opened, and that same
handle is what the caller then reads bytes from. Windows resolves the path
once, when the handle is created; subsequent queries on the handle cannot be
redirected by a concurrent rename or replace in the containing directory. There
is therefore no interval between the decision and the read in which the
filesystem object can change identity. This is the same argument the Unix path
relies on, where `O_NOFOLLOW` is a property of the one `open` call.

## Known risks and limitations

- `FILE_FLAG_BACKUP_SEMANTICS` is required to open a directory, which the
  regular-file check must be able to do in order to reject directories with the
  documented diagnostic rather than an open failure.
- The regular-file test is unchanged and platform-shared: it still runs after
  the platform-specific open, so directories and other non-regular objects are
  rejected identically everywhere.
- Reparse points outside the final component are out of scope. A symlinked or
  junctioned parent directory is resolved when the parent handle is opened by
  `parent_dir`, which is the pre-existing behaviour on both platforms and is
  not changed here.
- A junction cannot be *traversed* through a capability at all, under either
  policy: the refusal is part of resolving the link, which happens before the
  open policy is consulted. `mklink /J` records an absolute target, and the
  resolver rejects an absolute link destination outright, reporting
  `escape_attempt()` as `PermissionDenied`.
- The trigger is **absoluteness of the link target, not escape from the
  capability**. An earlier draft of this record said "a destination that leaves
  the capability", which is the wrong mechanism and misdescribes what an
  operator can rely on. The resolver never compares the resolved path against
  the capability root, so it cannot tell an absolute target that stays inside
  from one that does not. Both are refused. Verified two ways:

  - On Unix the kernel does this in `openat2` with `RESOLVE_BENEATH`, which
    rejects any absolute link target, and `EXDEV` maps to `escape_attempt()`.
    A probe on Linux confirmed it directly: two symlinks pointing at the *same
    file inside* the capability, one written relatively and one absolutely,
    with the opt-in policy in force — the relative link opened and the absolute
    link was refused with "a path led outside of the filesystem". Same file,
    same containment, different result; only the spelling of the target
    differed. The Windows claim rests on the resolver's shared
    `PrefixOrRootDir => escape_attempt()` arm, which is reached for a link
    destination containing a prefix or root component, and cannot be
    exercised on Linux.
  - Consequence for tests and docs: a junction is refused under *either*
    policy, so it can never demonstrate a successful opt-in follow. The opt-in
    policy's follow is exercised by the file-symlink opt-in test, which uses a
    relative target; the default policy's refusal of a junction needs no
    traversal, because `FILE_FLAG_OPEN_REPARSE_POINT` returns the reparse point
    itself.
- The Unix path is untouched. `apply_unix_open_flags` and `restore_blocking`
  keep their current behaviour byte for byte.

## Architectural rationale

The change keeps the policy boundary where the design already puts it. All four
filters still enter through one `open_file_checked`, which still resolves a
`ParentDir` capability handle and still rejects non-regular files from the
opened handle. The Windows branch simply stops approximating `O_NOFOLLOW` from
outside the open and starts requesting the platform's own equivalent as part of
it, through the same trait family the Unix branch uses.

Choosing the in-sandbox route over raw FFI also preserves the dependency
direction the module is built on. `src/stdlib/path/` reaches the filesystem
through `cap_std` handles; an FFI open of an ambient absolute path would have
introduced a second, weaker access path into the same module and required a
lint exemption to permit it.

## Verification

The guarantee is carried at three layers: a compile-time assertion, a unit
test, and an integration test.

The compile-time layer is a `const _: () = { ... }` block in
`windows_reparse_tests.rs`. Runtime tests in that file execute only on a
Windows host, so a regression could reach a merge on the strength of a green
Linux run; a `const` assertion has no such dependency, because rustc evaluates
it whenever the module is compiled, and `Windows / lint-windows` compiles it on
every push through `cargo clippy --all-targets`. It pins both branches of
`open_flags` and both outcomes of `is_prohibited_reparse_point`, so a change to
either policy fails the Windows build rather than only a test run. (Before
issue 743 was fixed this layer mattered most, because the test lane did not
reach the module at all; it remains the layer that fails fastest.)

The integration test asserts that a junction fixture — created with
`mklink /J`, which needs no privilege — is rejected by all four filters under
the default policy. The fixture follows the repository's "create the requested
file type or skip because that file type is unavailable" rule: the test asserts
the entry really carries `FILE_ATTRIBUTE_REPARSE_POINT` before rendering, so it
cannot pass against a plain directory.

The unit test covers the property the integration layer cannot see. `std`
reports a junction as a symlink, so `metadata.is_file()` would refuse the
directory even if the flag were never passed. Only the handle distinguishes
them, so the unit test opens the junction and asserts its attributes carry
`FILE_ATTRIBUTE_REPARSE_POINT` — the one check that fails when
`FILE_FLAG_OPEN_REPARSE_POINT` stops being applied — and then asserts the
policy refuses it, with the ordinary target directory as a control so the
refusal is shown to be about the link rather than about directories generally.
Because the capability resolver cannot reach the junction's absolute target,
that handle is taken through the ambient authority.

The existing symlink test continues to cover the file-symlink reparse case, and
the `follow_symlinks` opt-in test covers the retained follow path with a
relative-target symlink.

The compile-time assertions duplicate four decisions the unit test also makes,
which is deliberate rather than redundancy: the unit test decides them on a
Windows host, and the assertions decide them on the way to a Windows build.

### How far this evidence actually extends

Stated plainly, because the tests above are Windows-only, and because this
section previously recorded the opposite conclusion.

`Windows / build-test-windows` was red repository-wide until `061182b1` landed
on `main` with the write-side shutdown fix for the network-fixture race tracked
as issue 743. Before that, and on every head of this branch, the lane halted
inside `stdlib::network` before the nextest run reached `stdlib::path`: on
commit `2d8e5305` the run ended at 1078/2901 tests (1077 passed, 1 failed, 2
skipped), dying on
`stdlib::network::redirect::error_tests::protocol_failures_are_classified_from_a_live_response`,
with the strings `windows_reparse` and `junction` appearing **zero** times in
the whole job log. So no case described in this section had ever executed in
continuous integration.

**That is no longer true, and the tests now run.** This branch was rebased onto
`061182b1`, and on head `5d2dab68` the lane completes:
`Summary [ 341.726s] 2906 tests run: 2906 passed (1 slow), 2 skipped`. That
total is 2898 plus this branch's 8 — four unit tests and four
`reading_filters_reject_a_junction` cases.
`the_default_handle_is_the_junction_not_its_target` passes, as do all four
junction cases, and `windows_reparse` appears four times where it previously
appeared zero. The figure is cited with its commit because it is the claim here
most likely to age.

A Windows-only test can pass without testing anything, by skipping its own
fixture, and this repository's skip convention returns `Ok(())` — so a skipped
fixture is recorded as a **pass**, not as a skip. Neither the skip total nor
the pass total can therefore distinguish "asserted against a junction" from
"quietly did nothing". Two things can.

First, the fixtures have exactly one quiet arm, and it is narrow. Both
`junction_fixture` variants return `None` only on `ErrorKind::NotFound` from
`Command::new("cmd")`. Every other outcome is a failure: a `mklink` that exits
non-zero trips an `ensure!` quoting its stderr, and a spawn that fails for any
other reason propagates. So on a host where `cmd.exe` can be spawned, the only
ways to finish are "the junction was created" or "the test failed" — there is
no third way to pass. `cmd.exe` ships with the `windows-latest` image, which is
what makes that arm unreachable here.

Second, a junction that was created is checked before it is used.
`require_real_junction` reads the entry's attributes *without following the
link* and fails unless `FILE_ATTRIBUTE_REPARSE_POINT` is set, precisely so that
a plain directory cannot stand in for a reparse point. A fixture that succeeded
but produced an ordinary directory fails the test rather than silently
inverting the assertions.

The residual uncertainty is that the first step reasons about the runner image
rather than measuring it: the log does not record that `cmd` was spawnable,
because nextest hides the captured output of passing tests and the CI lane
passes no `--success-output`. (The `success-output = "immediate"` entries in
`.config/nextest.toml` cover three unrelated test groups.) So the absence of
the fixtures' skip lines proves nothing on its own, and is not relied on here.
What the run count does establish is that the cases ran at all: `main` reports
2898 tests and this head 2906, the difference being exactly the eight new
cases, all of which appear as `PASS`.

What *is* verified on this change, and by what. Native CI and a local probe,
and the boundary between them is stated at the end.

**Native Windows CI compiles and lints every Windows-gated line, tests
included.** `Windows / lint-windows` runs `make lint-clippy`, which expands to
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
then Whitaker's dylint suite over the same target and feature selection.
`--all-targets` pulls in the library's `cfg(test)` module and the integration
test targets, so `windows_reparse.rs`, `windows_reparse_tests.rs`, and the
junction fixture in `file_type_tests.rs` are all compiled on Windows itself,
under `-D warnings`. That job is green on this head. It is the only route that
compiles the Windows-gated lines with the platform's own toolchain rather than
an approximation of it.

**A local probe crate covers the development loop.** The main crate cannot be
cross-compiled on this host — `ring` needs MSVC's `lib.exe` — so Windows-gated
edits were iterated against a throwaway crate mirroring the module tree. Two
tools are needed there and neither subsumes the other: `cargo dylint` runs
`cargo check`, so it applies the Whitaker lints and no clippy lint, while
`cargo clippy` applies no Whitaker lint. Each was shown to be live by injecting
a defect it should catch, confirming a non-zero exit, and reverting. This is
what made the intermediate commits CI-worthwhile; it is a convenience, not the
guarantee.

**What the Windows lane shows, and what it does not.** Compilation under
`-D warnings` is a strong statement about the code and a weak one about its
behaviour, so the two lanes are described separately above rather than as one
result: the lint lane compiles the junction tests, and the test lane executes
them. What no test covers is the adversarial case the race analysis turns on. A
test cannot force a rename to land between two filesystem calls, because the
change removed the second call: the policy decision is read from the handle the
read uses, and this section is written as a construction argument for exactly
that reason. The runtime behaviour is now demonstrated on the platform it
governs; the *absence* of a window between the decision and the read remains an
argument from handle semantics rather than something a test could schedule.
