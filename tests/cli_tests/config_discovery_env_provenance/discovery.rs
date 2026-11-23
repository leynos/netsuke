//! Tests for operator, project, and explicit config-chain provenance.

use super::*;
use rstest::rstest;
use tempfile::tempdir;

mod user_project_overlap;

/// Name the primary file a configuration root uses to carry its chain.
#[derive(Clone, Copy)]
enum ChainRoot {
    /// The project file discovered from the working directory.
    Project,
    /// A user-controlled `config.toml`, whether user-scope or explicitly
    /// selected.
    User,
}

impl ChainRoot {
    /// Return the file name that carries the chain in this root.
    const fn file_name(self) -> &'static str {
        match self {
            Self::Project => ".netsuke.toml",
            Self::User => "config.toml",
        }
    }
}

/// Name a TOML literal that reaches the merge as `null` or as an invalid
/// array element.
#[derive(Clone, Copy)]
enum NullLiteral {
    /// A bare `nan`.
    Nan,
    /// A bare `inf`.
    Infinity,
    /// A bare `-inf`.
    NegativeInfinity,
    /// An array holding `nan`.
    ArrayContainingNan,
}

impl NullLiteral {
    /// Return the TOML spelling of this literal.
    const fn toml(self) -> &'static str {
        match self {
            Self::Nan => "nan",
            Self::Infinity => "inf",
            Self::NegativeInfinity => "-inf",
            Self::ArrayContainingNan => "[nan]",
        }
    }
}

/// Name where a malformed project blocklist is written.
#[derive(Clone, Copy)]
enum BlocklistSource {
    /// The primary project file itself.
    PrimaryFile,
    /// An ancestor the project file reaches through `extends`.
    ExtendsAncestor,
}

impl BlocklistSource {
    /// Write `env_block_var = <literal>` into this source's file.
    fn write(self, sandbox: &ConfigSandbox, literal: NullLiteral) -> Result<()> {
        let config = format!("env_block_var = {}\n", literal.toml());
        match self {
            Self::PrimaryFile => {
                write_config(sandbox.project_path().join(".netsuke.toml"), &config)
            }
            Self::ExtendsAncestor => {
                write_extends_chain(sandbox.project_path(), ChainRoot::Project, &config)
            }
        }
    }
}

/// Write a configuration root whose primary file extends `ancestor`.
fn write_extends(root: &Path, chain: ChainRoot, ancestor: &Path) -> Result<()> {
    write_config(root.join(chain.file_name()), &extends(ancestor)?)
}

/// Write a configuration root whose primary file extends a sibling
/// `base.toml` holding `ancestor_contents`.
fn write_extends_chain(root: &Path, chain: ChainRoot, ancestor_contents: &str) -> Result<()> {
    let ancestor = root.join("base.toml");
    write_config(&ancestor, ancestor_contents)?;
    write_extends(root, chain, &ancestor)
}

/// Assert one environment variable name is denied and never read.
fn assert_env_name_blocked(cli: &Cli, name: &str) -> Result<()> {
    let policy = cli.env_access_policy();
    ensure!(
        policy.evaluate(name).is_err(),
        "{name} should be denied by the effective environment policy"
    );
    assert_blocked_without_read(&policy, name)
}

/// Assert a malformed project blocklist fails loading with the field-specific
/// validation error.
///
/// `is_block_effective` describes the earlier block entry. When the merge
/// succeeds, the message reports whether that entry survived, which names the
/// affected operation this regression guards: before the fix a project `nan`
/// reset the accumulated blocklist and silently cleared the entry.
fn assert_project_env_blocklist_rejected(
    merged: Result<Cli>,
    literal: NullLiteral,
    is_block_effective: impl Fn(&Cli) -> bool,
) -> Result<()> {
    let spelling = literal.toml();
    match merged {
        Err(error) => {
            let chain = format!("{error:#}");
            ensure!(
                chain.contains("Validation failed for 'env_block_var'"),
                "a non-finite {spelling} project blocklist should fail with the \
                 env_block_var validation error: {chain}"
            );
            Ok(())
        }
        Ok(cli) => bail!(
            "a non-finite {spelling} project blocklist must stop loading; instead \
             the merge succeeded and the earlier block entry was {}",
            if is_block_effective(&cli) {
                "kept"
            } else {
                "cleared"
            }
        ),
    }
}

/// Retain environment grants from a trusted XDG chain.
#[test]
fn operator_xdg_chain_retains_inherited_environment_grants() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    write_extends_chain(
        &sandbox.user_config_dir(),
        ChainRoot::User,
        "env_allow_var = [\"CI_TOKEN\"]\n",
    )?;
    let cli = sandbox.merge_auto(&[])?;

    ensure!(
        cli.env_allow_var == ["CI_TOKEN"],
        "the operator-chain ancestor grant must remain in the effective allowlist"
    );
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
    write_extends(&sandbox.user_config_dir(), ChainRoot::User, &shared_file)?;
    write_extends(sandbox.project_path(), ChainRoot::Project, &shared_file)?;
    let cli = sandbox.merge_auto(&[])?;

    ensure!(
        cli.env_allow_var == ["CI_TOKEN"],
        "the operator occurrence should retain its grant after project quarantine"
    );
    ensure!(
        cli.env_access_policy().evaluate("CI_TOKEN").is_ok(),
        "operator grant should apply"
    );
    assert_env_name_blocked(&cli, "SAFE")
}

