//! Tests for the release-help generation script boundary.
//!
//! The script shells out to `cargo orthohelp`, so these tests place a fake
//! `cargo` executable first on `PATH`. This validates command construction and
//! output verification without requiring network access or a real
//! `cargo-orthohelp` installation.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use rstest::{fixture, rstest};
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

const SCRIPT_PATH: &str = "scripts/generate-release-help.sh";

struct ScriptFixture {
    _temp_dir: TempDir,
    fake_bin_dir: PathBuf,
    log_path: PathBuf,
    out_dir: PathBuf,
}

#[fixture]
fn script_fixture() -> Result<ScriptFixture> {
    let temp_dir = tempfile::tempdir().context("create release help test tempdir")?;
    let fake_bin_dir = temp_dir.path().join("bin");
    let out_dir = temp_dir.path().join("out");
    let log_path = temp_dir.path().join("cargo-args.log");
    fs::create_dir_all(&fake_bin_dir).context("create fake cargo bin directory")?;
    write_fake_cargo(&fake_bin_dir.join("cargo"))?;
    Ok(ScriptFixture {
        _temp_dir: temp_dir,
        fake_bin_dir,
        log_path,
        out_dir,
    })
}

fn write_fake_cargo(path: &Path) -> Result<()> {
    fs::write(path, fake_cargo_script())
        .with_context(|| format!("write fake cargo script {}", path.display()))?;
    let mut permissions = fs::metadata(path)
        .with_context(|| format!("read fake cargo metadata {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("mark fake cargo executable {}", path.display()))
}

const fn fake_cargo_script() -> &'static str {
    r#"#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' "$*" >>"${CARGO_FAKE_LOG}"

if [[ "${ORTHOHELP_FAKE_FAIL:-0}" == "1" ]]; then
  echo "fake cargo failure" >&2
  exit 42
fi

format=""
out_dir=""
module_name="Netsuke"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --format)
      format="$2"
      shift 2
      ;;
    --out-dir)
      out_dir="$2"
      shift 2
      ;;
    --ps-module-name)
      module_name="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

if [[ "${ORTHOHELP_FAKE_SKIP_OUTPUT:-}" == "$format" ]]; then
  exit 0
fi

case "$format" in
  man)
    mkdir -p "$out_dir/man/man1"
    printf '.TH netsuke 1\n' >"$out_dir/man/man1/netsuke.1"
    ;;
  ps)
    help_dir="$out_dir/powershell/$module_name/en-US"
    mkdir -p "$help_dir"
    printf '# module\n' >"$out_dir/powershell/$module_name/$module_name.psm1"
    printf '@{}\n' >"$out_dir/powershell/$module_name/$module_name.psd1"
    printf '<helpItems />\n' >"$help_dir/$module_name-help.xml"
    printf 'about help\n' >"$help_dir/about_$module_name.help.txt"
    ;;
esac
"#
}

fn path_with_fake_cargo(fixture: &ScriptFixture) -> Result<OsString> {
    let existing_path = std::env::var_os("PATH").unwrap_or_default();
    let mut entries = vec![fixture.fake_bin_dir.clone()];
    entries.extend(std::env::split_paths(&existing_path));
    std::env::join_paths(entries).context("construct PATH with fake cargo first")
}

#[derive(Clone, Copy)]
struct ReleaseHelpRun<'a> {
    target: &'a str,
    source_date_epoch: Option<&'a str>,
    fail_cargo: bool,
    skip_output: Option<&'a str>,
}

impl<'a> ReleaseHelpRun<'a> {
    const fn for_target(target: &'a str) -> Self {
        Self {
            target,
            source_date_epoch: None,
            fail_cargo: false,
            skip_output: None,
        }
    }

    const fn source_date_epoch(mut self, value: &'a str) -> Self {
        self.source_date_epoch = Some(value);
        self
    }

    const fn fail_cargo(mut self) -> Self {
        self.fail_cargo = true;
        self
    }

    const fn skip_output(mut self, format: &'a str) -> Self {
        self.skip_output = Some(format);
        self
    }
}

fn run_release_help(fixture: &ScriptFixture, run: ReleaseHelpRun<'_>) -> Result<Output> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new("bash");
    command
        .arg(repo_root.join(SCRIPT_PATH))
        .arg(run.target)
        .arg("netsuke")
        .arg(&fixture.out_dir)
        .current_dir(repo_root)
        .env("PATH", path_with_fake_cargo(fixture)?)
        .env("CARGO_FAKE_LOG", &fixture.log_path)
        .env_remove("SOURCE_DATE_EPOCH")
        .env_remove("ORTHOHELP_FAKE_FAIL")
        .env_remove("ORTHOHELP_FAKE_SKIP_OUTPUT");

    if let Some(value) = run.source_date_epoch {
        command.env("SOURCE_DATE_EPOCH", value);
    }
    if run.fail_cargo {
        command.env("ORTHOHELP_FAKE_FAIL", "1");
    }
    if let Some(format) = run.skip_output {
        command.env("ORTHOHELP_FAKE_SKIP_OUTPUT", format);
    }

    command.output().context("run release help script")
}

fn logged_args(fixture: &ScriptFixture) -> Result<String> {
    fs::read_to_string(&fixture.log_path)
        .with_context(|| format!("read fake cargo log {}", fixture.log_path.display()))
}

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
    let log = logged_args(&fixture)?;
    ensure!(
        log.contains("orthohelp --format man"),
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
    Ok(())
}

#[rstest]
fn generates_powershell_help_for_windows_target(
    script_fixture: Result<ScriptFixture>,
) -> Result<()> {
    let fixture = script_fixture?;
    let output = run_release_help(
        &fixture,
        ReleaseHelpRun::for_target("x86_64-pc-windows-msvc").source_date_epoch("1"),
    )?;

    ensure!(output.status.success(), "script failed: {output:?}");
    for path in [
        "powershell/Netsuke/Netsuke.psm1",
        "powershell/Netsuke/Netsuke.psd1",
        "powershell/Netsuke/en-US/Netsuke-help.xml",
        "powershell/Netsuke/en-US/about_Netsuke.help.txt",
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
        log.contains("--ps-module-name Netsuke"),
        "PowerShell module name should be pinned, got {log}"
    );
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
        "script should fail when cargo orthohelp fails"
    );
    let stderr = String::from_utf8(output.stderr).context("stderr should be UTF-8")?;
    ensure!(
        stderr.contains("fake cargo failure"),
        "expected cargo failure to be visible, got {stderr}"
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
