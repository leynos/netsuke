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
/// Compile with a stand-in compiler written as `script`, bounded by `deadline`.
#[cfg(unix)]
fn compile_with_stand_in(script: &str, deadline: std::time::Duration) -> Result<Utf8PathBuf> {
    use super::{BoundedHelper, compile_rust_helper_within};

    let temp = tempdir().context("create compiler stand-in directory")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("stand-in path is not UTF-8: {path:?}"))?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open compiler stand-in directory")?;
    let stand_in = write_exec_with_content(temp.path(), "stand-in-rustc", script)
        .context("write compiler stand-in")?;
    let configured = OsString::from(stand_in.as_os_str());
    let mut env = MockEnv::new();
    env.expect_os_string()
        .withf(|key| key == "RUSTC")
        .once()
        .return_once(move |_| Some(configured));
    compile_rust_helper_within(
        &env,
        &dir,
        &root,
        BoundedHelper {
            helper: RustHelperSource {
                name: "probe",
                source: "fn main() {}\n",
            },
            deadline,
        },
    )
}

#[cfg(unix)]
#[test]
fn a_failing_compiler_reports_its_stderr() -> Result<()> {
    let error = compile_with_stand_in(
        "#!/bin/sh\necho 'boom: no such crate' >&2\nexit 1\n",
        std::time::Duration::from_secs(20),
    )
    .expect_err("a failing compiler must fail the helper");

    let message = format!("{error:#}");
    ensure!(
        message.contains("boom: no such crate"),
        "the compiler's stderr must reach the error, got: {message}"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_hanging_compiler_fails_at_the_deadline() -> Result<()> {
    let started = std::time::Instant::now();
    let error = compile_with_stand_in(
        "#!/bin/sh\nexec sleep 30\n",
        std::time::Duration::from_millis(400),
    )
    .expect_err("a hanging compiler must fail the helper");

    let message = format!("{error:#}");
    ensure!(
        message.contains("did not finish"),
        "the failure must name the deadline, got: {message}"
    );
    ensure!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "the hung compiler must be killed, not waited for"
    );
    Ok(())
}
