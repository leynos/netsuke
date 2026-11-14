//! Step definitions for trusted configured shell definitions.
//!
//! These scenarios observe the boundary between operator configuration and
//! every untrusted source. Only the append-order scenario merges in process,
//! because no public command reports the resolved registry yet; the authority
//! scenarios run the real binary so a rejection is proven to stop startup
//! before any manifest is compiled.

use crate::bdd::fixtures::TestWorld;
use crate::bdd::helpers::env_mutation::mutate_env_var;
use crate::bdd::steps::manifest_command::manifest_command_helpers::run_netsuke_and_store;
use crate::bdd::types::EnvVarKey;
use anyhow::{Context, Result, ensure};
use netsuke::cli::CliConfig;
use netsuke::cli::config::shells::ShellDefinition;
use ortho_config::{MergeComposer, load_config_file_as_chain, sanitize_value};
use rstest_bdd_macros::{given, then, when};
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use test_support::fs as test_fs;

/// Name the file `generate` writes only when manifest compilation succeeds.
const SENTINEL: &str = "sentinel.ninja";

/// Return the scenario workspace, creating it when no step has yet.
fn ensure_workspace(world: &TestWorld) -> Result<PathBuf> {
    if world.temp_dir.borrow().is_none() {
        let temp = tempdir().context("create temporary workspace")?;
        *world.temp_dir.borrow_mut() = Some(temp);
    }
    workspace_path(world)
}

/// Return the scenario workspace path without creating it.
fn workspace_path(world: &TestWorld) -> Result<PathBuf> {
    world
        .temp_dir
        .borrow()
        .as_ref()
        .map(|temp| temp.path().to_path_buf())
        .context("temporary workspace should be initialised")
}

