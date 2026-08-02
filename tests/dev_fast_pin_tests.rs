//! Behavioural tests for how the `dev-fast` scripts resolve their pins.
//!
//! The scripts locate `tools/mold/VERSION` and `rust-toolchain.toml` relative to
//! their own path, so they work from any working directory and without the
//! Makefile supplying every path. These tests cover that resolution and its
//! refusals; the capability gate's own diagnostics live in
//! `dev_fast_check_tests.rs`.
//!
//! A malformed pin must be refused rather than silently rewritten — a corrupted
//! version would otherwise reach a download URL looking well-formed.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use rstest::rstest;
use test_support::dev_fast::{
    PinOverrides, Sandbox, combined, pinned_mold_version, pinned_toolchain,
};

/// Run without the pin-file variables, the check must still find the committed
/// pins by locating the repository from the script's own path.
#[test]
fn falls_back_to_the_committed_pins_when_no_overrides_are_given() -> Result<()> {
    let sandbox = Sandbox::new()?;
    // Both fakes report exactly what the committed pins name, so the check can
    // only pass if it read those files rather than defaulting to nothing. The
    // mold goes on the sandbox PATH directly: only the Make recipes prepend the
    // install prefix, and this case invokes the script itself.
    sandbox.write_mold(&sandbox.bin(), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;

    let output = sandbox.script_with("dev-fast-check.sh", PinOverrides::Omitted, &[])?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "check should pass on the committed pins, got `{text}`"
    );
    ensure!(
        text.contains(&format!("mold {}", pinned_mold_version()?)),
        "should read tools/mold/VERSION by default, got `{text}`"
    );
    ensure!(
        text.contains(&pinned_toolchain()?),
        "should read rust-toolchain.toml by default, got `{text}`"
    );
    Ok(())
}

/// The drift diagnostic names the pinned version, so a mismatching fake proves
/// which file the default resolved to rather than merely that some file was
/// read.
#[test]
fn default_pins_are_the_committed_ones_not_an_empty_fallback() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.bin(), "99.0.0")?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;

    let output = sandbox.script_with("dev-fast-check.sh", PinOverrides::Omitted, &[])?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "a drift should fail, got `{text}`"
    );
    ensure!(
        text.contains(&format!("the pin {}", pinned_mold_version()?)),
        "the drift message should name the committed pin, got `{text}`"
    );
    Ok(())
}

/// An override must still win over the default.
#[test]
fn an_explicit_pin_override_wins_over_the_committed_default() -> Result<()> {
    let sandbox = Sandbox::new()?;
    let toolchain_pin = sandbox.home().join("rust-toolchain.toml");
    sandbox.write_file(
        &toolchain_pin,
        "[toolchain]\nchannel = \"nightly-1970-01-01\"\n",
    )?;
    sandbox.write_mold(&sandbox.bin(), &pinned_mold_version()?)?;
    // rustup knows only the committed toolchain, so the run can fail only if
    // the override displaced the default.
    sandbox.write_rustup(&pinned_toolchain()?, true)?;

    let output = sandbox.script_with(
        "dev-fast-check.sh",
        PinOverrides::Omitted,
        &[("RUST_TOOLCHAIN_FILE", toolchain_pin.to_string())],
    )?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "the overridden toolchain is not installed, so the check should fail: `{text}`"
    );
    ensure!(
        text.contains("nightly-1970-01-01"),
        "the override should be the toolchain reported, got `{text}`"
    );
    Ok(())
}

/// A pin path that does not exist must produce the ordinary actionable
/// diagnostic, whether it arrived as a default or as an override.
#[test]
fn a_missing_pin_file_reports_the_actionable_diagnostic() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.bin(), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let missing = sandbox.home().join("absent/MOLD_VERSION");

    let output = sandbox.script_with(
        "dev-fast-check.sh",
        PinOverrides::Omitted,
        &[("MOLD_VERSION_FILE", missing.to_string())],
    )?;
    let text = combined(&output);

    ensure!(!output.status.success(), "should fail, got `{text}`");
    ensure!(
        text.contains("missing version pin") && text.contains(missing.as_str()),
        "should name the missing pin file, got `{text}`"
    );
    Ok(())
}

/// A malformed pin must be refused, never silently rewritten.
///
/// Deleting every whitespace character would turn `1.2 3` into `1.23` and two
/// lines into their concatenation, so a corrupted pin would reach the download
/// URL looking well-formed. Boundary whitespace is the only kind trimmed.
#[rstest]
#[case::internal_space("1.2 3\n", "contains whitespace")]
#[case::internal_tab("1.2\t3\n", "contains whitespace")]
#[case::two_lines("1.2.3\n4.5.6\n", "expected one line")]
#[case::blank("\n", "empty version pin")]
fn a_malformed_version_pin_is_refused_not_rewritten(
    #[case] contents: &str,
    #[case] expected: &str,
) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.bin(), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let pin = sandbox.home().join("MOLD_VERSION");
    sandbox.write_file(&pin, contents)?;

    let output = sandbox.script_with(
        "dev-fast-check.sh",
        PinOverrides::Omitted,
        &[("MOLD_VERSION_FILE", pin.to_string())],
    )?;
    let text = combined(&output);

    ensure!(!output.status.success(), "should refuse, got `{text}`");
    ensure!(
        text.contains(expected),
        "should report `{expected}`, got `{text}`"
    );
    Ok(())
}

/// Whitespace either side of the version is trimmed, so a pin file written with
/// a trailing newline or stray indentation still resolves.
#[rstest]
#[case::trailing_newline("2.41.0\n")]
#[case::surrounding_spaces("   2.41.0   \n")]
#[case::no_trailing_newline("2.41.0")]
fn boundary_whitespace_around_a_pin_is_trimmed(#[case] contents: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.bin(), "2.41.0")?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    let pin = sandbox.home().join("MOLD_VERSION");
    sandbox.write_file(&pin, contents)?;

    let output = sandbox.script_with(
        "dev-fast-check.sh",
        PinOverrides::Omitted,
        &[("MOLD_VERSION_FILE", pin.to_string())],
    )?;
    let text = combined(&output);

    ensure!(
        output.status.success(),
        "`{contents:?}` should resolve to 2.41.0, got `{text}`"
    );
    Ok(())
}
