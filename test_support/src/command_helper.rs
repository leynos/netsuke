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

/// Source inputs for one helper compilation.
///
/// This private request keeps compiler selection inside this module while
/// avoiding a wider public API solely for test injection.
#[derive(Clone, Copy)]
struct RustHelperSource<'a> {
    name: &'a str,
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

fn compile_rust_helper_with_env(
    env: &impl Env,
    dir: &Dir,
    root: &Utf8PathBuf,
    helper: RustHelperSource<'_>,
) -> Result<Utf8PathBuf> {
    let RustHelperSource { name, source } = helper;
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
    //! Unit tests for compiler selection and helper compilation.

    use super::rust_compiler;
    #[cfg(unix)]
    use super::{RustHelperSource, compile_rust_helper_with_env};
    #[cfg(unix)]
    use crate::exec::write_exec_with_content;
    #[cfg(unix)]
    use anyhow::{Context, Result, ensure};
    #[cfg(unix)]
    use camino::Utf8PathBuf;
    #[cfg(unix)]
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use mockable::MockEnv;
    use std::ffi::OsString;
    #[cfg(unix)]
    use tempfile::tempdir;

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

    #[cfg(unix)]
    #[test]
    fn compile_helper_invokes_configured_absolute_wrapper() -> Result<()> {
        let temp = tempdir().context("create compiler wrapper directory")?;
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .map_err(|path| anyhow::anyhow!("compiler wrapper path is not UTF-8: {path:?}"))?;
        let dir = Dir::open_ambient_dir(&root, ambient_authority())
            .context("open compiler wrapper directory")?;
        let wrapper = write_exec_with_content(
            temp.path(),
            "configured-rustc",
            concat!(
                "#!/bin/sh\n",
                "set -eu\n",
                "output=\n",
                "while [ \"$#\" -gt 0 ]; do\n",
                "    if [ \"$1\" = \"-o\" ]; then\n",
                "        shift\n",
                "        output=$1\n",
                "        break\n",
                "    fi\n",
                "    shift\n",
                "done\n",
                "test -n \"$output\"\n",
                ": > \"$output\"\n",
                "printf invoked > \"$0.invoked\"\n",
            ),
        )
        .context("write configured compiler wrapper")?;
        // `write_exec_with_content` is a std::path boundary; convert once here so
        // the sidecar marker stays in camino's UTF-8 domain like the rest of the
        // helper.
        let wrapper_path = Utf8PathBuf::from_path_buf(wrapper)
            .map_err(|path| anyhow::anyhow!("compiler wrapper path is not UTF-8: {path:?}"))?;
        let marker = Utf8PathBuf::from(format!("{wrapper_path}.invoked"));
        let configured = OsString::from(wrapper_path.as_str());
        let mut env = MockEnv::new();
        env.expect_os_string()
            .withf(|key| key == "RUSTC")
            .once()
            .return_once(move |_| Some(configured));

        let executable = compile_rust_helper_with_env(
            &env,
            &dir,
            &root,
            RustHelperSource {
                name: "probe",
                source: "fn main() {}\n",
            },
        )?;

        ensure!(
            marker.exists(),
            "configured compiler wrapper was not invoked"
        );
        ensure!(
            executable.as_std_path().exists(),
            "configured compiler wrapper did not create the requested output"
        );
        Ok(())
    }
}