/// Write `contents` to `relative` beneath the scenario workspace.
fn write_workspace_file(world: &TestWorld, relative: &str, contents: &str) -> Result<PathBuf> {
    let path = ensure_workspace(world)?.join(relative);
    if let Some(parent) = path.parent() {
        test_fs::create_dir_all(parent)
            .with_context(|| format!("create directory {}", parent.display()))?;
    }
    test_fs::write(&path, contents).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// Copy the harmless manifest so a rejection cannot be blamed on a missing one.
fn ensure_valid_manifest(world: &TestWorld) -> Result<()> {
    let destination = ensure_workspace(world)?.join("Netsukefile");
    test_fs::copy("tests/data/minimal.yml", &destination)
        .with_context(|| format!("copy minimal manifest to {}", destination.display()))?;
    Ok(())
}

/// Point every configuration root at the scenario workspace.
///
/// The child process starts with a cleared environment, so an unset `HOME`
/// would let the host's own configuration participate. `HOME` deliberately
/// names a `home` subdirectory rather than the workspace root: the project
/// file lives at the root, and keeping the two apart means no layer is
/// ambiguous about which root discovered it.
fn isolate_configuration_roots(world: &TestWorld) -> Result<()> {
    let root = ensure_workspace(world)?;
    let home = root.join("home").to_string_lossy().into_owned();
    let user = root.join("user").to_string_lossy().into_owned();
    let system = root.join("system").to_string_lossy().into_owned();
    mutate_env_var(world, EnvVarKey::new("HOME"), Some(home.as_str()))?;
    mutate_env_var(
        world,
        EnvVarKey::new("XDG_CONFIG_HOME"),
        Some(user.as_str()),
    )?;
    mutate_env_var(
        world,
        EnvVarKey::new("XDG_CONFIG_DIRS"),
        Some(system.as_str()),
    )?;
    Ok(())
}

/// Render one definition whose fixed arguments name the shell they belong to.
///
/// Deriving the executable and arguments from the name keeps the expectation
/// in the assertion identical to the declaration, so a reordering or a dropped
/// argument is visible without a second table to keep in step.
fn definition_body(name: &str) -> String {
    format!(
        "[[shells]]\nname = \"{name}\"\nexecutable = \"{name}-bin\"\nargs = [\"--tag\", \"{name}\"]\n"
    )
}

/// Record the declaration order the merged registry must preserve.
fn record_definition(world: &TestWorld, name: &str) {
    world.shell_order.borrow_mut().push(name.to_owned());
}

/// Merge the operator chain rooted at `path` through the production schema.
fn merge_operator_chain(path: &Path) -> Result<Vec<ShellDefinition>> {
    let chain = load_config_file_as_chain(path)
        .with_context(|| format!("load configuration chain for {}", path.display()))?
        .with_context(|| format!("{} should declare a configuration chain", path.display()))?;
    let mut composer = MergeComposer::with_capacity(chain.values.len() + 1);
    composer.push_defaults(sanitize_value(&CliConfig::default())?);
    for (value, source) in chain.values {
        composer.push_file(value, Some(source));
    }
    let merged = CliConfig::merge_from_layers(composer.layers())
        .context("merge operator configuration layers")?;
    Ok(merged.shells)
}

#[given("a system configuration defining shell {name:string}")]
fn system_configuration_defines_shell(world: &TestWorld, name: String) -> Result<()> {
    write_workspace_file(world, "system/netsuke/config.toml", &definition_body(&name))?;
    record_definition(world, &name);
    Ok(())
}

#[given("the selected user configuration extends that system configuration")]
fn user_configuration_extends_system(world: &TestWorld) -> Result<()> {
    let user = write_workspace_file(
        world,
        "user/netsuke/config.toml",
        "extends = \"../../system/netsuke/config.toml\"\n",
    )?;
    world.shell_chain.set(user);
    Ok(())
}

#[given("that user configuration defines shell {name:string}")]
fn user_configuration_defines_shell(world: &TestWorld, name: String) -> Result<()> {
    let path = world
        .shell_chain
        .get()
        .context("the selected user configuration should be recorded")?;
    let mut contents =
        test_fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    contents.push_str(&definition_body(&name));
    test_fs::write(&path, contents).with_context(|| format!("write {}", path.display()))?;
    record_definition(world, &name);
    Ok(())
}

#[given("a project configuration containing a shell definition")]
fn project_configuration_contains_shell(world: &TestWorld) -> Result<()> {
    isolate_configuration_roots(world)?;
    ensure_valid_manifest(world)?;
    write_workspace_file(world, ".netsuke.toml", &definition_body("dash"))?;
    Ok(())
}

#[given("an otherwise untrusted file defining shell {name:string}")]
fn untrusted_file_defines_shell(world: &TestWorld, name: String) -> Result<()> {
    isolate_configuration_roots(world)?;
    ensure_valid_manifest(world)?;
    let path = write_workspace_file(world, "untrusted.toml", &definition_body(&name))?;
    world.shell_untrusted_file.set(path);
    Ok(())
}

#[given("two trusted layers defining shell {name:string}")]
fn two_trusted_layers_define_shell(world: &TestWorld, name: String) -> Result<()> {
    isolate_configuration_roots(world)?;
    ensure_valid_manifest(world)?;
    write_workspace_file(world, "user/layer.toml", &definition_body(&name))?;
    write_workspace_file(
        world,
        "user/netsuke/config.toml",
        &format!("extends = \"../layer.toml\"\n{}", definition_body(&name)),
    )?;
    Ok(())
}

#[when("Netsuke loads its configuration")]
fn netsuke_loads_configuration(world: &TestWorld) -> Result<()> {
    if let Some(path) = world.shell_chain.get() {
        let outcome = merge_operator_chain(&path).map_err(|error| format!("{error:#}"));
        world.shell_merge.set(outcome);
        return Ok(());
    }
    run_netsuke_and_store(world, &["generate", "--output", SENTINEL])
}

#[when("that file is selected with --config")]
fn that_file_is_selected(world: &TestWorld) -> Result<()> {
    let recorded = world
        .shell_untrusted_file
        .get()
        .context("the untrusted file should be recorded")?;
    let selected = recorded.to_string_lossy().into_owned();
    run_netsuke_and_store(
        world,
        &["--config", &selected, "generate", "--output", SENTINEL],
    )
}

#[then("both definitions retain their fixed arguments in layer order")]
fn definitions_retain_fixed_arguments(world: &TestWorld) -> Result<()> {
    let merged = world
        .shell_merge
        .get()
        .context("configuration load should be recorded")?
        .map_err(|error| anyhow::anyhow!("configuration load failed: {error}"))?;
    let expected = world.shell_order.borrow().clone();
    ensure!(
        merged.len() == expected.len(),
        "expected {} definitions, got {}",
        expected.len(),
        merged.len()
    );
    for (definition, name) in merged.iter().zip(&expected) {
        ensure!(
            definition.name.as_str() == name,
            "expected shell {name:?}, got {:?}",
            definition.name.as_str()
        );
        ensure!(
            definition.executable.to_string() == format!("{name}-bin"),
            "shell {name:?} should keep its executable, got {}",
            definition.executable
        );
        let expected_args = ["--tag".to_owned(), name.clone()];
        ensure!(
            definition.args.as_slice() == expected_args.as_slice(),
            "shell {name:?} should keep its fixed arguments, got {:?}",
            definition.args.as_slice()
        );
    }
    Ok(())
}

#[then("loading fails at configuration key {key:string}")]
fn loading_fails_at_key(world: &TestWorld, key: String) -> Result<()> {
    let succeeded = world
        .run_status
        .get()
        .context("netsuke should have been run")?;
    ensure!(!succeeded, "expected loading to fail, but it succeeded");
    let stderr = world.command_stderr.get().unwrap_or_default();
    ensure!(
        stderr.contains("Validation failed for") && stderr.contains(&key),
        "failure should report a validation fault for {key:?}, got: {stderr}"
    );
    Ok(())
}

#[then("no manifest is compiled")]
fn no_manifest_is_compiled(world: &TestWorld) -> Result<()> {
    let sentinel = workspace_path(world)?.join(SENTINEL);
    ensure!(
        !sentinel.exists(),
        "manifest compilation should not have run, but {} exists",
        sentinel.display()
    );
    Ok(())
}

#[then("loading fails at the duplicate shell name")]
fn loading_fails_at_duplicate_name(world: &TestWorld) -> Result<()> {
    let succeeded = world
        .run_status
        .get()
        .context("netsuke should have been run")?;
    ensure!(!succeeded, "expected loading to fail, but it succeeded");
    let stderr = world.command_stderr.get().unwrap_or_default();
    ensure!(
        stderr.contains("shells[1]") && stderr.contains("shells[0]"),
        "failure should name both duplicate positions, got: {stderr}"
    );
    ensure!(
        stderr.contains("already declares"),
        "failure should explain the collision, got: {stderr}"
    );
    Ok(())
}
