//! Benchmark base-anchored manifest glob expansion.
//!
//! The fixture is created before timing begins. The two benches compare the
//! injected-base form used by manifest parsing with an equivalent absolute
//! pattern, retaining each result through [`test::black_box`].

#![feature(test)]

extern crate test;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use netsuke::manifest::glob_paths;
use tempfile::TempDir;
use test::{Bencher, black_box};
use test_support::fs as test_fs;

/// Number of directories in the deterministic benchmark tree.
const DIRECTORY_COUNT: usize = 128;
/// Number of text files in each benchmark directory.
const FILES_PER_DIRECTORY: usize = 32;

/// Retain the temporary tree and both pattern forms needed by the benchmarks.
struct GlobFixture {
    /// Keep the fixture alive for each timed iteration.
    _temporary_directory: TempDir,
    /// Canonical UTF-8 directory supplied to the manifest-oriented call.
    base: Utf8PathBuf,
    /// Equivalent absolute pattern used as an unbased comparison.
    absolute_pattern: String,
}

/// Build a deterministic nested fixture tree outside the timed loop.
fn benchmark_fixture() -> Result<GlobFixture> {
    let temporary_directory = tempfile::tempdir().context("create benchmark directory")?;
    let base_directory = temporary_directory.path().join("workspace");
    test_fs::create_dir(&base_directory).context("create benchmark workspace")?;
    for directory_index in 0..DIRECTORY_COUNT {
        let directory = base_directory.join(format!("directory-{directory_index:03}"));
        test_fs::create_dir(&directory)
            .with_context(|| format!("create {}", directory.display()))?;
        for file_index in 0..FILES_PER_DIRECTORY {
            let file = directory.join(format!("file-{file_index:03}.txt"));
            test_fs::write(&file, "fixture")
                .with_context(|| format!("write {}", file.display()))?;
        }
    }
    let base = Utf8Path::from_path(&base_directory)
        .context("benchmark paths must be UTF-8")?
        .to_path_buf();
    Ok(GlobFixture {
        absolute_pattern: base.join("**/*.txt").to_string(),
        _temporary_directory: temporary_directory,
        base,
    })
}

/// Benchmark the manifest-oriented relative pattern anchored to an injected base.
#[bench]
fn expands_relative_pattern_under_injected_base(bencher: &mut Bencher) {
    let fixture = benchmark_fixture().unwrap_or_else(|error| panic!("build fixture: {error}"));
    bencher.iter(|| {
        let paths = glob_paths(
            "**/*.txt",
            Some(Utf8Path::new(black_box(fixture.base.as_str()))),
        )
        .unwrap_or_else(|error| panic!("expand base-anchored glob: {error}"));
        black_box(paths);
    });
}

/// Benchmark the equivalent absolute pattern without an injected base.
#[bench]
fn expands_equivalent_absolute_pattern(bencher: &mut Bencher) {
    let fixture = benchmark_fixture().unwrap_or_else(|error| panic!("build fixture: {error}"));
    bencher.iter(|| {
        let paths = glob_paths(black_box(&fixture.absolute_pattern), None)
            .unwrap_or_else(|error| panic!("expand absolute glob: {error}"));
        black_box(paths);
    });
}
