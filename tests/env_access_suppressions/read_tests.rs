//! Self-tests for which sources the scan reads.
//!
//! The contract's scan and the coverage invariant each walk the workspace, and
//! they have to agree about what a source is. They do not agree by
//! construction — they are two functions with two filters — so the agreement
//! is pinned here, against a synthetic tree rather than against the repository:
//! a shape the repository does not happen to contain today is exactly the shape
//! that would go unnoticed if the test read the real tree.
//!
//! The walk's own reach is pinned in `walk_tests.rs`; this module is about the
//! read set the reach is used for.

use super::roots::collect_rust_sources;
use super::scanner::scan_source;
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::tempdir;

/// Fail if the scan reads only the files a `.rs` name points at.
///
/// The compiler reaches a module through whatever `#[path = "..."]` names, with
/// no extension test of its own: `#[path = "suppressed.inc"] mod suppressed;`
/// compiles, and the module may open with
/// `#![allow(clippy::disallowed_methods)]`, which `clippy::allow_attributes`
/// does not report because it does not fire on the inner form. An extension
/// filter therefore names the compiled sources by a spelling the language does
/// not require, and the shape is invisible to every other test here: the file
/// is under a scanned root, so [`is_scanned`] would say it is governed, and it
/// is simply never asked because the walk filtered it out first.
///
/// The tree is synthetic for the usual reason — the repository holds no such
/// file today, which is exactly the shape that would go unnoticed. The binary
/// is here too, because the wider read set is what admits it and a walk that
/// failed on it would report an I/O error rather than a suppression.
#[test]
fn a_source_without_an_rs_extension_is_still_read() -> Result<()> {
    let scratch = tempdir().context("create a scratch directory for the walk")?;
    let scratch_path = Utf8Path::from_path(scratch.path())
        .context("a temporary directory path should be valid UTF-8")?;
    let root = Dir::open_ambient_dir(scratch_path, ambient_authority())
        .context("open the scratch directory")?;
    let directory = "src";
    root.create_dir_all(directory)
        .with_context(|| format!("create `{directory}`"))?;
    root.write(
        "src/suppressed.inc",
        b"#![allow(clippy::disallowed_methods)]\n",
    )
    .context("write the module reached by `#[path]`")?;
    root.write("src/kept.rs", b"fn kept() {}\n")
        .context("write the kept source")?;
    // Not UTF-8, so not a module: the walk must pass over it rather than fail
    // on a read it cannot decode.
    root.write("src/blob.bin", b"\xff\xfe\x00\x01")
        .context("write the binary")?;
    // A dot-file is tooling state rather than a module, and stays out.
    root.write("src/.hidden", b"#![allow(clippy::disallowed_methods)]\n")
        .context("write the dot-file")?;

    let mut read = Vec::new();
    collect_rust_sources(&root, "src", &mut read)?;
    let mut names: Vec<&str> = read.iter().map(|(path, _)| path.as_str()).collect();
    names.sort_unstable();

    ensure!(
        names == ["src/kept.rs", "src/suppressed.inc"],
        "the scan must read a source the compiler reaches through `#[path]`, \
         whatever the file is named, and must pass over a file that is not text \
         and a name a `#[path]` cannot be written as; got {names:?}"
    );
    // Reading it is only half of it: the file the compiler compiles must be one
    // the scan reports on, which is what turns the read into a finding.
    let findings: Vec<String> = read
        .iter()
        .flat_map(|(path, contents)| scan_source(path, contents))
        .map(|(path, lint)| format!("{path}: {lint}"))
        .collect();
    ensure!(
        findings == ["src/suppressed.inc: clippy::disallowed_methods"],
        "the module the compiler reaches through `#[path]` carries the policy \
         suppression, so the scan must report it; got {findings:?}"
    );
    Ok(())
}

/// Fail if the walk treats a source it could not read as one that is absent.
///
/// The walk's wider read set is what makes the distinction matter: it now reads
/// files whose being Rust this walk cannot see, so a file that will not decode
/// has to be declined rather than fatal. But declining has to stay the narrow
/// case. A file that *is* text and could not be read is a source the gate did
/// not scan, which is the silent non-coverage the contract exists to prevent,
/// and calling that "nothing to see" is that same silence wearing an
/// error-handling hat.
///
/// The two are indistinguishable from the walk the suite performs, since every
/// path it touches is readable — so a filter that swallowed *every* error would
/// leave the whole suite green, measured exactly that way. Asserting the
/// predicate's own output is not enough to close that: it pins which kinds are
/// named, but the filter that consults it could still be a catch-all and the
/// suite would not notice, which was measured by replacing the guard with
/// `Err(_) => Ok(None)` and watching all 57 tests pass. So the read is taken
/// through [`read_source`] itself, and the kind that must not be swallowed is
/// produced by a path that really reports it.
///
/// A directory is that path, and it is the one shape here that behaves the same
/// way on every machine: the read fails with `IsADirectory`, which no filter may
/// classify as "not text". A permissions-based version was written and rejected
/// — it fails spuriously when the suite runs as root, which is how a container
/// lane runs it, and the mode bits it relies on are Unix-only. A missing file
/// and a file that will not decode are read here too, so that the pair is
/// asserted at the same level rather than one of them by proxy.
#[test]
fn only_a_file_that_is_not_text_may_be_passed_over() -> Result<()> {
    let scratch = tempdir().context("create a scratch directory for the walk")?;
    let scratch_path =
        Utf8Path::from_path(scratch.path()).context("a temporary path should be valid UTF-8")?;
    let root = Dir::open_ambient_dir(scratch_path, ambient_authority())
        .context("open the scratch directory")?;
    root.create_dir_all("sub")
        .context("create a directory to read as a file")?;
    // Not UTF-8, so there is no source text here and the walk may pass it over.
    root.write("blob.bin", b"\xff\xfe\x00\x01")
        .context("write a file that will not decode")?;
    for absent in ["missing.file", "blob.bin"] {
        ensure!(
            super::roots::read_source(&root, absent)
                .with_context(|| format!("read `{absent}`"))?
                .is_none(),
            "`{absent}` holds no source text, so the walk may pass it over"
        );
    }
    // The load-bearing half: the file is there and could not be read as text,
    // so it is a source this gate did not scan and must fail rather than be
    // reported as absent. A catch-all filter passes every assertion above and
    // fails this one.
    let error = super::roots::read_source(&root, "sub")
        .expect_err("a directory is not text, so reading it must be an error");
    ensure!(
        !super::roots::is_not_text(
            error
                .downcast_ref::<std::io::Error>()
                .map_or(std::io::ErrorKind::Other, std::io::Error::kind)
        ),
        "a directory that could not be read is a source that went unscanned, so \
         the walk must report it rather than let it go silently unread; got {error:?}"
    );
    Ok(())
}
