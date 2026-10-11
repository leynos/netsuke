//! Preserve environment-policy authority across discovered config chains.

use super::super::merge_probe::{isolated_environment, merge_in_child};
use anyhow::{Context, Result, anyhow, bail, ensure};
use netsuke::ast::NetsukeManifest;
use netsuke::ast::Recipe;
use netsuke::cli::{self, Cli, ConfigEnvProvider};
use netsuke::cli_localization;
use netsuke::manifest::{self, EnvAccessPolicy, EnvReadError, EnvReader, ManifestEnvironment};
use netsuke::stdlib::NetworkPolicy;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::{TempDir, tempdir};

#[path = "config_discovery_env_provenance/discovery.rs"]
mod discovery;

/// Hold isolated project, home, XDG, and child environment state for one test.
struct ConfigSandbox {
    /// Project directory containing an automatically discovered file.
    project: TempDir,
    /// Fake home directory kept separate from the untrusted project.
    home: TempDir,
    /// Empty system-config directory returned by the isolation helper.
    _system_config: TempDir,
    /// Environment passed to the child merge process.
    environment: Vec<(OsString, OsString)>,
}

impl ConfigSandbox {
    /// Create isolated project and operator roots without changing the parent.
    fn new() -> Result<Self> {
        let project = tempdir().context("create project directory")?;
        let home = tempdir().context("create fake operator home")?;
        let (system_config, environment) = isolated_environment(home.path(), &[])?;
        Ok(Self {
            project,
            home,
            _system_config: system_config,
            environment,
        })
    }

    /// Return the isolated project path.
    fn project_path(&self) -> &Path {
        self.project.path()
    }

    /// Return the isolated operator home path.
    fn home_path(&self) -> &Path {
        self.home.path()
    }

    /// Merge CLI arguments through the child-process configuration boundary.
    fn merge(&self, args: &[&str]) -> Result<Cli> {
        merge_in_child(args, self.project_path(), &self.environment)
    }

    /// Discover the project file and merge the supplied command-line options.
    fn merge_auto(&self, extra_args: &[&str]) -> Result<Cli> {
        let directory = self
            .project_path()
            .to_str()
            .context("project path is UTF-8")?;
        let mut args = vec!["netsuke", "--directory", directory];
        args.extend_from_slice(extra_args);
        self.merge(&args)
    }

    /// Return the path used for the discovered XDG user configuration.
    fn user_config_dir(&self) -> PathBuf {
        self.home_path().join(".config/netsuke")
    }
}

/// Write one config file after creating its parent directory.
fn write_config(path: impl AsRef<Path>, contents: &str) -> Result<()> {
    let config_path = path.as_ref();
    std::fs::create_dir_all(config_path.parent().context("config file has a parent")?)
        .with_context(|| format!("create parent for {}", config_path.display()))?;
    std::fs::write(config_path, contents)
        .with_context(|| format!("write {}", config_path.display()))
}

/// Build a TOML `extends` entry for a path accepted by the config loader.
fn extends(path: &Path) -> Result<String> {
    let extends_path = path.to_str().context("extends path is UTF-8")?;
    Ok(format!("extends = {extends_path:?}\n"))
}

/// Provide an immutable environment snapshot for direct cached-merge tests.
#[derive(Clone)]
struct ConfigEnvironment(Vec<(OsString, OsString)>);

impl ConfigEnvProvider for ConfigEnvironment {
    fn get(&self, key: &str) -> Option<OsString> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }

    fn entries(&self) -> Vec<(OsString, OsString)> {
        self.0.clone()
    }
}

/// Create a reader that records each variable name before returning a value.
fn recording_env_reader(calls: &Arc<Mutex<Vec<String>>>) -> EnvReader {
    let reader_calls = Arc::clone(calls);
    Arc::new(move |name| {
        let Ok(mut recorded_calls) = reader_calls.lock() else {
            return Err(EnvReadError::NotPresent);
        };
        recorded_calls.push(name.to_owned());
        drop(recorded_calls);
        match name {
            "SAFE" => Ok("safe-value".to_owned()),
            "CI_TOKEN" => Ok("secret-value".to_owned()),
            _ => Err(EnvReadError::NotPresent),
        }
    })
}

/// Build a valid manifest command that renders one environment variable.
fn env_manifest(name: &str) -> String {
    format!(
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "rules:\n",
            "  - name: reveal\n",
            "    command: \"echo {{{{ env('{}') }}}}\"\n",
            "targets:\n",
            "  - name: output\n",
            "    rule: reveal\n",
        ),
        name
    )
}

/// Return the rendered command from the first rule when it uses a command recipe.
fn first_rule_command(manifest: &NetsukeManifest) -> Option<&str> {
    match &manifest.rules.first()?.recipe {
        Recipe::Command { command } => command.as_single(),
        _ => None,
    }
}

