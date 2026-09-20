//! Which sources the suppression scan reads, and how the walk finds them.
//!
//! The roots are the crate's answer to "what is a compiled source here", and
//! both the scan and the coverage invariant are built on that answer, so the
//! two must agree. They are kept together for that reason: a change to the root
//! list, to the skip list, or to either walk is a change to the same rule, and
//! a reader who has only one half of it cannot tell whether the gate still
//! covers what it claims to.
//!
//! The crate root holds the assertions; this module holds the traversal they
//! are made of.

use anyhow::{Context, Result};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Roots, relative to the workspace root, whose Rust sources the crate compiles.
///
/// The list is the roots the workspace lints rather than the ones a convention
/// calls source. `tests` is included because `cargo clippy --workspace
/// --all-targets` lints integration-test targets and the modules they wire in,
/// exactly as it lints the library, so an inner attribute there silences the
/// policy for the whole test binary. Measured: with an integration target that
/// reads the environment, the target fails to compile without the attribute and
/// compiles clean with it. Leaving `tests` out would hand the evasion a second
/// home. `benches` and `examples` are included for the same reason: Cargo
/// discovers targets in both, and a benchmark or example target is compiled and
/// linted like any other, so its own environment reads are governed by the same
/// policy. `examples` holds no Rust source today; it is listed because the
/// directory Cargo discovers is governed whether or not it is currently
/// occupied, and the coverage assertion below is what makes that safe to say —
/// it fails if any Rust source anywhere in the workspace is outside these
/// roots, so a root that is misspelled, or a target location nobody predicted,
/// is caught rather than silently excusing its sources.
pub(super) const COMPILED_SOURCE_ROOTS: [&str; 6] = [
    "src",
    "build_l10n_audit",
    "test_support/src",
    "tests",
    "benches",
    "examples",
];

/// The fewest Rust sources the workspace can hold while the walk still works.
///
/// Well below the count a healthy tree carries, so ordinary growth and pruning
/// never trip it; a walk that descended nowhere, or stopped after one directory,
/// would.
pub(super) const MINIMUM_WORKSPACE_SOURCES: usize = 100;

/// Compiled sources that sit outside every [`COMPILED_SOURCE_ROOTS`] root.
///
/// `build.rs` is compiled and linted by `cargo clippy --all-targets`, and it is
/// where a build script's own environment reads live, so an inner attribute
/// there silences the policy for the build script exactly as it would in a
/// library source. It is listed separately because the walk is over directories
/// and this is a file.
pub(super) const STANDALONE_COMPILED_SOURCES: [&str; 1] = ["build.rs"];

