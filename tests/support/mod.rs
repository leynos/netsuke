//! Test utilities for process management and log capture.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tracing::Level;
use tracing_subscriber::fmt;

/// Create a fake Ninja executable that exits with `exit_code`.
///
/// Returns the temporary directory and the path to the executable.
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

#[allow(dead_code, reason = "compiled as its own crate during linting")]
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
#[allow(dead_code, reason = "compiled as its own crate during linting")]
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
#[allow(dead_code, reason = "used only in directory tests")]
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
