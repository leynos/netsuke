//! Tests for operator, project, and explicit config-chain provenance.

use super::*;
use tempfile::tempdir;

/// Retain environment grants from a trusted XDG chain.
#[test]
fn operator_xdg_chain_retains_inherited_environment_grants() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    let config_dir = sandbox.user_config_dir();
    write_config(config_dir.join("config.toml"), "extends = \"base.toml\"\n")?;
    write_config(
        config_dir.join("base.toml"),
        "env_allow_var = [\"CI_TOKEN\"]\n",
    )?;
    let cli = sandbox.merge_auto(&[])?;

    ensure!(
        cli.env_access_policy().evaluate("CI_TOKEN").is_ok(),
        "operator-chain grants must retain operator authority"
    );
    Ok(())
}

/// Preserve the operator occurrence when both roots reach one physical file.
#[test]
fn shared_xdg_and_project_ancestor_keep_operator_grant_and_project_block() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    let shared = tempdir().context("create shared configuration directory")?;
    let shared_file = shared.path().join("shared.toml");
    write_config(
        &shared_file,
        "env_allow_var = [\"CI_TOKEN\"]\nenv_block_var = [\"SAFE\"]\n",
    )?;
    let shared_extends = extends(&shared_file)?;
    write_config(
        sandbox.user_config_dir().join("config.toml"),
        &shared_extends,
    )?;
    write_config(
        sandbox.project_path().join(".netsuke.toml"),
        &shared_extends,
    )?;
    let cli = sandbox.merge_auto(&[])?;
    let policy = cli.env_access_policy();

    ensure!(
        cli.env_allow_var == ["CI_TOKEN"],
        "the operator occurrence should retain its grant after project quarantine"
    );
    ensure!(
        policy.evaluate("CI_TOKEN").is_ok(),
        "operator grant should apply"
    );
    ensure!(
        policy.evaluate("SAFE").is_err(),
        "a block from the shared project occurrence should remain cumulative"
    );
    assert_blocked_without_read(&policy, "SAFE")?;
    Ok(())
}

/// Keep an inherited project block authoritative over a CLI allow.
#[test]
fn inherited_project_block_overrides_operator_allow() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    write_config(
        sandbox.project_path().join(".netsuke.toml"),
        "extends = \"base.toml\"\n",
    )?;
    write_config(
        sandbox.project_path().join("base.toml"),
        "env_block_var = [\"CI_TOKEN\"]\n",
    )?;
    let cli = sandbox.merge_auto(&["--env-allow-var", "CI_TOKEN"])?;
    let policy = cli.env_access_policy();

    ensure!(
        policy.evaluate("CI_TOKEN").is_err(),
        "an inherited block must override an operator allow"
    );
    assert_blocked_without_read(&policy, "CI_TOKEN")
}

/// Give an operator block precedence over an operator-chain grant.
#[test]
fn operator_block_overrides_operator_allow() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    write_config(
        sandbox.user_config_dir().join("config.toml"),
        "extends = \"base.toml\"\n",
    )?;
    write_config(
        sandbox.user_config_dir().join("base.toml"),
        "env_allow_var = [\"CI_TOKEN\"]\nenv_block_var = [\"CI_TOKEN\"]\n",
    )?;
    let cli = sandbox.merge_auto(&["--env-allow-var", "CI_TOKEN"])?;
    let policy = cli.env_access_policy();

    ensure!(
        policy.evaluate("CI_TOKEN").is_err(),
        "the operator block must override the matching operator allow"
    );
    assert_blocked_without_read(&policy, "CI_TOKEN")
}

/// Keep an ancestor grant when the caller explicitly selects its chain.
#[test]
fn explicit_config_chain_retains_ancestor_environment_grants() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    let config_dir = sandbox.project_path().join("explicit");
    write_config(config_dir.join("config.toml"), "extends = \"base.toml\"\n")?;
    write_config(
        config_dir.join("base.toml"),
        "env_allow_var = [\"CI_TOKEN\"]\n",
    )?;
    write_config(
        sandbox.project_path().join(".netsuke.toml"),
        "env_allow_var = [\"PROJECT_TOKEN\"]\n",
    )?;
    let config = config_dir
        .join("config.toml")
        .to_str()
        .context("explicit config path is UTF-8")?
        .to_owned();
    let cli = sandbox.merge(&["netsuke", "--config", &config])?;

    ensure!(
        cli.env_access_policy().evaluate("CI_TOKEN").is_ok(),
        "an explicitly selected operator ancestor should retain its grant"
    );
    ensure!(
        !cli.env_allow_var.iter().any(|name| name == "PROJECT_TOKEN"),
        "an explicit selector should bypass automatic project discovery"
    );
    Ok(())
}

/// Match cached layer replay to standalone and child-process discovery.
#[test]
fn cached_merge_replay_matches_project_chain_merge() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    write_config(
        sandbox.project_path().join(".netsuke.toml"),
        "extends = \"base.toml\"\n",
    )?;
    write_config(
        sandbox.project_path().join("base.toml"),
        "env_allow_var = [\"CI_TOKEN\"]\n",
    )?;
    let directory = sandbox
        .project_path()
        .to_str()
        .context("project path is UTF-8")?;
    let args = [
        "netsuke",
        "--directory",
        directory,
        "--env-allow-var",
        "SAFE",
    ];
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (parsed, matches) = cli::parse_with_localizer_from(args, &localizer)?;
    let environment = ConfigEnvironment(sandbox.environment.clone());
    let (json, outcome) =
        cli::resolve_json_and_layers_outcome_with_env(&parsed, &matches, &environment);
    json?;
    let cached =
        cli::merge_with_cached_file_layers(&parsed, &matches, &environment, outcome.into_layers())?;
    let standalone = cli::merge_with_config_and_env(&parsed, &matches, &environment)?;
    let child = sandbox.merge_auto(&["--env-allow-var", "SAFE"])?;

    ensure!(
        cached.env_access_policy() == standalone.env_access_policy()
            && cached.env_access_policy() == child.env_access_policy(),
        "cached replay, standalone merge, and isolated child merge must agree"
    );
    Ok(())
}
