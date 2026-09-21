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

/// Target-directory slug for the LLVM baseline variant.
pub const DEFAULT_SLUG: &str = "default";
/// Target-directory slug for the repository default: the `mold` linker.
pub const MOLD_SLUG: &str = "mold";
/// Target-directory slug for the default plus the parallel `rustc` frontend.
pub const MOLD_THREADS_SLUG: &str = "mold-threads";
/// Every variant slug the benchmark can measure.
///
/// Unordered, and deliberately so: the benchmark shuffles the variants afresh
/// for each sample, because separate target directories isolate build artefacts
/// but not shared host state. Position here means nothing.
pub const BENCH_SLUGS: [&str; 3] = [DEFAULT_SLUG, MOLD_SLUG, MOLD_THREADS_SLUG];

/// How many times the benchmark measures each variant, as `bench-build.sh`
/// defaults it.
///
/// Mirrored here so the tests can assert on the default run's shape without
/// restating the number in two places. Every test asserts *equality* against
/// this value — the invocation count and the reported row count are both exact
/// multiples of it — and none overrides the environment variable the script
/// reads. A test that did override it would have to compute its own expected
/// count, because the shared assertions here would no longer describe the run
/// it had asked for.
pub const BENCH_REPEATS: usize = 2;

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
        for slug in BENCH_SLUGS {
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