/// Append every source the scan reads beneath `directory`, with its contents.
///
/// The order is the directory's, not this function's: `read_dir` reports
/// entries as the filesystem lists them and std promises nothing about that
/// order. Nothing here depends on it — the assertion over the result is about
/// the set of findings, and each source is read in full — so no caller should
/// either. The failure message sorts its findings for the same reason; see
/// [`build_error_message`](super::build_error_message).
///
/// [`MACHINE_LOCAL_DIRECTORIES`] is skipped by name, at whatever depth it
/// appears, exactly as the coverage walk below skips it. A scanned root is a
/// directory a contributor edits, but a cache inside one is still a cache: the
/// entry names are relative to a machine, so a `.uv-cache` under `tests/` holds
/// third-party or generated sources that are not repository content. Reading
/// them would make the verdict depend on which tools had run — a vendored crate
/// that suppressed the policy would fail the gate on one machine and pass on
/// another — and the failure would be near-impossible to diagnose, because such
/// a path is git-ignored, so it appears in no diff and in no `git status`.
///
/// Skipping here does not open a hole, because this walk is not what decides
/// which sources are governed: [`is_scanned`] does. A machine-local name is one
/// the repository ignores, so a source under it is not one this gate is
/// answerable for, and the walk's self-test fails if that ever stops holding
/// for a name in the list.
///
/// An absent directory is an empty one rather than an error. A root that does
/// not exist holds no sources, so there is nothing here to miss, and the
/// coverage invariant is what keeps that from becoming a hole: the walk below
/// finds every Rust source in the workspace and fails on any that is not
/// scanned, so a root whose sources moved elsewhere — renamed, misspelled,
/// deleted — is reported there by name. Failing here instead would mean
/// reporting the same fact twice, in the less useful form of an I/O error that
/// does not say which source went uncovered. Every other I/O error still
/// propagates: an unreadable directory is a real failure, and the distinction
/// between "holds nothing" and "could not be read" is worth keeping.
pub(super) fn collect_rust_sources(
    root: &Dir,
    directory: &Utf8Path,
    sources: &mut Vec<(String, String)>,
) -> Result<()> {
    let entries = match root.read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).with_context(|| format!("read `{directory}`")),
    };
    for entry_result in entries {
        let entry = entry_result.with_context(|| format!("read an entry of `{directory}`"))?;
        let name = entry
            .file_name()
            .with_context(|| format!("read an entry name in `{directory}`"))?;
        if MACHINE_LOCAL_DIRECTORIES.contains(&name.as_str()) {
            continue;
        }
        let path = format!("{directory}/{name}");
        let file_type = entry
            .file_type()
            .with_context(|| format!("read the file type of `{path}`"))?;
        if file_type.is_dir() {
            collect_rust_sources(root, Utf8Path::new(&path), sources)?;
        } else if is_readable_source(&name) {
            sources.extend(read_source(root, &path)?.map(|text| (path, text)));
        }
    }
    Ok(())
}

/// Return whether an entry name is a Rust source.
pub(super) fn is_rust_source(name: &str) -> bool {
    Utf8Path::new(name).extension().is_some_and(|it| it == "rs")
}

/// Read one source, returning `None` when the file is not text at all.
///
/// The wider read set admits files whose being Rust is not something this walk
/// can see — anything named by a `#[path]` — and a directory under a scanned
/// root may hold a binary that no `#[path]` names. Rust source is UTF-8 by
/// definition, so a file that will not decode cannot be a module the compiler
/// reads, and skipping it is the honest classification rather than a failure.
///
/// Every other error propagates, and that half is the load-bearing one: a file
/// that *is* text but could not be read is a source that went unscanned, which
/// is the silent non-coverage this whole contract exists to prevent, so it must
/// fail rather than be passed over. The two cases are distinguished by
/// [`is_not_text`] rather than by a catch-all, and the distinction is pinned by
/// a test — a filter that swallowed every error would be indistinguishable from
/// this one on the walk the suite actually performs, so it is measured against
/// an unreadable path directly.
pub(super) fn read_source(root: &Dir, path: &str) -> Result<Option<String>> {
    match root.read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if is_not_text(error.kind()) => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read {path}")),
    }
}

/// Return whether an I/O error means the file is not text this walk can read.
///
/// `InvalidData` is what a decode failure reports, and `NotFound` is the file
/// that was listed at the start of the walk and is gone by the time it is read
/// — a concurrent build writing under a scanned root, or a lint run that
/// deleted a fixture. Both mean "there is no source text here", which is a
/// classification rather than a failure. Anything else means the file is there
/// and could not be read, which is a source this gate did not scan, and that
/// must fail. The list is a function of its own so the error filter is a named
/// rule rather than a pattern inside a `match`, and so it can be exercised
/// directly by a test: a catch-all here would look identical from the walk the
/// suite performs, since every path that walk touches is readable.
pub(super) const fn is_not_text(kind: std::io::ErrorKind) -> bool {
    matches!(
        kind,
        std::io::ErrorKind::InvalidData | std::io::ErrorKind::NotFound
    )
}