/// Reject a rendered lookup before the supplied reader records the name.
fn assert_blocked_without_read(policy: &EnvAccessPolicy, name: &str) -> Result<()> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = recording_env_reader(&calls);
    let Err(error) = manifest::from_str_with_env_and_policy(&env_manifest(name), &reader, policy)
    else {
        bail!("blocked environment access must fail rendering");
    };
    let error_chain = format!("{error:#}");
    ensure!(
        error_chain.contains("environment variable is blocked"),
        "the render error should report the blocked diagnostic: {error_chain}"
    );
    let recorded_calls = calls
        .lock()
        .map_err(|_| anyhow!("recording reader mutex poisoned"))?;
    ensure!(
        recorded_calls.is_empty(),
        "a blocked name must not reach the environment reader"
    );
    Ok(())
}

/// Render a manifest from disk through the same policy and injected reader.
fn assert_path_based_manifest_uses_policy(
    project_path: &Path,
    policy: EnvAccessPolicy,
) -> Result<()> {
    let manifest_path = project_path.join("Netsukefile");
    write_config(&manifest_path, &env_manifest("SAFE"))?;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = recording_env_reader(&calls);
    let environment = ManifestEnvironment::new(&reader, policy);
    let loaded = manifest::from_path_with_policy_and_environment(
        &manifest_path,
        NetworkPolicy::default(),
        &environment,
        None,
    )?;
    ensure!(
        first_rule_command(&loaded) == Some("echo safe-value"),
        "the bundle-based manifest environment should use the same policy"
    );
    Ok(())
}

/// Prevent a project ancestor's allowlist entry from reaching `env()`.
#[test]
fn inherited_project_grant_cannot_widen_operator_allowlist_during_rendering() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    write_config(
        sandbox.project_path().join(".netsuke.toml"),
        "extends = \"base.toml\"\n",
    )?;
    write_config(
        sandbox.project_path().join("base.toml"),
        "env_allow_var = [\"CI_TOKEN\"]\n",
    )?;
    let cli = sandbox.merge_auto(&["--env-allow-var", "SAFE"])?;

    ensure!(
        cli.env_allow_var == ["SAFE"],
        "the inherited project grant must be removed before ordinary merging"
    );
    let policy = cli.env_access_policy();
    ensure!(
        policy.evaluate("SAFE").is_ok(),
        "operator grant should remain"
    );
    ensure!(
        policy.evaluate("CI_TOKEN").is_err(),
        "the operator allowlist must remain restrictive"
    );

    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = recording_env_reader(&calls);
    let Err(error) =
        manifest::from_str_with_env_and_policy(&env_manifest("CI_TOKEN"), &reader, &policy)
    else {
        bail!("inherited project grant must not expose the token");
    };
    let error_chain = format!("{error:#}");
    ensure!(
        error_chain.contains("environment variable is blocked"),
        "the render error should contain the blocked diagnostic: {error_chain}"
    );
    let recorded_calls = calls
        .lock()
        .map_err(|_| anyhow!("recording reader mutex poisoned"))?;
    ensure!(
        recorded_calls.is_empty(),
        "the denied token must not reach the reader"
    );
    drop(recorded_calls);

    let rendered = manifest::from_str_with_env_and_policy(&env_manifest("SAFE"), &reader, &policy)?;
    ensure!(
        first_rule_command(&rendered) == Some("echo safe-value"),
        "the operator-allowlisted value should render"
    );
    let safe_calls = calls
        .lock()
        .map_err(|_| anyhow!("recording reader mutex poisoned"))?;
    ensure!(
        *safe_calls == ["SAFE"],
        "only the allowed name should reach the reader"
    );
    drop(safe_calls);
    assert_path_based_manifest_uses_policy(sandbox.project_path(), policy)?;
    Ok(())
}

/// Preserve default-allow when a project ancestor is the only grant source.
#[test]
fn inherited_project_grant_alone_does_not_activate_restrictive_mode() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    let shared = tempdir().context("create project-only shared config directory")?;
    let shared_file = shared.path().join("shared.toml");
    write_config(
        &shared_file,
        "env_allow_var = [\"CI_TOKEN\"]\nenv_block_var = [\"SAFE\"]\n",
    )?;
    write_config(
        sandbox.project_path().join(".netsuke.toml"),
        &extends(&shared_file)?,
    )?;
    let cli = sandbox.merge_auto(&[])?;
    ensure!(
        cli.env_allow_var.is_empty(),
        "the project-only occurrence must not retain its grant"
    );
    ensure!(
        cli.env_access_policy().evaluate("SAFE").is_err(),
        "the project-only occurrence should retain its cumulative block"
    );
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = recording_env_reader(&calls);
    let rendered = manifest::from_str_with_env_and_policy(
        &env_manifest("CI_TOKEN"),
        &reader,
        &cli.env_access_policy(),
    )?;

    ensure!(
        first_rule_command(&rendered) == Some("echo secret-value"),
        "an empty effective allowlist must preserve default-allow behaviour"
    );
    let recorded_calls = calls
        .lock()
        .map_err(|_| anyhow!("recording reader mutex poisoned"))?;
    ensure!(
        *recorded_calls == ["CI_TOKEN"],
        "the default-allowed name should reach the reader"
    );
    Ok(())
}
