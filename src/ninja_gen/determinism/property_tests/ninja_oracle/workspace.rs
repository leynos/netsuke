//! Writing one emitted bundle to a fresh directory and asking `ninja` about it.
//!
//! The directory is fresh per case, and that is a correctness requirement
//! rather than tidiness. Sidecar names are content-addressed, so a directory
//! shared across cases would let a sidecar written for one case satisfy another
//! case's reference to the same digest. A missing-sidecar defect — the exact
//! failure `-n` exists to catch — would then pass, because some earlier case
//! had already written the file. See the parent module's documentation.
//!
//! Everything else here exists to make the two invocations meaningful:
//! `.netsuke/dyndep/` must exist because Ninja opens each `dyndep =` file by
//! path, and every declared output and input is written as an empty file
//! because `-n` rejects an input no rule produces. `.netsuke/serial/` is
//! created too, though nothing reads it: gate paths are only ever *named*, and
//! creating the directory keeps the on-disk shape identical to a real
//! `netsuke` run's.

use crate::ninja_gen::GeneratedNinja;
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::test_runner::TestCaseError;
use std::process::Command;
use tempfile::TempDir;

/// Directory the emitter's sidecars are addressed under.
const DYNDEP_DIR: &str = ".netsuke/dyndep";
/// Directory serial gates are addressed under.
const SERIAL_DIR: &str = ".netsuke/serial";
/// The generated build file's name.
const BUILD_FILE: &str = "build.ninja";

/// One case's bundle, written to disk and answerable by a real `ninja` binary.
pub(super) struct BundleWorkspace {
    /// The directory holding the bundle. Dropping it removes the directory.
    directory: TempDir,
    /// The targets the two invocations are asked about.
    ///
    /// Held here rather than passed to each call so that the "must not be
    /// empty" precondition is checked once, at construction: `ninja -t
    /// commands` with no target names the defaults, and an empty target list
    /// would therefore silently compare two default builds rather than the
    /// outputs the graph declares.
    targets: Vec<Utf8PathBuf>,
}

impl BundleWorkspace {
    /// Write `bundle` and every path it names into a fresh temporary directory.
    ///
    /// # Errors
    ///
    /// Returns a test failure if the directory cannot be created, a path
    /// escapes the directory, a file cannot be written, or `targets` is empty.
    pub(super) fn write(
        bundle: &GeneratedNinja,
        paths: &[Utf8PathBuf],
        targets: &[Utf8PathBuf],
    ) -> Result<Self, TestCaseError> {
        let directory =
            TempDir::new().map_err(|error| TestCaseError::fail(format!("temp dir: {error}")))?;
        if targets.is_empty() {
            return Err(TestCaseError::fail(
                "a generated graph must name at least one explicit output, or \
                 the command-list comparison would fall back to Ninja's defaults",
            ));
        }
        let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
            .map_err(|path| TestCaseError::fail(format!("non-UTF-8 temporary path: {path:?}")))?;
        let writer = Writer { root: &root };
        writer.create_directory(DYNDEP_DIR)?;
        writer.create_directory(SERIAL_DIR)?;
        for path in paths {
            writer.write_empty(path)?;
        }
        writer.write_bytes(Utf8Path::new(BUILD_FILE), bundle.build_file().as_bytes())?;
        for sidecar in bundle.dyndep_files() {
            let relative = sidecar.relative_path();
            if escapes(relative) {
                return Err(TestCaseError::fail(format!(
                    "emitted sidecar path {relative} escapes the bundle"
                )));
            }
            writer.write_bytes(relative.as_path(), sidecar.content().as_bytes())?;
        }
        Ok(Self {
            directory,
            targets: targets.to_vec(),
        })
    }

    /// Require both invocations to succeed.
    ///
    /// `-t commands` catches a bundle Ninja cannot resolve, and `-n` catches a
    /// bundle whose dyndep sidecars it cannot load; neither subsumes the other,
    /// which is why both run.
    ///
    /// # Errors
    ///
    /// Returns a test failure carrying Ninja's stderr for whichever invocation
    /// rejected the bundle.
    pub(super) fn require_accepts(&self) -> Result<(), TestCaseError> {
        self.check("-n", &[])?;
        self.check("-t", &["commands"])?;
        Ok(())
    }

