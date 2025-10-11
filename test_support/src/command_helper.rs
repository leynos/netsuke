//! Test-support helpers for compiling tiny Rust programmes used by command
//! filter tests.

use std::{ffi::OsString, process::Command};

use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;

const UPPERCASE_SOURCE: &str = concat!(
    "use std::io::{self, Read};\n",
    "fn main() {\n",
    "    let mut input = String::new();\n",
    "    io::stdin().read_to_string(&mut input).expect(\"stdin\");\n",
    "    print!(\"{}\", input.to_uppercase());\n",
    "}\n",
);

const FAILURE_SOURCE: &str = concat!(
    "use std::io::{self, Read};\n",
    "fn main() {\n",
    "    let mut input = String::new();\n",
    "    let _ = io::stdin().read_to_string(&mut input);\n",
    "    std::process::exit(1);\n",
    "}\n",
);

/// Compile a helper binary that converts stdin to upper case and return the
/// executable path.
///
/// # Examples
///
/// ```rust,no_run
/// use camino::Utf8PathBuf;
/// use cap_std::{ambient_authority, fs_utf8::Dir};
/// use tempfile::tempdir;
/// use test_support::command_helper::compile_uppercase_helper;
///
/// let temp = tempdir().expect("tempdir");
/// let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
///     .expect("utf8 path");
/// let dir = Dir::open_ambient_dir(&root, ambient_authority())
///     .expect("open temp dir");
/// let exe = compile_uppercase_helper(&dir, &root, "cmd_upper");
/// assert!(exe.as_std_path().exists());
/// ```
pub fn compile_uppercase_helper(dir: &Dir, root: &Utf8PathBuf, name: &str) -> Utf8PathBuf {
    compile_rust_helper(dir, root, name, UPPERCASE_SOURCE)
}

/// Compile a helper binary that exits with status code `1` after consuming
/// stdin.
///
/// # Examples
///
/// ```rust,no_run
/// # use camino::Utf8PathBuf;
/// # use cap_std::{ambient_authority, fs_utf8::Dir};
/// # use tempfile::tempdir;
/// # use test_support::command_helper::compile_failure_helper;
/// let temp = tempdir().expect("tempdir");
/// let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
///     .expect("utf8 path");
/// let dir = Dir::open_ambient_dir(&root, ambient_authority())
///     .expect("open temp dir");
/// let exe = compile_failure_helper(&dir, &root, "cmd_fail");
/// assert!(exe.as_std_path().exists());
/// ```
pub fn compile_failure_helper(dir: &Dir, root: &Utf8PathBuf, name: &str) -> Utf8PathBuf {
    compile_rust_helper(dir, root, name, FAILURE_SOURCE)
}

/// Compile an arbitrary Rust helper source to an executable.
///
/// Writes `{name}.rs` into `dir`, invokes the toolchain, and returns the
/// executable path, which remains valid whilst `dir`'s backing directory
/// exists.
///
/// # Examples
///
/// ```rust,no_run
/// # use camino::Utf8PathBuf;
/// # use cap_std::{ambient_authority, fs_utf8::Dir};
/// # use tempfile::tempdir;
/// # use test_support::command_helper::compile_rust_helper;
/// let temp = tempdir().expect("tempdir");
/// let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
///     .expect("utf8 path");
/// let dir = Dir::open_ambient_dir(&root, ambient_authority())
///     .expect("open temp dir");
/// let exe = compile_rust_helper(
///     &dir,
///     &root,
///     "cmd",
///     "fn main() {}\n",
/// );
/// assert!(exe.as_std_path().exists());
/// ```
pub fn compile_rust_helper(dir: &Dir, root: &Utf8PathBuf, name: &str, source: &str) -> Utf8PathBuf {
    dir.write(&format!("{name}.rs"), source.as_bytes())
        .expect("write helper source");

    let src_path = root.join(format!("{name}.rs"));
    let exe_path = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let status = Command::new(&rustc)
        .arg(src_path.as_std_path())
        .arg("-o")
        .arg(exe_path.as_std_path())
        .status()
        .expect("compile helper");

    assert!(status.success(), "failed to compile helper: {status:?}");
    exe_path
}
