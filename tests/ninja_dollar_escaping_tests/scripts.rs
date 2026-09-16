//! Script recipe integration cases for Ninja dollar escaping.

use super::*;

#[rstest]
fn script_placeholders_execute_against_real_paths() -> Result<()> {
    assert_script_output(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: in\n    script: \"printf '%s' $in > $out\"\n",
        ("in", "script input"),
        "in",
        "expected script to write the lowered input path \"in\", got",
    )
}

/// Verify double-quoted script placeholders keep shell punctuation inert.
#[cfg(unix)]
#[rstest]
fn script_double_quoted_placeholders_quote_shell_punctuation() -> Result<()> {
    assert_script_output(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: 'foo;id'\n    script: \"printf '%s' \\\"$in\\\" > $out\"\n",
        ("foo;id", "script input"),
        "foo;id",
        "double-quoted script interpolation must not execute shell punctuation",
    )
}

/// Verify single-quoted script placeholders keep shell punctuation inert.
#[cfg(unix)]
#[rstest]
fn script_single_quoted_placeholders_quote_shell_punctuation() -> Result<()> {
    assert_script_output(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: 'foo;id'\n    script: \"printf '%s' '$in' > '$out'\"\n",
        ("foo;id", "script input"),
        "foo;id",
        "single-quoted script interpolation must not execute shell punctuation",
    )
}

/// Verify an escaped script marker still lowers to the declared Ninja output.
#[cfg(unix)]
#[rstest]
fn escaped_script_marker_reaches_the_declared_output() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: 'printf %s escaped > \\{{ outs }}'\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;
    let commands = ninja_commands(&ninja, "out")?;
    ensure!(
        commands.contains("\\out"),
        "the escaped marker must lower to the declared output path:\n{commands}"
    );
    let actual = ninja_output(&ninja, None, None)?;
    ensure!(
        actual == "escaped",
        "expected escaped marker output, got {actual:?}"
    );
    Ok(())
}

/// Verify a double-quoted script marker cannot create a second POSIX command.
#[cfg(unix)]
#[rstest]
#[case::apostrophe("# unmatched apostrophe '")]
#[case::double_quote("# unmatched double quote \"")]
fn quoted_script_marker_cannot_inject_a_command_boundary(#[case] comment: &str) -> Result<()> {
    let output_path = "x\"; touch injected; echo \"y";
    let manifest = manifest::from_str(&format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: '{output_path}'\n    script: |\n      {comment}\n      echo \"{{{{ outs }}}}\"\n"
    ))?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;
    let commands = ninja_commands(&ninja, output_path)?;
    let workspace = tempfile::tempdir()?;
    let output = Command::new("/bin/sh")
        .args(["-ec", &commands])
        .current_dir(workspace.path())
        .output()
        .context("run the generated script command")?;
    ensure!(
        output.status.success(),
        "generated script command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        output.stdout == format!("{output_path}\n").as_bytes(),
        "script marker must remain one argument, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    ensure!(
        !workspace.path().join("injected").exists(),
        "double-quoted marker must not execute the injected touch command"
    );
    Ok(())
}

/// Verify that placeholder-looking text inside backticks is rejected before escaping.
#[rstest]
fn placeholders_inside_backticks_are_rejected_before_backend_escaping() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: in\n    script: 'echo `basename {{ outs }}`'\n",
    )?;
    let result = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix);
    ensure!(
        result.is_err(),
        "a placeholder protected by backticks must not silently reach the shell"
    );
    Ok(())
}

/// Verify that command control characters cannot inject Ninja syntax.
#[rstest]
#[case::newline("echo safe\nbuild injected: action")]
#[case::carriage_return("echo safe\rbuild injected: action")]
fn command_control_characters_are_rejected(#[case] command: &str) -> Result<()> {
    let result = generate_posix(&graph(
        Recipe::Command {
            command: command.into(),
        },
        "in",
        "out",
    )?);
    ensure!(
        result.is_err(),
        "unsafe control characters must not reach a generated Ninja binding"
    );
    Ok(())
}

/// Verify that Ninja-unsafe path characters are rejected during generation.
#[rstest]
#[case::dollar("input$file")]
#[case::colon("input:file")]
#[case::pipe("input|file")]
#[case::tab("input\tfile")]
#[case::nul("input\0file")]
#[case::carriage_return("input\rfile")]
#[case::newline("input\nfile")]
fn unsafe_paths_are_rejected(#[case] input: &str) -> Result<()> {
    let result = generate_posix(&graph(
        Recipe::Command {
            command: format!("cat {INS_TOKEN} > {OUTS_TOKEN}").into(),
        },
        input,
        "out",
    )?);
    ensure!(
        result.is_err(),
        "a Ninja-unsupported path must fail generation rather than corrupt an edge"
    );
    Ok(())
}

/// Verify that commands without dollars retain their exact text.
#[rstest]
fn dollar_free_commands_remain_byte_identical() -> Result<()> {
    let ninja = generate_posix(&graph(
        Recipe::Command {
            command: "echo hi".into(),
        },
        "in",
        "out",
    )?)?;
    ensure!(
        ninja.contains("  command = echo hi\n"),
        "dollar-free command output must not change:\n{ninja}"
    );
    Ok(())
}
