# Architectural decision record (ADR) 026: Validate the Windows final component through a same-handle reparse-point open

## Status

Accepted.

## Date

2026-09-18.

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

A Windows-only regression test asserts that a junction fixture — created with
`mklink /J`, which needs no privilege — is rejected by all four filters under
the default policy. The fixture follows the repository's "create the requested
file type or skip because that file type is unavailable" rule: the test asserts
the entry really carries `FILE_ATTRIBUTE_REPARSE_POINT` before rendering, so it
cannot pass against a plain directory. The existing symlink test continues to
cover the file-symlink reparse case, and the `follow_symlinks` opt-in test
covers the retained `open_with` path.
