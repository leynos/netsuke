//! End-to-end configuration precedence ladder tests.
//!
//! These tests exercise the complete implemented precedence ladder from the
//! lowest layer to the highest for scalar fields:
//!
//! ```text
//! CLI flags            (--file, --emoji, --jobs, --locale, --color)
//! Environment          (NETSUKE_FILE, NETSUKE_EMOJI, ...)
//! Project scope        (.netsuke.toml, appended on top of discovery)
//! Discovered scope     (XDG_CONFIG_HOME > XDG_CONFIG_DIRS > $HOME dotfile)
//! Defaults             (CliConfig::default)
//! ```
//!
//! The profile rung (`--profile`) is not implemented yet; it is tracked by
//! roadmap item 5.3.1 and will sit between the project scope and the
//! environment layer. No test asserts unimplemented profile behaviour.
//!
//! The environment selector set is closed at `NETSUKE_CONFIG` (ADR-004).
//! [`NETSUKE_CONFIG_PATH`][2] is never set as a selector here; a guard test
//! confirms it has no effect on selection.
//!
//! [2]: https://github.com/leynos/netsuke/blob/main/docs/adr-004-explicit-config-selection-outside-orthoconfig.md

use super::merge_probe::{environment_with_system_scope, merge_in_child};
use anyhow::{Context, Result, ensure};
use netsuke::cli::{Cli, EmojiPolicy, config::ColourPolicy};
use rstest::rstest;
use std::ffi::OsString;
use std::path::Path;
use tempfile::tempdir;
use test_support::fs as test_fs;

/// Build the full ladder environment with a distinct value per layer.
///
/// Layer-to-value mapping (all for the same field set):
/// - system scope: `XDG_CONFIG_DIRS/netsuke/config.toml`
/// - user scope: `XDG_CONFIG_HOME/netsuke/config.toml`
/// - project scope: `project/.netsuke.toml`
/// - environment: `NETSUKE_*` overrides
///
/// Returns the environment vector (caller keeps the `TempDir`s alive).
fn ladder_environment(
    project: &Path,
    home: &Path,
    system: &Path,
    enabled_layers: &[Layer],
) -> Result<Vec<(OsString, OsString)>> {
    // System scope (lowest discovered layer).
    if enabled_layers.contains(&Layer::System) {
        write_scope_config(system.join("netsuke").join("config.toml"), SYSTEM_CONFIG)?;
    }
    // User scope.
    if enabled_layers.contains(&Layer::User) {
        let user_config = home.join(".config").join("netsuke").join("config.toml");
        write_scope_config(user_config, USER_CONFIG)?;
    }
    // Project scope.
    if enabled_layers.contains(&Layer::Project) {
        write_scope_config(project.join(".netsuke.toml"), PROJECT_CONFIG)?;
    }

    let mut environment = environment_with_system_scope(
        home,
        system,
        &[(OsString::from("NETSUKE_FILE"), OsString::from("Envfile"))],
    );
    environment.extend([
        (OsString::from("NETSUKE_EMOJI"), OsString::from("never")),
        (OsString::from("NETSUKE_JOBS"), OsString::from("16")),
        (OsString::from("NETSUKE_LOCALE"), OsString::from("it-IT")),
        (OsString::from("NETSUKE_COLOR"), OsString::from("never")),
    ]);
    if !enabled_layers.contains(&Layer::Environment) {
        // Clear the NETSUKE_* value layer so it does not participate.
        environment.retain(|(key, _)| !key.to_string_lossy().starts_with("NETSUKE_"));
    }
    Ok(environment)
}

/// Write a per-scope configuration file, creating parent directories first.
fn write_scope_config(path: impl AsRef<Path>, contents: &'static str) -> Result<()> {
    let target = path.as_ref();
    test_fs::create_dir_all(
        target
            .parent()
            .context("config file has no parent directory")?,
    )
    .context("create config directory")?;
    test_fs::write(target, contents).with_context(|| format!("write {}", target.display()))?;
    Ok(())
}

/// System-scope (lowest discovered) configuration values.
const SYSTEM_CONFIG: &str = r#"
file = "Systemfile"
emoji = "always"
jobs = 9
locale = "de-DE"
color = "always"
"#;

/// User-scope configuration values.
const USER_CONFIG: &str = r#"
file = "Userfile"
emoji = "never"
jobs = 4
locale = "fr-FR"
color = "never"
"#;

