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

/// Append every `.rs` source beneath `directory`, with its contents, in order.
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
    directory: &str,
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
            collect_rust_sources(root, &path, sources)?;
        } else if is_rust_source(&name) {
            let contents = root
                .read_to_string(&path)
                .with_context(|| format!("read {path}"))?;
            sources.push((path, contents));
        }
    }
    Ok(())
}

/// Return whether an entry name is a Rust source.
pub(super) fn is_rust_source(name: &str) -> bool {
    Utf8Path::new(name).extension().is_some_and(|it| it == "rs")
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
    directory: &str,
    found: &mut Vec<String>,
) -> Result<()> {
    for entry_result in root
        .read_dir(directory)
        .with_context(|| format!("read `{directory}`"))?
    {
        let entry = entry_result.with_context(|| format!("read an entry of `{directory}`"))?;
        let name = entry
            .file_name()
            .with_context(|| format!("read an entry name in `{directory}`"))?;
        if MACHINE_LOCAL_DIRECTORIES.contains(&name.as_str()) {
            continue;
        }
        let path = join_path(directory, &name);
        let file_type = entry
            .file_type()
            .with_context(|| format!("read the file type of `{path}`"))?;
        if file_type.is_dir() {
            collect_all_sources(root, &path, found)?;
        } else if is_rust_source(&name) {
            found.push(path);
        }
    }
    Ok(())
}

/// Join a directory and an entry name, keeping the walk root's paths bare.
pub(super) fn join_path(directory: &str, name: &str) -> String {
    match directory {
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
        collect_rust_sources(&crate_root, root_path, &mut sources)
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
