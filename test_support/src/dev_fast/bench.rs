//! Disposable inputs for a `make bench-build` run.
//!
//! The benchmark mutates two things a test must not let it reach: the variant
//! target directories and the file it touches to make the second pass
//! incremental. Both are redirected into the sandbox here, so a benchmark test
//! never disturbs the working tree's build cache or a tracked source.

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use super::{BuildScenario, Sandbox};

/// Timestamp stamped on the benchmark's touch file before the run, chosen far
/// enough in the past that any `touch` is unambiguously newer. Comparing
/// against a fixed baseline keeps the assertion deterministic, where comparing
/// the two passes to each other would depend on filesystem timestamp
/// granularity.
pub const BASELINE_MTIME: i64 = 1_600_000_000;

/// Target-directory slugs the benchmark uses, one per variant.
pub const DEFAULT_SLUG: &str = "default";
/// Stable benchmark scenario identifier used in generated paths and commands.
pub const DEV_FAST_SLUG: &str = "dev-fast";

/// Create the touch file with [`BASELINE_MTIME`], returning that timestamp.
///
/// # Errors
///
/// Returns an error if the fixture cannot be written or its timestamp cannot be changed.
pub fn write_with_old_mtime(sandbox: &Sandbox, path: &Utf8Path) -> Result<i64> {
    let baseline = UNIX_EPOCH + Duration::from_secs(BASELINE_MTIME.unsigned_abs());
    sandbox.write_file_with_mtime(path, "", baseline)?;
    Ok(BASELINE_MTIME)
}

/// The disposable inputs one benchmark run needs.
///
/// Named fields rather than a returned tuple: `root` and `touch_file` are both
/// paths, so positional returns could be transposed at the call site without
/// the compiler noticing.
pub struct BenchFixture {
    /// Benchmark root; each variant gets its own target directory beneath it.
    pub root: Utf8PathBuf,
    /// The file the benchmark touches between a variant's two passes.
    pub touch_file: Utf8PathBuf,
    /// The touch file's timestamp before the run, for ordering assertions.
    pub baseline_mtime: i64,
}

impl BenchFixture {
    /// Stage a benchmark run's disposable inputs.
    ///
    /// The touch file stands in for `src/main.rs`, so a run does not invalidate
    /// the working tree's build cache. Both target directories are seeded to
    /// model a re-run: on a fresh sandbox the benchmark's `rm -rf` would be
    /// indistinguishable from doing nothing, and the clean-pass assertion would
    /// hold vacuously.
    ///
    /// # Errors
    ///
    /// Returns an error if the benchmark fixture cannot be prepared.
    pub fn prepare(scenario: &BuildScenario) -> Result<Self> {
        let sandbox = scenario.sandbox();
        let touch_file = sandbox.home().join("bench-touch");
        let baseline_mtime = write_with_old_mtime(sandbox, &touch_file)?;

        let root = sandbox.home().join("bench");
        for slug in [DEFAULT_SLUG, DEV_FAST_SLUG] {
            sandbox.create_dir(&root.join(slug))?;
        }
        Ok(Self {
            root,
            touch_file,
            baseline_mtime,
        })
    }

    /// The lock directory the benchmark takes for the duration of a run.
    ///
    /// Derived from the root the same way the script derives it, so a test
    /// asserting on contention holds the same path the script will try to
    /// create rather than a guess that could drift from it.
    #[must_use]
    pub fn lock_dir(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(format!("{}.lock", self.root))
    }
}