/// Project-scope configuration values.
const PROJECT_CONFIG: &str = r#"
file = "Projectfile"
emoji = "never"
jobs = 8
locale = "es-ES"
color = "never"
"#;

/// The discovered or injected layers the ladder test may seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    System,
    User,
    Project,
    Environment,
}

/// Expected merged values for one ladder rung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LadderExpectation {
    file: &'static str,
    emoji: EmojiPolicy,
    jobs: Option<usize>,
    locale: Option<&'static str>,
    color: ColourPolicy,
}

impl LadderExpectation {
    const fn defaults() -> Self {
        Self {
            file: "Netsukefile",
            emoji: EmojiPolicy::Auto,
            jobs: None,
            locale: None,
            color: ColourPolicy::Auto,
        }
    }
}

fn assert_ladder(merged: &Cli, expected: LadderExpectation) -> Result<()> {
    ensure!(
        merged.file.as_path() == Path::new(expected.file),
        "manifest path should be {:?}, got {}",
        expected.file,
        merged.file.display()
    );
    ensure!(
        merged.emoji == expected.emoji,
        "emoji policy should be {:?}, got {:?}",
        expected.emoji,
        merged.emoji
    );
    ensure!(
        merged.jobs == expected.jobs,
        "jobs should be {:?}, got {:?}",
        expected.jobs,
        merged.jobs
    );
    ensure!(
        merged.locale.as_deref() == expected.locale,
        "locale should be {:?}, got {:?}",
        expected.locale,
        merged.locale
    );
    ensure!(
        merged.color == expected.color,
        "color policy should be {:?}, got {:?}",
        expected.color,
        merged.color
    );
    Ok(())
}

/// Merge a ladder scenario with the given CLI/extra env layer enabled and
/// return the merged CLI.
fn run_ladder_scenario(
    enabled_layers: &[Layer],
    cli_args: &[&str],
    extra_env: &[(OsString, OsString)],
) -> Result<Cli> {
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create home directory")?;
    let system = tempdir().context("create system directory")?;
    let mut environment =
        ladder_environment(project.path(), home.path(), system.path(), enabled_layers)?;
    environment.extend_from_slice(extra_env);
    merge_in_child(cli_args, project.path(), &environment)
}

/// Cases where the environment layer alone supplies the value for each rung.
#[rstest]
#[case::defaults_only(&[], &["netsuke"], &[], LadderExpectation::defaults())]
#[case::system_only(
    &[Layer::System],
    &["netsuke"],
    &[],
    LadderExpectation {
        file: "Systemfile",
        emoji: EmojiPolicy::Always,
        jobs: Some(9),
        locale: Some("de-DE"),
        color: ColourPolicy::Always,
    }
)]
#[case::user_overrides_system(
    &[Layer::System, Layer::User],
    &["netsuke"],
    &[],
    LadderExpectation {
        file: "Userfile",
        emoji: EmojiPolicy::Never,
        jobs: Some(4),
        locale: Some("fr-FR"),
        color: ColourPolicy::Never,
    }
)]
#[case::project_overrides_user_and_system(
    &[Layer::System, Layer::User, Layer::Project],
    &["netsuke"],
    &[],
    LadderExpectation {
        file: "Projectfile",
        emoji: EmojiPolicy::Never,
        jobs: Some(8),
        locale: Some("es-ES"),
        color: ColourPolicy::Never,
    }
)]
#[case::environment_overrides_project(
    &[Layer::System, Layer::User, Layer::Project, Layer::Environment],
    &["netsuke"],
    &[],
    LadderExpectation {
        file: "Envfile",
        emoji: EmojiPolicy::Never,
        jobs: Some(16),
        locale: Some("it-IT"),
        color: ColourPolicy::Never,
    }
)]
#[case::cli_overrides_everything(
    &[Layer::System, Layer::User, Layer::Project, Layer::Environment],
    &[
        "netsuke",
        "--file",
        "CliFile",
        "--emoji",
        "always",
        "--jobs",
        "1",
        "--locale",
        "ja-JP",
        "--color",
        "always",
    ],
    &[],
    LadderExpectation {
        file: "CliFile",
        emoji: EmojiPolicy::Always,
        jobs: Some(1),
        locale: Some("ja-JP"),
        color: ColourPolicy::Always,
    }
)]
fn config_ladder_seeds_every_layer_and_winner_follows_precedence(
    #[case] enabled_layers: &[Layer],
    #[case] cli_args: &[&str],
    #[case] extra_env: &[(OsString, OsString)],
    #[case] expected: LadderExpectation,
) -> Result<()> {
    let merged = run_ladder_scenario(enabled_layers, cli_args, extra_env)?;
    assert_ladder(&merged, expected)
}