/// Return whether an entry name is one the scan reads.
///
/// A `.rs` file is read by name, and so is every other file that is not
/// dot-prefixed — because a module's file need not be named `.rs`. Rust reads
/// a module from whatever `#[path = "..."]` names, with no extension test of
/// its own: `#[path = "suppressed.inc"] mod suppressed;` compiles, the module
/// may open with `#![allow(clippy::disallowed_methods)]`, and that inner form
/// is exactly what `clippy::allow_attributes` does not report. Measured on a
/// probe crate: the module compiles where the same file without the attribute
/// exits 101, so an extension filter names the compiled sources by a spelling
/// the language does not require, and the one reader who cares about that
/// filter is the one who would rather the source went unread.
///
/// Reading every file is the safer direction because the two mistakes are not
/// symmetric: a file read but never compiled costs a failure message naming a
/// real file, while a file compiled but not read hides a suppression, which is
/// the failure this contract exists to prevent. The file's other rule breaks
/// the same way for the same reason — see [`MACHINE_LOCAL_DIRECTORIES`].
///
/// The breadth is bounded rather than open-ended. The walk is already confined
/// to [the scanned roots](COMPILED_SOURCE_ROOTS), the skip list still removes
/// the caches and `target`, and a name beginning with a dot stays out because a
/// dot-file under a source root is tooling state — `.gitignore`,
/// `.editorconfig`, `.rustfmt.toml` — rather than anything a `#[path]` names.
/// A `.rs` file is read whatever its name, so the dot rule can only ever narrow
/// the files that were never Rust sources to begin with.
///
/// Measured against the tree this was written on: 216 files across the scanned
/// roots are read by the wider rule, and the scan finds nothing in any of them.
/// The scan is token-wise over source text, so prose that quotes an attribute
/// in a comment, a string, or a snapshot of either is blanked before the
/// matcher sees it; only a file that really opens with the attribute would be
/// reported, and such a file is one `#[path]` away from being compiled.
pub(super) fn is_readable_source(name: &str) -> bool {
    is_rust_source(name) || !name.starts_with('.')
}

/// Directories the coverage walk does not descend into.
///
/// These are the machine-local directories the repository declares in
/// `.gitignore`, plus the compiler's output and the caches of the tools this
/// repository runs. Each belongs to a machine rather than to the repository, so
/// each may hold a Rust source that is not a source here: `target` holds
/// generated and vendored output, and a package cache holds the extracted
/// sources of third-party crates — a Python distribution with a Rust extension
/// ships `.rs` files with it. Descending into one would make the gate turn on
/// what a cache happened to contain on one machine, which is the thing it must
/// not do.
///
/// An entry is skipped by *name*, at whatever depth it appears, rather than by
/// comparing the whole workspace-relative path against a list of root
/// directories. That is the rule `.gitignore` already states — its patterns
/// carry no leading slash, so `target/`, `memories/`, and `__pycache__/` are
/// ignored at every level, and the list is drawn from them. Matching by name
/// keeps the two in step: a name git will not track is not a name a compiled
/// source can live under without `git add -f`, which is deliberate
/// circumvention rather than an accident this invariant is shaped to catch.
/// Verified both ways: `git ls-files` finds no tracked path beneath any of the
/// fifteen names at any depth, and every nested occurrence in the tree sits
/// inside another skipped directory or a cache.
///
/// That appeal to `.gitignore` is only sound while it holds for every name, so
/// it is enforced rather than trusted. It did not hold twice: `.netsuke` is
/// netsuke's own runtime state, and `.ruff_cache` a tool cache, but both were
/// skipped while the repository's `.gitignore` declined them — every sibling
/// cache is listed, and they were missed — so a `.rs` file placed under either
/// would have been tracked, compiled, skipped by the walk, and reported by
/// nobody. Both names are now in `.gitignore` like their siblings, and the
/// walk's self-test requires each skipped name to be one the repository's own
/// rules ignore, with `.git` as the single named exception.
///
/// The list is named rather than "anything dot-prefixed", and that distinction
/// is the point. A dot-directory is not evidence of a cache: `.config`,
/// `.github`, and `.rules` are tracked repository content, and Cargo will
/// compile a target declared under any directory at all, hidden or not. A walk
/// that skipped every dot-prefixed name would therefore neither scan nor report
/// a target sitting in one, which is precisely the silent non-coverage this
/// invariant exists to prevent. Walking them instead turns that into a loud
/// failure that names the source. Where the two rules disagree, the tie breaks
/// towards reporting: an entry that should have been here but is missing costs
/// a false failure that names a real file, while an entry that should not be
/// here hides a source.
pub(super) const MACHINE_LOCAL_DIRECTORIES: [&str; 15] = [
    "__pycache__",
    ".claude",
    ".crush",
    ".git",
    ".grepai",
    ".hypothesis",
    ".memdb",
    ".netsuke",
    ".pytest_cache",
    ".ruff_cache",
    ".uv-cache",
    ".uv-tools",
    ".vtcode",
    "memories",
    "target",
];

