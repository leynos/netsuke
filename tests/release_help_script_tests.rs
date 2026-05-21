//! Tests for the release-help generation script boundary.
//!
//! The script shells out to `cargo-orthohelp`, so these tests place a fake
//! `cargo-orthohelp` executable first on `PATH`. This validates command
//! construction and output verification without requiring network access or a
//! real `cargo-orthohelp` installation.

#![cfg(unix)]

mod release_help;

use anyhow::{Context, Result, ensure};
use insta::assert_snapshot;
use release_help::{
    ReleaseHelpRun, ScriptFixture, logged_args, run_release_help, script_fixture, snapshot_settings,
};
use rstest::rstest;
use std::fs;

#[rstest]
fn generates_manual_page_for_non_windows_target(
    script_fixture: Result<ScriptFixture>,
) -> Result<()> {
    let fixture = script_fixture?;
    let output = run_release_help(
        &fixture,
        ReleaseHelpRun::for_target("x86_64-unknown-linux-gnu").source_date_epoch("1"),
    )?;

    ensure!(output.status.success(), "script failed: {output:?}");
    ensure!(
        fixture.out_dir.join("man/man1/netsuke.1").is_file(),
        "manual page should be generated"
    );
    let man_page = fs::read_to_string(fixture.out_dir.join("man/man1/netsuke.1"))
        .context("read generated man page")?;
    snapshot_settings().bind(|| {
        assert_snapshot!("generated_man_page", man_page);
    });
    let log = logged_args(&fixture)?;
    ensure!(
        log.contains("--format man"),
        "expected man generation command, got {log}"
    );
    ensure!(
        log.contains("--man-date 1970-01-01"),
        "expected reproducible date from SOURCE_DATE_EPOCH, got {log}"
    );
    ensure!(
        !log.contains("--format ps"),
        "non-Windows targets should not generate PowerShell help"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains("target=x86_64-unknown-linux-gnu format=man locale=en-US"),
        "script should log cargo-orthohelp invocation context, got {stderr}"
    );
    Ok(())
}

#[rstest]
fn generates_powershell_help_for_windows_target(
    script_fixture: Result<ScriptFixture>,
) -> Result<()> {
    let fixture = script_fixture?;
    let output = run_release_help(
        &fixture,
        ReleaseHelpRun::for_target("x86_64-pc-windows-msvc")
            .module_name("CustomNetsuke")
            .source_date_epoch("1"),
    )?;

    ensure!(output.status.success(), "script failed: {output:?}");
    for path in [
        "powershell/CustomNetsuke/CustomNetsuke.psm1",
        "powershell/CustomNetsuke/CustomNetsuke.psd1",
        "powershell/CustomNetsuke/en-US/CustomNetsuke-help.xml",
        "powershell/CustomNetsuke/en-US/about_CustomNetsuke.help.txt",
    ] {
        ensure!(
            fixture.out_dir.join(path).is_file(),
            "expected PowerShell help file {path}"
        );
    }
    let log = logged_args(&fixture)?;
    ensure!(
        log.contains("--format ps"),
        "Windows targets should generate PowerShell help, got {log}"
    );
    ensure!(
        log.contains("--ps-module-name CustomNetsuke"),
        "PowerShell module name should be pinned, got {log}"
    );
    let ps_module = fs::read_to_string(
        fixture
            .out_dir
            .join("powershell/CustomNetsuke/CustomNetsuke.psm1"),
    )
    .context("read generated PowerShell module")?;
    let ps_manifest = fs::read_to_string(
        fixture
            .out_dir
            .join("powershell/CustomNetsuke/CustomNetsuke.psd1"),
    )
    .context("read generated PowerShell module manifest")?;
    let maml = fs::read_to_string(
        fixture
            .out_dir
            .join("powershell/CustomNetsuke/en-US/CustomNetsuke-help.xml"),
    )
    .context("read generated PowerShell MAML help")?;
    let about_help = fs::read_to_string(
        fixture
            .out_dir
            .join("powershell/CustomNetsuke/en-US/about_CustomNetsuke.help.txt"),
    )
    .context("read generated PowerShell about help")?;
    snapshot_settings().bind(|| {
        assert_snapshot!(
            "generated_powershell_help",
            format!("{ps_module}\n---\n{ps_manifest}\n---\n{maml}\n---\n{about_help}")
        );
    });
    Ok(())
}

#[rstest]
#[case(None, "1970-01-01")]
#[case(Some("86400"), "1970-01-02")]
#[case(Some("not-a-number"), "1970-01-01")]
fn resolves_manual_date_from_source_date_epoch(
    script_fixture: Result<ScriptFixture>,
    #[case] source_date_epoch: Option<&str>,
    #[case] expected_date: &str,
) -> Result<()> {
    let fixture = script_fixture?;
    let mut run = ReleaseHelpRun::for_target("x86_64-unknown-linux-gnu");
    if let Some(value) = source_date_epoch {
        run = run.source_date_epoch(value);
    }
    let output = run_release_help(&fixture, run)?;

    ensure!(output.status.success(), "script failed: {output:?}");
    let log = logged_args(&fixture)?;
    ensure!(
        log.contains(&format!("--man-date {expected_date}")),
        "expected man date {expected_date}, got {log}"
    );
    if source_date_epoch == Some("not-a-number") {
        let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
        ensure!(
            stderr.contains("Invalid SOURCE_DATE_EPOCH"),
            "invalid SOURCE_DATE_EPOCH should emit a warning, got {stderr}"
        );
    }
    Ok(())
}

#[rstest]
fn propagates_cargo_orthohelp_failures(script_fixture: Result<ScriptFixture>) -> Result<()> {
    let fixture = script_fixture?;
    let output = run_release_help(
        &fixture,
        ReleaseHelpRun::for_target("x86_64-unknown-linux-gnu")
            .source_date_epoch("1")
            .fail_cargo(),
    )?;

    ensure!(
        !output.status.success(),
        "script should fail when cargo-orthohelp fails"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains("fake cargo-orthohelp failure"),
        "expected cargo-orthohelp failure to be visible, got {stderr}"
    );
    ensure!(
        stderr.contains("cargo-orthohelp failed"),
        "expected contextual failure annotation, got {stderr}"
    );
    ensure!(
        stderr.contains("target=x86_64-unknown-linux-gnu format=man locale=en-US"),
        "expected target, format, and locale in failure context, got {stderr}"
    );
    Ok(())
}

#[rstest]
#[case("x86_64-unknown-linux-gnu", "man", "manual page was not generated")]
#[case(
    "x86_64-pc-windows-msvc",
    "ps",
    "PowerShell module script was not generated"
)]
fn fails_when_expected_help_output_is_missing(
    script_fixture: Result<ScriptFixture>,
    #[case] target: &str,
    #[case] skipped_format: &str,
    #[case] expected_error: &str,
) -> Result<()> {
    let fixture = script_fixture?;
    let output = run_release_help(
        &fixture,
        ReleaseHelpRun::for_target(target)
            .source_date_epoch("1")
            .skip_output(skipped_format),
    )?;

    ensure!(
        !output.status.success(),
        "script should fail when {skipped_format} output is missing"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains(expected_error),
        "expected missing output error {expected_error:?}, got {stderr}"
    );
    Ok(())
}
