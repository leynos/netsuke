//! Child-shell default expansion through Ninja.
//!
//! Scalar commands, command lists, and scripts must all preserve a child
//! shell's `${VAR:-fallback}` default after placeholder lowering and Ninja's
//! dollar collapsing. Each case runs the generated file with a real child
//! shell, because the generated text alone cannot show which dollar the shell
//! keeps.

use super::*;

/// Build the optional sentinel variable for an isolated child shell.
fn sentinel_environment(value: Option<&str>) -> Vec<(&str, &str)> {
    value.map_or_else(Vec::new, |sentinel_value| vec![(SENTINEL, sentinel_value)])
}

/// Verify scalar command recipes preserve child-shell defaults after Ninja execution.
#[rstest]
#[case::unset(None, "fallback")]
#[case::set(Some(SENTINEL_VALUE), SENTINEL_VALUE)]
fn shell_default_reaches_the_child_shell(
    #[case] environment_value: Option<&str>,
    #[case] expected: &str,
) -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    command: 'printf %s \"${NETSUKE_TEST_SENTINEL:-fallback}\" > {{ outs }}'\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let environment = sentinel_environment(environment_value);
    let actual = ninja_output(&ninja, &environment, None)?;
    ensure!(
        actual == expected,
        "expected child shell output {expected:?}, got {actual:?}"
    );
    Ok(())
}

/// Verify command-list recipes preserve braced child-shell defaults through Ninja.
#[rstest]
#[case::unset(None, "fallback")]
#[case::set(Some(SENTINEL_VALUE), SENTINEL_VALUE)]
fn command_list_default_reaches_the_child_shell(
    #[case] environment_value: Option<&str>,
    #[case] expected: &str,
) -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    command:\n      - 'printf %s \"${NETSUKE_TEST_SENTINEL:-fallback}\" > {{ outs }}'\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let environment = sentinel_environment(environment_value);
    let actual = ninja_output(&ninja, &environment, None)?;
    ensure!(
        actual == expected,
        "expected command-list child shell output {expected:?}, got {actual:?}"
    );
    Ok(())
}

/// Verify script recipes preserve braced child-shell defaults through Ninja.
#[rstest]
#[case::unset(None, "fallback")]
#[case::set(Some(SENTINEL_VALUE), SENTINEL_VALUE)]
fn script_default_reaches_the_child_shell(
    #[case] environment_value: Option<&str>,
    #[case] expected: &str,
) -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: 'printf %s \"${NETSUKE_TEST_SENTINEL:-fallback}\" > {{ outs }}'\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let environment = sentinel_environment(environment_value);
    let actual = ninja_output(&ninja, &environment, None)?;
    ensure!(
        actual == expected,
        "expected script child shell output {expected:?}, got {actual:?}"
    );
    Ok(())
}