/// Append every Rust source the coverage invariant governs, in no set order.
///
/// The walk descends everything but [`MACHINE_LOCAL_DIRECTORIES`], which is
/// where the generated output and the third-party sources live. Everything
/// else is repository content until proven otherwise — including a
/// dot-directory — so a compiled source anywhere in the workspace is found and
/// named rather than passed over.
pub(super) fn collect_all_sources(
    root: &Dir,
    directory: &Utf8Path,
    found: &mut Vec<String>,
) -> Result<()> {
    for entry_result in root
        .read_dir(directory)
        .with_context(|| format!("read `{directory}`"))?
    {
        let entry = entry_result.with_context(|| format!("read an entry of `{directory}`"))?;
        collect_source_entry(root, directory, found, &entry)?;
    }
    Ok(())
}

/// Classify one entry of `directory`, recursing where the walk must descend.
///
/// The early returns are the whole classification: a machine-local name is
/// skipped, a directory is descended, a Rust source is kept, and anything else
/// is passed over. Only the last two touch `found`, and only a directory
/// recurses, so the caller above does nothing but open the directory and hand
/// its entries here.
fn collect_source_entry(
    root: &Dir,
    directory: &Utf8Path,
    found: &mut Vec<String>,
    entry: &cap_std::fs_utf8::DirEntry,
) -> Result<()> {
    let name = entry
        .file_name()
        .with_context(|| format!("read an entry name in `{directory}`"))?;
    if MACHINE_LOCAL_DIRECTORIES.contains(&name.as_str()) {
        return Ok(());
    }
    let path = join_path(directory, &name);
    let file_type = entry
        .file_type()
        .with_context(|| format!("read the file type of `{path}`"))?;
    if file_type.is_dir() {
        return collect_all_sources(root, Utf8Path::new(&path), found);
    }
    if is_rust_source(&name) {
        found.push(path);
    }
    Ok(())
}

/// Join a directory and an entry name, keeping the walk root's paths bare.
pub(super) fn join_path(directory: &Utf8Path, name: &str) -> String {
    match directory.as_str() {
        "." => name.to_owned(),
        _ => format!("{directory}/{name}"),
    }
}

/// Return whether `path` is one of the sources [`compiled_sources`] reads.
///
/// A source counts as covered when it sits beneath a scanned root or is one of
/// the standalone sources. Coverage is what makes the scan's silence mean
/// something: a source outside this set is not "clean", it is unread, and the
/// difference is the whole point of the invariant that calls this.
pub(super) fn is_scanned(path: &str) -> bool {
    STANDALONE_COMPILED_SOURCES.contains(&path)
        || COMPILED_SOURCE_ROOTS.iter().any(|root| {
            path.strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('/'))
        })
}

/// Read every compiled source the scan governs, with its workspace-relative path.
pub(super) fn compiled_sources() -> Result<Vec<(String, String)>> {
    let crate_root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the workspace root")?;
    let mut sources = Vec::new();
    for root_path in COMPILED_SOURCE_ROOTS {
        collect_rust_sources(&crate_root, Utf8Path::new(root_path), &mut sources)
            .with_context(|| format!("walk the `{root_path}` source root"))?;
    }
    for path in STANDALONE_COMPILED_SOURCES {
        let contents = crate_root
            .read_to_string(path)
            .with_context(|| format!("read {path}"))?;
        sources.push((path.to_owned(), contents));
    }
    Ok(sources)
}
