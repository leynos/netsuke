//! Shared fixtures for release-help generation script tests.

#[cfg(test)]
mod script_functions;

mod fake_renderer;

use fake_renderer::fake_cargo_orthohelp_script;

use anyhow::{Context, Result};
use insta::Settings;
use netsuke::cli::ReleaseHelpCli;
use ortho_config::docs::OrthoConfigDocs;
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

/// Return the repository path of the release-help generation script.
pub fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SCRIPT_PATH)
}

pub struct ScriptFixture {
    _temp_dir: TempDir,
    fake_bin_dir: PathBuf,
    pub log_path: PathBuf,
    pub out_dir: PathBuf,
    metadata_path: PathBuf,
}

/// Prepare an isolated fixture for release-help generation tests.
///
/// # Errors
///
/// Returns an error when the temporary directories or fake executable cannot
/// be created.
#[fixture]
pub fn script_fixture() -> Result<ScriptFixture> {
    let temp_dir = tempfile::tempdir().context("create release help test tempdir")?;
    let fake_bin_dir = temp_dir.path().join("bin");
    let out_dir = temp_dir.path().join("out");
    let log_path = temp_dir.path().join("cargo-orthohelp-args.log");
    let metadata_path = temp_dir.path().join("release-help-metadata.json");
    fs::create_dir_all(&fake_bin_dir).context("create fake cargo-orthohelp bin directory")?;
    write_fake_cargo_orthohelp(&fake_bin_dir.join("cargo-orthohelp"))?;
    Ok(ScriptFixture {
        _temp_dir: temp_dir,
        fake_bin_dir,
        log_path,
        out_dir,
        metadata_path,
    })
}

/// Write an executable fake `cargo-orthohelp` script to `path`.
///
/// # Errors
///
/// Returns an error when the script cannot be written, inspected, or marked
/// executable.
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

/// Prepend the fixture's fake `cargo-orthohelp` directory to `PATH`.
///
/// # Errors
///
/// Returns an error when the resulting platform-specific `PATH` cannot be
/// constructed.
#[expect(
    clippy::disallowed_methods,
    reason = "locating build artefacts Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]
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
    fallback_value: Option<(&'a str, &'a str)>,
    omit_flag: Option<&'a str>,
}

impl<'a> ReleaseHelpRun<'a> {
    /// Create release-help run settings for a target triple.
    pub const fn for_target(target: &'a str) -> Self {
        Self {
            target,
            module_name: "Netsuke",
            source_date_epoch: None,
            fail_cargo: false,
            skip_output: None,
            fallback_value: None,
            omit_flag: None,
        }
    }

    /// Set the PowerShell module name for the run.
    pub const fn module_name(mut self, value: &'a str) -> Self {
        self.module_name = value;
        self
    }

    /// Set the source date epoch used by the run.
    pub const fn source_date_epoch(mut self, value: &'a str) -> Self {
        self.source_date_epoch = Some(value);
        self
    }

    /// Configure the run to make the fake generator fail.
    pub const fn fail_cargo(mut self) -> Self {
        self.fail_cargo = true;
        self
    }

    /// Configure the run to omit one generated output format.
    pub const fn skip_output(mut self, format: &'a str) -> Self {
        self.skip_output = Some(format);
        self
    }

    /// Configure the fake generator to publish a machine value name.
    ///
    /// The generator writes the name into `format`'s placeholder markup, so the
    /// script's real-output guard is exercised as it would be by a regression.
    pub const fn fallback_value(mut self, format: &'a str, value: &'a str) -> Self {
        self.fallback_value = Some((format, value));
        self
    }

    /// Configure the fake generator to drop one documented long flag.
    pub const fn omit_flag(mut self, long: &'a str) -> Self {
        self.omit_flag = Some(long);
        self
    }
}

/// Run the release-help script with the supplied fixture and settings.
///
/// # Errors
///
/// Returns an error when metadata cannot be serialised or written, the test
/// command environment cannot be prepared, or the script cannot be started.
pub fn run_release_help(fixture: &ScriptFixture, run: ReleaseHelpRun<'_>) -> Result<Output> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::write(
        &fixture.metadata_path,
        serde_json::to_vec(&ReleaseHelpCli::get_doc_metadata())
            .context("serialise release-help metadata for fake cargo-orthohelp")?,
    )
    .with_context(|| {
        format!(
            "write release-help metadata fixture {}",
            fixture.metadata_path.display()
        )
    })?;
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
        .env("ORTHOHELP_FAKE_METADATA", &fixture.metadata_path)
        .env_remove("GITHUB_RUN_ID")
        .env_remove("GITHUB_RUN_ATTEMPT")
        .env_remove("RELEASE_HELP_BUILD_ID")
        .env_remove("RELEASE_HELP_CORRELATION_ID")
        .env_remove("SOURCE_DATE_EPOCH")
        .env_remove("ORTHOHELP_FAKE_FAIL")
        .env_remove("ORTHOHELP_FAKE_SKIP_OUTPUT")
        .env_remove("ORTHOHELP_FAKE_FALLBACK")
        .env_remove("ORTHOHELP_FAKE_FALLBACK_FORMAT")
        .env_remove("ORTHOHELP_FAKE_OMIT_FLAG");

    if let Some(value) = run.source_date_epoch {
        command.env("SOURCE_DATE_EPOCH", value);
    }
    if run.fail_cargo {
        command.env("ORTHOHELP_FAKE_FAIL", "1");
    }
    if let Some(format) = run.skip_output {
        command.env("ORTHOHELP_FAKE_SKIP_OUTPUT", format);
    }
    if let Some((format, value)) = run.fallback_value {
        command
            .env("ORTHOHELP_FAKE_FALLBACK_FORMAT", format)
            .env("ORTHOHELP_FAKE_FALLBACK", value);
    }
    if let Some(long) = run.omit_flag {
        command.env("ORTHOHELP_FAKE_OMIT_FLAG", long);
    }

    command.output().context("run release help script")
}

/// Configure snapshot storage for release-help artefacts.
pub fn snapshot_settings() -> Settings {
    let mut settings = Settings::clone_current();
    settings.set_snapshot_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/snapshots/release_help"
    ));
    settings
}

/// Read the arguments recorded by the fake generator.
///
/// # Errors
///
/// Returns an error when the fake generator log cannot be read.
pub fn logged_args(fixture: &ScriptFixture) -> Result<String> {
    fs::read_to_string(&fixture.log_path).with_context(|| {
        format!(
            "read fake cargo-orthohelp log {}",
            fixture.log_path.display()
        )
    })
}
