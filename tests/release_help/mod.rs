//! Shared fixtures for release-help generation script tests.

#[cfg(test)]
mod script_functions;

use anyhow::{Context, Result};
use insta::Settings;
use rstest::fixture;
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

const SCRIPT_PATH: &str = "scripts/generate-release-help.sh";

pub fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SCRIPT_PATH)
}

pub struct ScriptFixture {
    _temp_dir: TempDir,
    fake_bin_dir: PathBuf,
    pub log_path: PathBuf,
    pub out_dir: PathBuf,
}

#[fixture]
pub fn script_fixture() -> Result<ScriptFixture> {
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

pub fn write_fake_cargo_orthohelp(path: &Path) -> Result<()> {
    fs::write(path, fake_cargo_orthohelp_script())
        .with_context(|| format!("write fake cargo-orthohelp script {}", path.display()))?;
    let mut permissions = fs::metadata(path)
        .with_context(|| format!("read fake cargo-orthohelp metadata {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("mark fake cargo-orthohelp executable {}", path.display()))
}

pub const fn fake_cargo_orthohelp_script() -> &'static str {
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

pub fn path_with_fake_cargo_orthohelp(fixture: &ScriptFixture) -> Result<OsString> {
    let existing_path = std::env::var_os("PATH").unwrap_or_default();
    let mut entries = vec![fixture.fake_bin_dir.clone()];
    entries.extend(std::env::split_paths(&existing_path));
    std::env::join_paths(entries).context("construct PATH with fake cargo-orthohelp first")
}

#[derive(Clone, Copy)]
pub struct ReleaseHelpRun<'a> {
    target: &'a str,
    module_name: &'a str,
    source_date_epoch: Option<&'a str>,
    fail_cargo: bool,
    skip_output: Option<&'a str>,
}

impl<'a> ReleaseHelpRun<'a> {
    pub const fn for_target(target: &'a str) -> Self {
        Self {
            target,
            module_name: "Netsuke",
            source_date_epoch: None,
            fail_cargo: false,
            skip_output: None,
        }
    }

    pub const fn module_name(mut self, value: &'a str) -> Self {
        self.module_name = value;
        self
    }

    pub const fn source_date_epoch(mut self, value: &'a str) -> Self {
        self.source_date_epoch = Some(value);
        self
    }

    pub const fn fail_cargo(mut self) -> Self {
        self.fail_cargo = true;
        self
    }

    pub const fn skip_output(mut self, format: &'a str) -> Self {
        self.skip_output = Some(format);
        self
    }
}

pub fn run_release_help(fixture: &ScriptFixture, run: ReleaseHelpRun<'_>) -> Result<Output> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new("bash");
    command
        .arg(script_path())
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

pub fn snapshot_settings() -> Settings {
    let mut settings = Settings::clone_current();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/release_help"
    ));
    settings
}

pub fn logged_args(fixture: &ScriptFixture) -> Result<String> {
    fs::read_to_string(&fixture.log_path).with_context(|| {
        format!(
            "read fake cargo-orthohelp log {}",
            fixture.log_path.display()
        )
    })
}
