//! Contracts for the repository-owned Cargo linker selection.
//!
//! `.cargo/config.toml` names a wrapper in its Linux `linker` key. That key is
//! the one setting an exported `RUSTFLAGS` does not displace, so the wrapper is
//! what keeps mold selected on a worker whose caller exports its own flags,
//! such as the mutation worker.
//!
//! These tests hold the two halves the mechanism depends on: the wrapper the
//! configuration names exists and is executable, and the wrapper chooses mold
//! by default while the explicit `NETSUKE_PLATFORM_LINKER=1` opt-out selects
//! the platform linker. The wrapper is driven against a fake `cc` placed first
//! on `PATH`, so the tests assert the command it would run without needing
//! mold, a C toolchain, or a Cargo build.
#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use test_support::build_tools::cargo_config;
use test_support::fs;

/// The arguments the wrapper may add to select mold.
const MOLD_ARGUMENT: &str = "-fuse-ld=mold";

/// The variable that opts the wrapper out of mold selection.
const PLATFORM_LINKER_ENV: &str = "NETSUKE_PLATFORM_LINKER";

/// The `cross` configuration the packaged Linux builds read.
const CROSS_CONFIG_PATH: &str = "Cross.toml";

/// Arguments the test lets the wrapper forward unchanged.
///
/// `-fuse-ld=lld` is the one `rustc` itself adds for this target. Keeping it
/// here makes the ordering assertion test the real conflict rather than a
/// stand-in.
const FORWARDED_ARGUMENTS: [&str; 4] = ["--probe-object", "-o", "/tmp/probe", "-fuse-ld=lld"];

/// The linker value the committed configuration declares for Linux.
fn configured_linker() -> Result<String> {
    let config: toml::Value = toml::from_str(&cargo_config()?)?;
    config
        .get("target")
        .and_then(|table| table.get(r#"cfg(target_os = "linux")"#))
        .and_then(|table| table.get("linker"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .context("the Linux table should name a linker")
}

/// The wrapper's path, resolved from the repository root.
fn wrapper_path() -> Result<std::path::PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join(configured_linker()?))
}

/// Quote `path` for a single-quoted shell word.
fn shell_quote(path: &Path) -> String {
    let text = path.to_string_lossy();
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Install a fake `cc` that records its arguments to `log`.
fn install_fake_cc(bin: &Path, log: &Path) -> Result<()> {
    let script = format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\n", shell_quote(log));
    let cc = bin.join("cc");
    fs::write(&cc, script).with_context(|| format!("write {}", cc.display()))?;
    fs::set_mode(&cc, 0o755).with_context(|| format!("make {} executable", cc.display()))?;
    Ok(())
}

/// Run the wrapper with `marker` set and return the arguments the fake `cc`
/// received.
fn recorded_cc_arguments(marker: Option<&str>) -> Result<Vec<String>> {
    let temp = TempDir::new().context("create the linker probe directory")?;
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).context("create the fake C bin directory")?;
    let log = temp.path().join("cc-arguments.log");
    install_fake_cc(&bin, &log)?;

    let wrapper = wrapper_path()?;
    let mut command = Command::new(&wrapper);
    command
        .args(FORWARDED_ARGUMENTS)
        // The child is isolated on purpose: the wrapper must find `cc` by the
        // `PATH` this test supplies, and the caller's environment must not
        // decide the answer.
        .env_clear()
        .env("PATH", &bin);
    if let Some(value) = marker {
        command.env(PLATFORM_LINKER_ENV, value);
    }
    let output = command
        .output()
        .with_context(|| format!("run {}", wrapper.display()))?;
    ensure!(
        output.status.success(),
        "the linker wrapper should succeed, got `{}`",
        String::from_utf8_lossy(&output.stderr)
    );

    let recorded = fs::read_to_string(&log).context("read the fake cc arguments")?;
    Ok(recorded.lines().map(str::to_owned).collect())
}

/// The configuration names a wrapper that exists and can be executed.
///
/// A `linker` value that names a missing file, or a file without the execute
/// bit, reports at the first link as an opaque "not found" from rustc. This
/// test moves that failure to the configuration change that introduces it.
#[test]
fn the_configuration_names_an_executable_linker_wrapper() -> Result<()> {
    let configured = configured_linker()?;
    let wrapper = wrapper_path()?;
    ensure!(
        wrapper.is_file(),
        "the configured linker `{configured}` should be a file, resolved to {}",
        wrapper.display()
    );
    ensure!(
        fs::is_executable_file(&wrapper),
        "the configured linker `{configured}` should be executable, so a link \
         can run it"
    );
    Ok(())
}

/// The wrapper selects the linker from the marker, forwards everything, and
/// places its mold flag last.
///
/// The position is load-bearing: `rustc` forwards its own `-fuse-ld=lld`, and
/// a compiler driver honours the last `-fuse-ld` it receives. A prepended mold
/// flag would therefore be overridden, and the link would use lld while the
/// build still reported success.
#[rstest]
#[case::default_selects_mold(None, true)]
#[case::opt_out_selects_platform(Some("1"), false)]
#[case::other_value_keeps_the_standard(Some("0"), true)]
fn the_wrapper_selects_the_linker_from_the_marker(
    #[case] marker: Option<&str>,
    #[case] expects_mold: bool,
) -> Result<()> {
    let recorded = recorded_cc_arguments(marker)?;
    // The exact vector is the contract, not a set of memberships: it holds the
    // forwarding order, the absence of duplicates, the trailing position of
    // the mold flag that overrides rustc's own `-fuse-ld=lld`, and the
    // opt-out's promise to add no argument at all.
    let mut expected: Vec<&str> = FORWARDED_ARGUMENTS.to_vec();
    if expects_mold {
        expected.push(MOLD_ARGUMENT);
    }
    ensure!(
        recorded
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied()),
        "with {PLATFORM_LINKER_ENV}={marker:?}, the wrapper should pass exactly \
         {expected:?} to cc, got {recorded:?}"
    );
    Ok(())
}

/// The cross container receives the linker opt-out.
///
/// `cross` forwards only `CARGO_*`, `CROSS_*`, and a short allowlist by
/// default. The packaging lane sets the opt-out in the job environment, so
/// without this passthrough the variable never reaches the container and the
/// linker wrapper would demand a mold the image does not install.
#[test]
fn the_cross_configuration_passes_the_linker_opt_out_through() -> Result<()> {
    let contents = fs::read_to_string(CROSS_CONFIG_PATH)
        .with_context(|| format!("read {CROSS_CONFIG_PATH}"))?;
    let config: toml::Value = toml::from_str(&contents)?;
    let passthrough = config
        .get("build")
        .and_then(|table| table.get("env"))
        .and_then(|table| table.get("passthrough"))
        .and_then(toml::Value::as_array)
        .context("Cross.toml should declare build.env.passthrough")?;
    ensure!(
        passthrough
            .iter()
            .filter_map(toml::Value::as_str)
            .any(|name| name == PLATFORM_LINKER_ENV),
        "Cross.toml should pass {PLATFORM_LINKER_ENV} into the build \
         container, got {passthrough:?}"
    );
    Ok(())
}
