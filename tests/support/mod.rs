//! Test utilities for process management.
//!
//! This module provides helpers for creating fake executables and
//! generating minimal manifests used in behavioural tests.

use mockable::MockEnv;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tracing::Level;
use tracing_subscriber::fmt;

/// Create a fake Ninja executable that exits with `exit_code`.
///
/// Returns the temporary directory and the path to the executable.
#[allow(
    unfulfilled_lint_expectations,
    reason = "used only in some test crates"
)]
#[expect(dead_code, reason = "used in CLI behaviour tests")]
pub fn fake_ninja(exit_code: i32) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).expect("script");
    writeln!(file, "#!/bin/sh\nexit {exit_code}").expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("perms");
    }
    (dir, path)
}

/// Set up `env` so `PATH` resolves only to `dir`.
///
/// # Examples
/// ```ignore
/// let (dir, _) = fake_ninja(0);
/// let mut env = MockEnv::new();
/// mock_path_to(&mut env, dir.path());
/// ```
#[allow(
    unfulfilled_lint_expectations,
    reason = "used only in some test crates",
)]
#[expect(dead_code, reason = "used in PATH tests")]
pub fn mock_path_to(env: &mut MockEnv, dir: &Path) {
    let path = dir.to_string_lossy().into_owned();
    env.expect_raw()
        .withf(|key| key == "PATH")
        .returning(move |_| Ok(path.clone()));
}

/// Create a fake Ninja that validates the build file path provided via `-f`.
///
/// The script exits with status `1` if the file is missing or not a regular
/// file, otherwise `0`.
#[allow(
    unfulfilled_lint_expectations,
    reason = "used only in some test crates"
)]
#[expect(dead_code, reason = "used in build file validation tests")]
pub fn fake_ninja_check_build_file() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).expect("script");
    writeln!(
        file,
        concat!(
            "#!/bin/sh\n",
            "if [ \"$1\" = \"-f\" ] && [ ! -f \"$2\" ]; then\n",
            "  echo 'missing build file: $2' >&2\n",
            "  exit 1\n",
            "fi\n",
            "exit 0"
        )
    )
    .expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("perms");
    }
    (dir, path)
}

#[allow(
    unfulfilled_lint_expectations,
    reason = "compiled only for logging tests"
)]
#[expect(dead_code, reason = "compiled as its own crate during linting")]
#[derive(Clone)]
struct BufferWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl Write for BufferWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.lock().expect("lock").write(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf.lock().expect("lock").flush()
    }
}

/// Capture logs emitted within the provided closure.
///
/// # Examples
///
/// ```ignore
/// use tracing::Level;
/// let output = capture_logs(Level::INFO, || tracing::info!("hello"));
/// assert!(output.contains("hello"));
/// ```
#[allow(
    unfulfilled_lint_expectations,
    reason = "compiled only for logging tests"
)]
#[expect(dead_code, reason = "compiled as its own crate during linting")]
pub fn capture_logs<F>(level: Level, f: F) -> String
where
    F: FnOnce(),
{
    let buf = Arc::new(Mutex::new(Vec::new()));
    let writer = BufferWriter {
        buf: Arc::clone(&buf),
    };
    let subscriber = fmt()
        .with_max_level(level)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, f);
    let locked = buf.lock().expect("lock");
    String::from_utf8(locked.clone()).expect("utf8")
}

/// Create a fake Ninja executable that writes its current directory to the file
/// specified as the first argument.
///
/// Returns the temporary directory and the path to the executable.
#[allow(unfulfilled_lint_expectations, reason = "used only in directory tests")]
#[expect(dead_code, reason = "used only in directory tests")]
pub fn fake_ninja_pwd() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).expect("script");
    // The script writes its working directory to the provided file and exits.
    writeln!(file, "#!/bin/sh\npwd > \"$1\"").expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("perms");
    }
    (dir, path)
}

/// Write a minimal manifest to `file`.
///
/// The manifest declares a single `hello` target that prints a greeting.
/// This must be `allow` as `expect` will trigger an unfulfilled warning
/// despite the lint violation arising.
#[allow(
    unfulfilled_lint_expectations,
    reason = "shared test utility not used in all crates"
)]
#[expect(dead_code, reason = "shared test utility not used in all crates")]
pub fn write_manifest(file: &mut impl Write) -> io::Result<()> {
    writeln!(
        file,
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: hello\n",
            "    recipe:\n",
            "      kind: command\n",
            "      command: \"echo hi\"\n"
        ),
    )
}
