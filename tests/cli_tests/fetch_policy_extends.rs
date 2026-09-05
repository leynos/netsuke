//! Exercise the fetch-policy trust boundary across complete project chains.

use super::merge_probe::{environment_with_system_scope, merge_in_child};
use anyhow::{Context, Result, ensure};
use netsuke::cli::Cli;
use netsuke::stdlib::NetworkPolicyViolation;
use rstest::{fixture, rstest};
use std::ffi::OsString;
use std::path::Path;
use tempfile::{TempDir, tempdir};
use test_support::fs as test_fs;
use url::Url;

#[path = "fetch_policy_operator_extends.rs"]
mod operator_extends;

const OPERATOR_POLICY: &str = concat!(
    "fetch_default_deny = true\n",
    "fetch_allow_host = [\"downloads.example.org\"]\n",
);
const PROJECT_GRANTS: &str = concat!(
    "fetch_default_deny = false\n",
    "fetch_allow_scheme = [\"http\"]\n",
    "fetch_allow_host = [\"169.254.169.254\"]\n",
);

/// Own isolated scope directories for a real configuration merge.
struct ChainFixture {
    project: TempDir,
    home: TempDir,
    system: TempDir,
}

/// Write one fixture configuration and create any required parent directories.
fn write_config(config_path: impl AsRef<Path>, contents: &str) -> Result<()> {
    let path = config_path.as_ref();
    test_fs::create_dir_all(path.parent().context("configuration parent")?)?;
    test_fs::write(path, contents).context("write scope configuration")
}

/// Create independently owned system, user, and project scopes.
#[fixture]
fn chain_fixture() -> Result<ChainFixture> {
    Ok(ChainFixture {
        project: tempdir()?,
        home: tempdir()?,
        system: tempdir()?,
    })
}

impl ChainFixture {
    /// Write a primary file and optionally nest its policy dependency.
    fn write_chain(&self, primary: &str, policy: &str, nested: bool) -> Result<()> {
        write_config(
            self.project.path().join(".netsuke.toml"),
            &format!("extends = \"policy.toml\"\n{primary}"),
        )?;
        if nested {
            write_config(
                self.project.path().join("policy.toml"),
                "extends = \"nested/leaf.toml\"\n",
            )?;
            write_config(self.project.path().join("nested/leaf.toml"), policy)
        } else {
            write_config(self.project.path().join("policy.toml"), policy)
        }
    }

    /// Write trusted user configuration.
    fn write_user(&self, contents: &str) -> Result<()> {
        write_config(
            self.home.path().join(".config/netsuke/config.toml"),
            contents,
        )
    }

    /// Include an explicit operator dependency without changing root discovery.
    fn write_user_extending(&self, dependency: &Path, contents: &str) -> Result<()> {
        let quoted_path = serde_json::to_string(dependency)?;
        self.write_user(&format!("extends = {quoted_path}\n{contents}"))
    }

    /// Merge the scope fixture in a child process without changing harness state.
    fn merge(&self, environment: &[(&str, &str)], args: &[&str]) -> Result<Cli> {
        let overrides: Vec<_> = environment
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value)))
            .collect();
        let child_environment =
            environment_with_system_scope(self.home.path(), self.system.path(), &overrides);
        merge_in_child(args, self.project.path(), &child_environment)
    }
}

/// Evaluate a URL through the effective runtime policy.
fn evaluate(cli: &Cli, value: &str) -> Result<Result<(), NetworkPolicyViolation>> {
    Ok(cli.network_policy()?.evaluate(&Url::parse(value)?))
}

#[rstest]
#[case::direct(false)]
#[case::nested(true)]
fn extended_grants_cannot_weaken_user_policy(
    chain_fixture: Result<ChainFixture>,
    #[case] nested: bool,
) -> Result<()> {
    let fixture = chain_fixture?;
    fixture.write_user(OPERATOR_POLICY)?;
    fixture.write_chain("", PROJECT_GRANTS, nested)?;
    let merged = fixture.merge(&[], &["netsuke"])?;
    ensure!(
        merged.fetch_default_deny,
        "extended false cleared user deny"
    );
    ensure!(evaluate(&merged, "https://downloads.example.org")?.is_ok());
    ensure!(matches!(
        evaluate(&merged, "https://169.254.169.254")?,
        Err(NetworkPolicyViolation::HostNotAllowlisted { .. })
    ));
    ensure!(matches!(
        evaluate(&merged, "http://downloads.example.org")?,
        Err(NetworkPolicyViolation::SchemeNotAllowed { .. })
    ));
    Ok(())
}

