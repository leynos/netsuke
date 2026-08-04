//! Test-support helpers for compiling tiny Rust programmes used by command
//! filter tests.

use std::{ffi::OsString, process::Command};

use anyhow::{Context, Result, bail};

use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use mockable::{DefaultEnv, Env};

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
/// Returns an error if the source cannot be written or `rustc` cannot compile it.
pub fn compile_rust_helper(
    dir: &Dir,
    root: &Utf8PathBuf,
    name: &str,
    source: &str,
) -> Result<Utf8PathBuf> {
    compile_rust_helper_with_env(&DefaultEnv, dir, root, name, source)
}

/// Compile an arbitrary Rust helper using the compiler selected by an
/// injected environment.
///
/// `RUSTC` is honoured when present, including absolute paths that are not
/// discoverable through `PATH`. If it is absent, the conventional `rustc`
/// command is used.
///
/// # Examples
///
/// ```rust,no_run
/// # use camino::Utf8PathBuf;
/// # use cap_std::{ambient_authority, fs_utf8::Dir};
/// # use mockable::DefaultEnv;
/// # use tempfile::tempdir;
/// # use test_support::command_helper::compile_rust_helper_with_env;
/// let temp = tempdir().expect("tempdir");
/// let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
///     .expect("utf8 path");
/// let dir = Dir::open_ambient_dir(&root, ambient_authority())
///     .expect("open temp dir");
/// let exe = compile_rust_helper_with_env(
///     &DefaultEnv,
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
pub fn compile_rust_helper_with_env(
    env: &impl Env,
    dir: &Dir,
    root: &Utf8PathBuf,
    name: &str,
    source: &str,
) -> Result<Utf8PathBuf> {
    dir.write(format!("{name}.rs"), source.as_bytes())
        .with_context(|| format!("write helper source {name}.rs"))?;

    let src_path = root.join(format!("{name}.rs"));
    let exe_path = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let rustc = rust_compiler(env);
    let status = Command::new(&rustc)
        .arg(src_path.as_std_path())
        .arg("-o")
        .arg(exe_path.as_std_path())
        .status()
        .with_context(|| {
            format!(
                "invoke {} to compile helper {name}",
                rustc.to_string_lossy()
            )
        })?;

    if !status.success() {
        bail!("failed to compile helper {name}: {status:?}");
    }
    Ok(exe_path)
}

fn rust_compiler(env: &impl Env) -> OsString {
    env.os_string("RUSTC")
        .unwrap_or_else(|| OsString::from("rustc"))
}

#[cfg(test)]
mod tests {
    use super::rust_compiler;
    use mockable::MockEnv;
    use std::ffi::OsString;

    #[test]
    fn rust_compiler_honours_configured_path() {
        let configured = OsString::from("/toolchain/bin/rustc");
        let expected = configured.clone();
        let mut env = MockEnv::new();
        env.expect_os_string()
            .withf(|key| key == "RUSTC")
            .once()
            .return_once(move |_| Some(configured));

        assert_eq!(rust_compiler(&env), expected);
    }

    #[test]
    fn rust_compiler_falls_back_to_path_lookup() {
        let mut env = MockEnv::new();
        env.expect_os_string()
            .withf(|key| key == "RUSTC")
            .once()
            .return_once(|_| None);

        assert_eq!(rust_compiler(&env), OsString::from("rustc"));
    }
}