    /// Ask Ninja for the command list, requiring the invocation to succeed.
    ///
    /// Returns the lines Ninja printed, trimmed and with blank lines removed so
    /// that a trailing newline does not make two equal lists compare unequal.
    ///
    /// # Errors
    ///
    /// Returns a test failure when `ninja` cannot be spawned, exits non-zero, or
    /// prints non-UTF-8 output.
    pub(super) fn command_list(&self) -> Result<Vec<String>, TestCaseError> {
        let output = self.check("-t", &["commands"])?;
        let text = String::from_utf8(output)
            .map_err(|error| TestCaseError::fail(format!("ninja -t commands output: {error}")))?;
        Ok(text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// Run one invocation and return its standard output.
    fn check(&self, flag: &str, tool: &[&str]) -> Result<Vec<u8>, TestCaseError> {
        let output = Command::new("ninja")
            .current_dir(self.directory.path())
            .arg(flag)
            .args(tool)
            .args(&self.targets)
            .output()
            .map_err(|error| TestCaseError::fail(format!("spawn ninja {flag}: {error}")))?;
        if output.status.success() {
            return Ok(output.stdout);
        }
        // The status is reported as well as the stderr because a signal death
        // and a clean rejection are different findings with the same empty
        // stderr, and `ExitStatus`'s debug form names the signal.
        Err(TestCaseError::fail(format!(
            "ninja {flag} {tool:?} rejected the bundle ({:?}): {stderr}",
            output.status,
            stderr = String::from_utf8_lossy(&output.stderr).trim(),
        )))
    }
}

/// A capability over one bundle directory.
struct Writer<'a> {
    /// The bundle directory.
    root: &'a Utf8Path,
}

impl Writer<'_> {
    /// Create one directory inside the bundle.
    fn create_directory(&self, relative: &str) -> Result<(), TestCaseError> {
        self.open()?
            .create_dir_all(relative)
            .map_err(|error| TestCaseError::fail(format!("create {relative}: {error}")))
    }

    /// Write `path` as an empty file, creating its parent directory.
    fn write_empty(&self, path: &Utf8Path) -> Result<(), TestCaseError> {
        self.write_bytes(path, &[])
    }

    /// Write `contents` to `path`, creating its parent directory.
    fn write_bytes(&self, path: &Utf8Path, contents: &[u8]) -> Result<(), TestCaseError> {
        if escapes(path) {
            return Err(TestCaseError::fail(format!(
                "bundle path {path} escapes the bundle directory"
            )));
        }
        let directory = self.open()?;
        if let Some(parent) = parent_directory(path) {
            directory
                .create_dir_all(parent)
                .map_err(|error| create_failure(parent, &error))?;
        }
        directory
            .write(path, contents)
            .map_err(|error| TestCaseError::fail(format!("write {path}: {error}")))
    }

    /// Open the bundle directory with a capability.
    fn open(&self) -> Result<Dir, TestCaseError> {
        Dir::open_ambient_dir(self.root, ambient_authority())
            .map_err(|error| TestCaseError::fail(format!("open bundle dir: {error}")))
    }
}

/// Whether `path` names something outside its directory.
///
/// An absolute path or a `..` component would make every write below land
/// outside the temporary directory. The emitter produces neither, so seeing one
/// is a test failure rather than something to skip.
fn escapes(path: &Utf8Path) -> bool {
    path.is_absolute()
        || path
            .components()
            .any(|component| component == Utf8Component::ParentDir)
}

/// The directory to create before writing `path`, when it is worth creating.
///
/// A bare filename has a parent of `""`, which is the working directory rather
/// than a directory to create; `create_dir_all("")` would report an error for a
/// write the caller is entitled to make. Both conditions together decide it, so
/// the write site stays a single `if let` and never reaches the nesting depth
/// that reverses them.
fn parent_directory(path: &Utf8Path) -> Option<&Utf8Path> {
    path.parent().filter(|parent| !parent.as_str().is_empty())
}

/// The failure for a parent directory that could not be created.
///
/// Kept apart from the call site because formatting the path and its `io::Error`
/// inline lands inside both the `if let` and the closure, which is one level
/// deeper than the nesting ceiling allows.
fn create_failure(parent: &Utf8Path, error: &std::io::Error) -> TestCaseError {
    TestCaseError::fail(format!("create {parent} in the bundle: {error}"))
}