#[rstest]
#[case::direct(false)]
#[case::nested(true)]
fn extended_default_deny_tightens_even_when_primary_requests_false(
    chain_fixture: Result<ChainFixture>,
    #[case] nested: bool,
) -> Result<()> {
    let fixture = chain_fixture?;
    fixture.write_chain(
        "fetch_default_deny = false",
        "fetch_default_deny = true",
        nested,
    )?;
    let merged = fixture.merge(&[], &["netsuke"])?;
    ensure!(matches!(
        evaluate(&merged, "https://downloads.example.org")?,
        Err(NetworkPolicyViolation::HostNotAllowlisted { .. })
    ));
    Ok(())
}

/// Identify the operator source that authorizes a project chain.
#[derive(Clone, Copy)]
enum OperatorScope {
    System,
    User,
    Environment,
    Cli,
}

#[rstest]
#[case::system(OperatorScope::System)]
#[case::user(OperatorScope::User)]
#[case::environment(OperatorScope::Environment)]
#[case::cli(OperatorScope::Cli)]
fn trusted_operator_can_authorize_all_project_chain_grants(
    chain_fixture: Result<ChainFixture>,
    #[case] scope: OperatorScope,
) -> Result<()> {
    let fixture = chain_fixture?;
    fixture.write_user(OPERATOR_POLICY)?;
    fixture.write_chain(
        concat!(
            "fetch_default_deny = true\n",
            "fetch_allow_host = [\"primary.example.org\"]",
        ),
        PROJECT_GRANTS,
        true,
    )?;
    let mut environment = Vec::new();
    let mut args = vec!["netsuke"];
    match scope {
        OperatorScope::System => {
            let system_config = fixture.system.path().join("netsuke/config.toml");
            write_config(&system_config, "trust_project_fetch_policy = true")?;
            fixture.write_user_extending(&system_config, OPERATOR_POLICY)?;
        }
        OperatorScope::User => fixture.write_user(&format!(
            "{OPERATOR_POLICY}trust_project_fetch_policy = true"
        ))?,
        OperatorScope::Environment => {
            environment.push(("NETSUKE_TRUST_PROJECT_FETCH_POLICY", "true"));
        }
        OperatorScope::Cli => args.push("--trust-project-fetch-policy"),
    }
    let merged = fixture.merge(&environment, &args)?;
    ensure!(
        merged.fetch_default_deny,
        "primary deny should override leaf false"
    );
    for host in ["169.254.169.254", "primary.example.org"] {
        ensure!(evaluate(&merged, &format!("http://{host}"))?.is_ok());
    }
    ensure!(matches!(
        evaluate(&merged, "http://unlisted.example.org")?,
        Err(NetworkPolicyViolation::HostNotAllowlisted { .. })
    ));
    Ok(())
}

#[rstest]
#[case::primary("trust_project_fetch_policy = true", PROJECT_GRANTS)]
#[case::extended("", concat!("trust_project_fetch_policy = true\n", "fetch_allow_scheme = [\"http\"]\n", "fetch_allow_host = [\"169.254.169.254\"]\n"))]
fn project_chain_cannot_authorize_itself(
    chain_fixture: Result<ChainFixture>,
    #[case] primary: &str,
    #[case] extended: &str,
) -> Result<()> {
    let fixture = chain_fixture?;
    fixture.write_user(OPERATOR_POLICY)?;
    fixture.write_chain(primary, extended, true)?;
    let merged = fixture.merge(&[], &["netsuke"])?;
    ensure!(!merged.trust_project_fetch_policy);
    ensure!(matches!(
        evaluate(&merged, "https://169.254.169.254")?,
        Err(NetworkPolicyViolation::HostNotAllowlisted { .. })
    ));
    ensure!(matches!(
        evaluate(&merged, "http://downloads.example.org")?,
        Err(NetworkPolicyViolation::SchemeNotAllowed { .. })
    ));
    Ok(())
}

#[rstest]
fn blocklists_accumulate_across_every_scope_and_project_dependency(
    chain_fixture: Result<ChainFixture>,
) -> Result<()> {
    let fixture = chain_fixture?;
    write_config(
        fixture.system.path().join("netsuke/config.toml"),
        "fetch_block_host = [\"system.example.org\"]",
    )?;
    fixture.write_user_extending(
        &fixture.system.path().join("netsuke/config.toml"),
        "fetch_block_host = [\"user.example.org\"]",
    )?;
    fixture.write_chain(
        "fetch_block_host = [\"primary.example.org\"]",
        "fetch_block_host = [\"leaf.example.org\"]",
        true,
    )?;
    write_config(
        fixture.project.path().join("policy.toml"),
        concat!(
            "extends = \"nested/leaf.toml\"\n",
            "fetch_block_host = [\"middle.example.org\"]",
        ),
    )?;
    let merged = fixture.merge(
        &[("NETSUKE_FETCH_BLOCK_HOST", "environment.example.org")],
        &["netsuke", "--fetch-block-host", "cli.example.org"],
    )?;
    for scope in [
        "system",
        "user",
        "primary",
        "middle",
        "leaf",
        "environment",
        "cli",
    ] {
        ensure!(matches!(
            evaluate(&merged, &format!("https://{scope}.example.org"))?,
            Err(NetworkPolicyViolation::HostBlocked { .. })
        ));
    }
    Ok(())
}