/// Keep a block authoritative over an allow from either chain.
///
/// A block that arrives through the discovered project chain must override a
/// command-line allow, and an operator-chain block must override both its own
/// allow and the command-line one.
#[rstest]
#[case::project_chain_block(ChainRoot::Project, "env_block_var = [\"CI_TOKEN\"]\n")]
#[case::operator_chain_allow_and_block(
    ChainRoot::User,
    "env_allow_var = [\"CI_TOKEN\"]\nenv_block_var = [\"CI_TOKEN\"]\n"
)]
fn block_overrides_allow(#[case] chain: ChainRoot, #[case] config: &str) -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    let root = match chain {
        ChainRoot::Project => sandbox.project_path().to_path_buf(),
        ChainRoot::User => sandbox.user_config_dir(),
    };
    write_extends_chain(&root, chain, config)?;
    let cli = sandbox.merge_auto(&["--env-allow-var", "CI_TOKEN"])?;

    assert_env_name_blocked(&cli, "CI_TOKEN")
}

/// Reject a non-finite project environment blocklist instead of letting the
/// JSON conversion turn it into a `null` that resets the blocklist.
///
/// The ancestor cases reach the policy boundary as project-chain occurrences,
/// which follow project authority even though their physical path is not the
/// selected project file.
#[rstest]
#[case::primary_file_nan(BlocklistSource::PrimaryFile, NullLiteral::Nan)]
#[case::primary_file_infinity(BlocklistSource::PrimaryFile, NullLiteral::Infinity)]
#[case::primary_file_negative_infinity(BlocklistSource::PrimaryFile, NullLiteral::NegativeInfinity)]
#[case::primary_file_array_containing_nan(
    BlocklistSource::PrimaryFile,
    NullLiteral::ArrayContainingNan
)]
#[case::extends_ancestor_nan(BlocklistSource::ExtendsAncestor, NullLiteral::Nan)]
#[case::extends_ancestor_array_containing_nan(
    BlocklistSource::ExtendsAncestor,
    NullLiteral::ArrayContainingNan
)]
fn non_finite_project_env_block_var_is_rejected(
    #[case] source: BlocklistSource,
    #[case] literal: NullLiteral,
) -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    source.write(&sandbox, literal)?;
    let merged = sandbox.merge_auto(&["--env-block-var", "SAFE"]);

    assert_project_env_blocklist_rejected(merged, literal, |cli| {
        cli.env_access_policy().evaluate("SAFE").is_err()
    })
}

/// Reject a non-finite project blocklist before it clears an earlier
/// operator-file block entry.
///
/// The other rejection cases place their block on the command line, which
/// merges after the project files. This one places it in the user-scope
/// operator file, so before the fix the project value reset the accumulated
/// blocklist and `SAFE` became readable again.
#[rstest]
#[case::nan(NullLiteral::Nan)]
#[case::infinity(NullLiteral::Infinity)]
#[case::negative_infinity(NullLiteral::NegativeInfinity)]
fn non_finite_project_blocklist_does_not_clear_operator_file_block(
    #[case] literal: NullLiteral,
) -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    write_config(
        sandbox.user_config_dir().join("config.toml"),
        "env_block_var = [\"SAFE\"]\n",
    )?;
    write_config(
        sandbox.project_path().join(".netsuke.toml"),
        &format!("env_block_var = {}\n", literal.toml()),
    )?;
    let merged = sandbox.merge_auto(&[]);

    assert_project_env_blocklist_rejected(merged, literal, |cli| {
        cli.env_access_policy().evaluate("SAFE").is_err()
    })
}

/// Keep the operator block effective when the project adds an empty
/// environment blocklist or omits the field.
#[rstest]
#[case::empty(Some("env_block_var = []\n"))]
#[case::absent(None)]
fn empty_or_absent_project_env_blocklist_keeps_operator_block(
    #[case] project_config: Option<&str>,
) -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    if let Some(config) = project_config {
        write_config(sandbox.project_path().join(".netsuke.toml"), config)?;
    }
    let cli = sandbox.merge_auto(&["--env-block-var", "SAFE"])?;

    ensure!(
        cli.env_block_var == ["SAFE"],
        "the operator block must remain the effective blocklist"
    );
    assert_env_name_blocked(&cli, "SAFE")
}

/// Give an operator block precedence over an operator-chain grant.
#[test]
fn operator_block_overrides_operator_allow() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    write_extends_chain(
        &sandbox.user_config_dir(),
        ChainRoot::User,
        "env_allow_var = [\"CI_TOKEN\"]\nenv_block_var = [\"CI_TOKEN\"]\n",
    )?;
    let cli = sandbox.merge_auto(&["--env-allow-var", "CI_TOKEN"])?;

    assert_env_name_blocked(&cli, "CI_TOKEN")
}

/// Keep an ancestor grant when the caller explicitly selects its chain.
#[test]
fn explicit_config_chain_retains_ancestor_environment_grants() -> Result<()> {
    let sandbox = ConfigSandbox::new()?;
    let config_dir = sandbox.project_path().join("explicit");
    write_extends_chain(
        &config_dir,
        ChainRoot::User,
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
        cli.env_allow_var == ["CI_TOKEN"],
        "the explicitly selected ancestor grant must remain in the effective allowlist"
    );
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
    write_extends_chain(
        sandbox.project_path(),
        ChainRoot::Project,
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
    let (parsed, matches) =
        cli::parse_with_localizer_from(args, &localizer, clap::ColorChoice::Auto)?;
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
