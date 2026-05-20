//! Tests for the release-help generation script boundary.
//!
//! The script shells out to `cargo-orthohelp`, so these tests place a fake
//! `cargo-orthohelp` executable first on `PATH`. This validates command
//! construction and output verification without requiring network access or a
//! real `cargo-orthohelp` installation.

#![cfg(unix)]

use anyhow::{Context, Result, ensure};
use insta::{Settings, assert_snapshot};
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
    let log_path = temp_dir.path().join("cargo-orthohelp-args.log");
    fs::create_dir_all(&fake_bin_dir).context("create fake cargo-orthohelp bin directory")?;
    write_fake_cargo_orthohelp(&fake_bin_dir.join("cargo-orthohelp"))?;
    Ok(ScriptFixture {
        _temp_dir: temp_dir,
        fake_bin_dir,
        log_path,
        out_dir,
    })
}

fn write_fake_cargo_orthohelp(path: &Path) -> Result<()> {
    fs::write(path, fake_cargo_orthohelp_script())
        .with_context(|| format!("write fake cargo-orthohelp script {}", path.display()))?;
    let mut permissions = fs::metadata(path)
        .with_context(|| format!("read fake cargo-orthohelp metadata {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("mark fake cargo-orthohelp executable {}", path.display()))
}

const fn fake_cargo_orthohelp_script() -> &'static str {
    r#"#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' "$*" >>"${ORTHOHELP_FAKE_LOG}"

if [[ "${ORTHOHELP_FAKE_FAIL:-0}" == "1" ]]; then
  echo "fake cargo-orthohelp failure" >&2
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
    cat >"$out_dir/man/man1/netsuke.1" <<'MAN'
.TH NETSUKE 1 "1970-01-01"
.SH NAME
netsuke \- dependency-aware build orchestration
.SH SYNOPSIS
.B netsuke
[OPTIONS]
MAN
    ;;
  ps)
    help_dir="$out_dir/powershell/$module_name/en-US"
    mkdir -p "$help_dir"
    printf 'function Invoke-%s { param() }\n' "$module_name" >"$out_dir/powershell/$module_name/$module_name.psm1"
    printf '@{\n  RootModule = "%s.psm1"\n  ModuleVersion = "0.1.0"\n}\n' "$module_name" >"$out_dir/powershell/$module_name/$module_name.psd1"
    cat >"$help_dir/$module_name-help.xml" <<MAML
<helpItems schema="maml">
  <command:command xmlns:command="http://schemas.microsoft.com/maml/dev/command/2004/10">
    <command:details>
      <command:name>$module_name</command:name>
    </command:details>
  </command:command>
</helpItems>
MAML
    printf 'TOPIC\n    about_%s\n' "$module_name" >"$help_dir/about_$module_name.help.txt"
    ;;
esac
"#
}

fn path_with_fake_cargo_orthohelp(fixture: &ScriptFixture) -> Result<OsString> {
    let existing_path = std::env::var_os("PATH").unwrap_or_default();
    let mut entries = vec![fixture.fake_bin_dir.clone()];
    entries.extend(std::env::split_paths(&existing_path));
    std::env::join_paths(entries).context("construct PATH with fake cargo-orthohelp first")
}

#[derive(Clone, Copy)]
struct ReleaseHelpRun<'a> {
    target: &'a str,
    module_name: &'a str,
    source_date_epoch: Option<&'a str>,
    fail_cargo: bool,
    skip_output: Option<&'a str>,
}

impl<'a> ReleaseHelpRun<'a> {
    const fn for_target(target: &'a str) -> Self {
        Self {
            target,
            module_name: "Netsuke",
            source_date_epoch: None,
            fail_cargo: false,
            skip_output: None,
        }
    }

    const fn module_name(mut self, value: &'a str) -> Self {
        self.module_name = value;
        self
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
        .arg(run.module_name)
        .current_dir(repo_root)
        .env("PATH", path_with_fake_cargo_orthohelp(fixture)?)
        .env("ORTHOHELP_FAKE_LOG", &fixture.log_path)
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

fn snapshot_settings() -> Settings {
    let mut settings = Settings::clone_current();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/release_help"
    ));
    settings
}

fn logged_args(fixture: &ScriptFixture) -> Result<String> {
    fs::read_to_string(&fixture.log_path).with_context(|| {
        format!(
            "read fake cargo-orthohelp log {}",
            fixture.log_path.display()
        )
    })
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