/// Merge a scenario with a staged (enabled) set of layers and assert the
/// winner follows the ladder for every rung that participates.
#[rstest]
#[case::only_system(&[Layer::System], "Systemfile")]
#[case::only_user(&[Layer::User], "Userfile")]
#[case::only_project(&[Layer::Project], "Projectfile")]
#[case::only_environment(&[Layer::Environment], "Envfile")]
fn each_rung_wins_over_all_lower_rungs_when_alone(
    #[case] enabled_layers: &[Layer],
    #[case] expected_file: &str,
) -> Result<()> {
    let merged = run_ladder_scenario(enabled_layers, &["netsuke"], &[])?;
    ensure!(
        merged.file.as_path() == Path::new(expected_file),
        "the only enabled rung should win over all lower rungs, got {:?}",
        merged.file
    );
    Ok(())
}

/// The removed `NETSUKE_CONFIG_PATH` alias is not a selector: setting it alone
/// must not select a configuration file, and automatic discovery still runs.
#[rstest]
fn netsuke_config_path_alias_is_not_a_selector_and_discovery_continues() -> Result<()> {
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create home directory")?;
    let system = tempdir().context("create system directory")?;

    // Seed a system-scope file and point the legacy alias at a real, existing
    // file carrying values distinct from the system scope. A legacy alias that
    // were ever (incorrectly) treated as a selector would read this file and
    // override the discovered system scope.
    let mut environment =
        ladder_environment(project.path(), home.path(), system.path(), &[Layer::System])?;
    let legacy = tempdir().context("create legacy config directory")?;
    test_fs::write(
        legacy.path().join("legacy-config.toml"),
        r#"
file = "Legacyfile"
emoji = "never"
jobs = 1
locale = "en-US"
color = "never"
"#,
    )
    .context("write legacy config")?;
    environment.push((
        OsString::from("NETSUKE_CONFIG_PATH"),
        legacy.path().join("legacy-config.toml").into_os_string(),
    ));
    let merged = merge_in_child(&["netsuke"], project.path(), &environment)?;

    // Discovery still finds the system file; the legacy alias is ignored.
    ensure!(
        merged.file.as_path() == Path::new("Systemfile"),
        "legacy NETSUKE_CONFIG_PATH must not select a config file, got {:?}",
        merged.file
    );
    ensure!(
        merged.jobs == Some(9),
        "automatic discovery should continue when the legacy alias is set"
    );
    Ok(())
}

/// The profile rung is deferred to roadmap item 5.3.1 and does not exist yet.
/// This marker documents where it will sit and must never assert unimplemented
/// behaviour; it only pins the currently-implemented ladder around the gap.
#[rstest]
fn profile_rung_is_deferred_and_does_not_affect_current_ladder() -> Result<()> {
    // Today the implemented rungs are CLI > env > project > discovered >
    // defaults. When `--profile` lands (5.3.1) it sits between project and
    // environment; update this test then. For now the highest stable rung is
    // the CLI, so a full layered run still resolves to the CLI values.
    let project = tempdir().context("create project directory")?;
    let home = tempdir().context("create home directory")?;
    let system = tempdir().context("create system directory")?;
    let environment = ladder_environment(
        project.path(),
        home.path(),
        system.path(),
        &[
            Layer::System,
            Layer::User,
            Layer::Project,
            Layer::Environment,
        ],
    )?;
    let merged = merge_in_child(
        &["netsuke", "--jobs", "2", "--emoji", "always"],
        project.path(),
        &environment,
    )?;
    ensure!(
        merged.jobs == Some(2),
        "CLI jobs should win over every config and env layer while profile is deferred"
    );
    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "CLI emoji should win over every config and env layer while profile is deferred"
    );
    ensure!(
        merged.file.as_path() == Path::new("Envfile"),
        "environment manifest path should still merge through when the CLI does not set it"
    );
    Ok(())
}
