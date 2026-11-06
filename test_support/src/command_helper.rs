//! Test-support helpers for compiling tiny Rust programmes used by command
//! filter tests.

use std::{ffi::OsString, process::Command, time::Duration};

use anyhow::{Context, Result, bail};

use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use mockable::{DefaultEnv, Env};

use crate::bounded_process::{COMPILE_DEADLINE, output_within};

/// A helper `main` that upper-cases stdin to stdout.
const UPPERCASE_SOURCE: &str = concat!(
    "use std::io::{self, Read};\n",
    "fn main() {\n",
    "    let mut input = String::new();\n",
    "    io::stdin().read_to_string(&mut input).expect(\"stdin\");\n",
    "    print!(\"{}\", input.to_uppercase());\n",
    "}\n",
);

/// A helper `main` that exits with status `1`.
const FAILURE_SOURCE: &str = concat!(
    "use std::io::{self, Read};\n",
    "fn main() {\n",
    "    let mut input = String::new();\n",
    "    let _ = io::stdin().read_to_string(&mut input);\n",
    "    std::process::exit(1);\n",
    "}\n",
);

/// A helper `main` that streams 128 KiB of `x` bytes to stdout.
const LARGE_OUTPUT_SOURCE: &str = concat!(
    "use std::io::{self, Write};\n",
    "fn main() {\n",
    "    let mut out = io::stdout();\n",
    "    let chunk = [b'x'; 1024];\n",
    "    for _ in 0..128 {\n",
    "        out.write_all(&chunk).expect(\"stdout\");\n",
    "    }\n",
    "}\n",
);

/// Source inputs for one helper compilation.
///
/// This private request keeps compiler selection inside this module while
/// avoiding a wider public API solely for test injection.
#[derive(Clone, Copy)]
struct RustHelperSource<'a> {
    /// Executable file name for the compiled helper.
    name: &'a str,
    /// Rust source to compile.
    source: &'a str,
}
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
/// let exe = compile_uppercase_helper(&dir, &root, "cmd_upper")
///     .expect("compile helper");
/// assert!(exe.as_std_path().exists());
/// ```
///
/// # Errors
///
/// Returns an error if the helper source cannot be written or compiled.
pub fn compile_uppercase_helper(dir: &Dir, root: &Utf8PathBuf, name: &str) -> Result<Utf8PathBuf> {
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
/// let exe = compile_failure_helper(&dir, &root, "cmd_fail")
///     .expect("compile helper");
/// assert!(exe.as_std_path().exists());
/// ```
///
/// # Errors
///
/// Returns an error if the helper source cannot be written or compiled.
pub fn compile_failure_helper(dir: &Dir, root: &Utf8PathBuf, name: &str) -> Result<Utf8PathBuf> {
    compile_rust_helper(dir, root, name, FAILURE_SOURCE)
}

/// Compile a helper that writes a sizeable payload to stdout.
///
/// # Errors
///
/// Returns an error if the helper source cannot be written or compiled.
pub fn compile_large_output_helper(
    dir: &Dir,
    root: &Utf8PathBuf,
    name: &str,
) -> Result<Utf8PathBuf> {
    compile_rust_helper(dir, root, name, LARGE_OUTPUT_SOURCE)
}

/// Compile an arbitrary Rust helper source to an executable.
///
/// Writes `{name}.rs` into `dir`, invokes the toolchain, and returns the
/// executable path, which remains valid whilst `dir`'s backing directory
/// exists.
///
/// The compiler is selected from `RUSTC` through [`mockable::DefaultEnv`]. If
/// `RUSTC` is absent, the conventional `rustc` command is used.
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
/// )
/// .expect("compile Rust helper");
/// assert!(exe.as_std_path().exists());
/// ```
///
/// # Errors
///
/// Returns an error if the source cannot be written or the selected compiler
/// cannot compile it.
pub fn compile_rust_helper(
    dir: &Dir,
    root: &Utf8PathBuf,
    name: &str,
    source: &str,
) -> Result<Utf8PathBuf> {
    compile_rust_helper_with_env(&DefaultEnv, dir, root, RustHelperSource { name, source })
}

/// Compile `helper` inside `dir` using `env`, reporting the executable path.
///
/// # Errors
///
/// Returns an error if the source cannot be written, starting or launching the
/// configured `RUSTC` compiler fails before an exit status is available, or
/// the compiler exits unsuccessfully.
fn compile_rust_helper_with_env(
    env: &impl Env,
    dir: &Dir,
    root: &Utf8PathBuf,
    helper: RustHelperSource<'_>,
) -> Result<Utf8PathBuf> {
    compile_rust_helper_within(
        env,
        dir,
        root,
        BoundedHelper {
            helper,
            deadline: COMPILE_DEADLINE,
        },
    )
}

/// A helper to compile and the longest the compiler may take.
#[derive(Clone, Copy)]
struct BoundedHelper<'a> {
    /// The helper source to compile.
    helper: RustHelperSource<'a>,
    /// How long the compiler may run before the compile fails.
    deadline: Duration,
}

/// Compile a [`BoundedHelper`] as [`compile_rust_helper_with_env`] does.
///
/// # Errors
///
/// As [`compile_rust_helper_with_env`], and when the compiler is still running
/// at the deadline.
fn compile_rust_helper_within(
    env: &impl Env,
    dir: &Dir,
    root: &Utf8PathBuf,
    bounded: BoundedHelper<'_>,
) -> Result<Utf8PathBuf> {
    let BoundedHelper { helper, deadline } = bounded;
    let RustHelperSource { name, source } = helper;
    dir.write(format!("{name}.rs"), source.as_bytes())
        .with_context(|| format!("write helper source {name}.rs"))?;

    let src_path = root.join(format!("{name}.rs"));
    let exe_path = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let rustc = rust_compiler(env);
    // nested-cargo-allow: timed the spawn is bounded by `output_within`'s deadline
    let mut compile = Command::new(&rustc);
    compile
        .arg(src_path.as_std_path())
        .arg("-o")
        .arg(exe_path.as_std_path());
    let output = output_within(&mut compile, deadline).with_context(|| {
        format!(
            "invoke {} to compile helper {name}",
            rustc.to_string_lossy()
        )
    })?;

    if !output.status.success() {
        bail!(
            "failed to compile helper {name}: {:?}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(exe_path)
}

/// Return the configured `RUSTC` path, defaulting to `rustc` on `PATH`.
fn rust_compiler(env: &impl Env) -> OsString {
    env.os_string("RUSTC")
        .unwrap_or_else(|| OsString::from("rustc"))
}

#[cfg(test)]
#[path = "command_helper_tests.rs"]
mod tests;
