//! Behavioural tests for the `dev-fast-check` capability gate.
//!
//! The gate's whole purpose is to turn a missing tool into an actionable
//! message instead of an opaque codegen-backend or linker failure, so these
//! tests assert on the diagnostics and exit status rather than on any build
//! artefact. Every case runs against fakes in a sandboxed `PATH`; none needs
//! mold, rustup, or a network.

#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Result, ensure};
use rstest::rstest;
use test_support::dev_fast::{
    PinOverrides, Sandbox, combined, pinned_mold_version, pinned_toolchain,
};

/// A sandbox with everything present and matching the pins.
fn healthy_sandbox() -> Result<Sandbox> {
    let sandbox = Sandbox::new()?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    Ok(sandbox)
}

#[test]
fn reports_resolved_path_and_version_when_prerequisites_are_met() -> Result<()> {
    let sandbox = healthy_sandbox()?;
    let output = sandbox.make("dev-fast-check")?;
    let text = combined(&output);

    ensure!(output.status.success(), "check should pass, got `{text}`");
    ensure!(
        text.contains(&format!("mold {}", pinned_mold_version()?)),
        "should report the resolved version, got `{text}`"
    );
    ensure!(
        text.contains(&sandbox.prefix().join("bin/mold").to_string()),
        "should name the resolved path so an unexpected pick is visible, got `{text}`"
    );
    Ok(())
}

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

/// The regression this guards: the Makefile unconditionally exports
/// `$(HOME)/.local/bin` ahead of the caller's `PATH`, so an overridden
/// `DEV_FAST_PREFIX` used to be installed to but never selected.
#[test]
fn overridden_prefix_wins_over_a_mold_in_the_default_location() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    // A decoy in the location the Makefile's export would otherwise favour.
    sandbox.write_mold(&sandbox.home().join(".local/bin"), "0.0.0-decoy")?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;

    let output = sandbox.make("dev-fast-check")?;
    let text = combined(&output);

    ensure!(output.status.success(), "check should pass, got `{text}`");
    ensure!(
        text.contains(&sandbox.prefix().join("bin/mold").to_string()),
        "the overridden prefix should win PATH resolution, got `{text}`"
    );
    ensure!(
        !text.contains("0.0.0-decoy"),
        "the default-location mold should not be selected, got `{text}`"
    );
    Ok(())
}

/// An advisory pin is not a pin: a `mold` that does not match it must fail the
/// gate, so the linker actually used cannot silently diverge from the one the
/// repository claims.
#[test]
fn rejects_a_version_drift_from_the_pin() -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    sandbox.write_mold(&sandbox.prefix().join("bin"), "99.0.0")?;

    let output = sandbox.make("dev-fast-check")?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "a drifting mold should fail the check, got `{text}`"
    );
    ensure!(
        text.contains("run make install-dev-fast to match"),
        "the remedy should be named, got `{text}`"
    );
    Ok(())
}

/// Each unusable-tool case names the fault and the remedy, and exits non-zero
/// so `dev-build` and `dev-test` stop before Cargo runs.
/// `arrange` is carried as a function rather than dispatched on a name, so
/// adding a case cannot leave an unhandled arm behind.
#[derive(Copy, Clone)]
struct FailureCase {
    arrange: fn(&Sandbox) -> Result<()>,
    expected: &'static str,
}

fn without_mold(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    Ok(())
}

/// A truncated download or an unresolved shared library: on `PATH`, but
/// incapable of reporting a version.
fn with_unrunnable_mold(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_fake(&sandbox.prefix().join("bin"), "mold", "exit 1")?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    Ok(())
}

fn without_rustup(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    Ok(())
}

fn without_pinned_toolchain(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup("nightly-1970-01-01", true)?;
    Ok(())
}

fn without_cranelift_component(sandbox: &Sandbox) -> Result<()> {
    sandbox.write_mold(&sandbox.prefix().join("bin"), &pinned_mold_version()?)?;
    sandbox.write_rustup(&pinned_toolchain()?, false)?;
    Ok(())
}

#[rstest]
#[case::mold_absent(FailureCase { arrange: without_mold, expected: "mold not found on PATH" })]
#[case::mold_unrunnable(
    FailureCase { arrange: with_unrunnable_mold, expected: "cannot report its version" }
)]
#[case::rustup_absent(
    FailureCase { arrange: without_rustup, expected: "rustup not found on PATH" }
)]
#[case::toolchain_absent(
    FailureCase { arrange: without_pinned_toolchain, expected: "is not installed" }
)]
#[case::component_absent(
    FailureCase { arrange: without_cranelift_component, expected: "is not installed for" }
)]
fn unusable_prerequisites_fail_with_an_actionable_message(#[case] case: FailureCase) -> Result<()> {
    let sandbox = Sandbox::new()?;
    (case.arrange)(&sandbox)?;

    let output = sandbox.make("dev-fast-check")?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "case should fail the check, got `{text}`"
    );
    ensure!(
        text.contains(case.expected),
        "case should explain the fault (`{}`), got `{text}`",
        case.expected
    );
    ensure!(
        text.contains("make install-dev-fast") || text.contains("https://rustup.rs"),
        "case should point at a remedy, got `{text}`"
    );
    Ok(())
}

/// `dev-build` and `dev-test` depend on the check, so a missing prerequisite
/// must stop them before Cargo is invoked.
#[rstest]
#[case("dev-build")]
#[case("dev-test")]
fn build_targets_stop_when_the_check_fails(#[case] target: &str) -> Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.write_rustup(&pinned_toolchain()?, true)?;
    // No mold anywhere, and no cargo in the sandbox either: if the recipe ran,
    // the failure would name cargo rather than the capability check.
    let output = sandbox.make(target)?;
    let text = combined(&output);

    ensure!(
        !output.status.success(),
        "`{target}` should fail, got `{text}`"
    );
    ensure!(
        text.contains("mold not found on PATH"),
        "`{target}` should fail in the capability check, got `{text}`"
    );
    Ok(())
}