#[rstest]
#[case::default_deny("fetch_default_deny", "\"true\"")]
#[case::scheme("fetch_allow_scheme", "true")]
#[case::host("fetch_allow_host", "[123]")]
#[case::trust("trust_project_fetch_policy", "\"true\"")]
fn malformed_extended_policy_fields_fail_the_real_merge(
    chain_fixture: Result<ChainFixture>,
    #[case] key: &str,
    #[case] value: &str,
) -> Result<()> {
    let fixture = chain_fixture?;
    fixture.write_chain("", &format!("{key} = {value}"), true)?;
    let error = fixture
        .merge(&[], &["netsuke"])
        .expect_err("malformed extended policy must fail merging");
    ensure!(
        error.to_string().contains(key),
        "missing field context: {error}"
    );
    Ok(())
}

#[rstest]
fn environment_can_revoke_user_authorization_for_project_chain(
    chain_fixture: Result<ChainFixture>,
) -> Result<()> {
    let fixture = chain_fixture?;
    fixture.write_user(&format!(
        "{OPERATOR_POLICY}trust_project_fetch_policy = true"
    ))?;
    fixture.write_chain("", PROJECT_GRANTS, true)?;
    let merged = fixture.merge(
        &[("NETSUKE_TRUST_PROJECT_FETCH_POLICY", "false")],
        &["netsuke"],
    )?;
    ensure!(!merged.trust_project_fetch_policy);
    ensure!(matches!(
        evaluate(&merged, "https://169.254.169.254")?,
        Err(NetworkPolicyViolation::HostNotAllowlisted { .. })
    ));
    Ok(())
}

#[rstest]
#[case::extended_value("", "fetch_default_deny = false")]
#[case::primary_override("fetch_default_deny = false", "fetch_default_deny = true")]
fn trusted_chain_can_deliberately_override_operator_default_deny(
    chain_fixture: Result<ChainFixture>,
    #[case] primary: &str,
    #[case] extended: &str,
) -> Result<()> {
    let fixture = chain_fixture?;
    fixture.write_user(OPERATOR_POLICY)?;
    fixture.write_chain(primary, extended, true)?;
    let merged = fixture.merge(&[], &["netsuke", "--trust-project-fetch-policy"])?;
    ensure!(!merged.fetch_default_deny);
    ensure!(evaluate(&merged, "https://unlisted.example.org")?.is_ok());
    Ok(())
}

#[rstest]
fn shared_dependency_preserves_operator_grants_without_authorizing_project_grants(
    chain_fixture: Result<ChainFixture>,
) -> Result<()> {
    let fixture = chain_fixture?;
    let shared = fixture.home.path().join("shared.toml");
    write_config(
        &shared,
        concat!(
            "fetch_default_deny = true\n",
            "fetch_allow_scheme = [\"http\"]\n",
            "fetch_allow_host = [\"shared.example.org\"]\n",
        ),
    )?;
    fixture.write_user_extending(&shared, "")?;
    let quoted_path = serde_json::to_string(&shared)?;
    write_config(
        fixture.project.path().join(".netsuke.toml"),
        &format!("extends = {quoted_path}\n{PROJECT_GRANTS}"),
    )?;
    let merged = fixture.merge(&[], &["netsuke"])?;
    ensure!(merged.fetch_default_deny);
    ensure!(evaluate(&merged, "http://shared.example.org")?.is_ok());
    ensure!(matches!(
        evaluate(&merged, "http://169.254.169.254")?,
        Err(NetworkPolicyViolation::HostNotAllowlisted { .. })
    ));
    Ok(())
}

#[rstest]
fn cli_can_reauthorize_project_grants_after_environment_revocation(
    chain_fixture: Result<ChainFixture>,
) -> Result<()> {
    let fixture = chain_fixture?;
    fixture.write_user(&format!(
        "{OPERATOR_POLICY}trust_project_fetch_policy = true"
    ))?;
    fixture.write_chain("fetch_default_deny = true", PROJECT_GRANTS, true)?;
    let merged = fixture.merge(
        &[("NETSUKE_TRUST_PROJECT_FETCH_POLICY", "false")],
        &["netsuke", "--trust-project-fetch-policy"],
    )?;
    ensure!(merged.trust_project_fetch_policy);
    ensure!(evaluate(&merged, "http://169.254.169.254")?.is_ok());
    ensure!(matches!(
        evaluate(&merged, "http://unlisted.example.org")?,
        Err(NetworkPolicyViolation::HostNotAllowlisted { .. })
    ));
    Ok(())
}
