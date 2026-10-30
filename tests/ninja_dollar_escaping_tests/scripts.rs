//! Script recipe integration cases for Ninja dollar escaping.

use super::*;
#[cfg(unix)]
use netsuke::manifest::{EnvAccessPolicy, EnvReadError, EnvReader};
#[cfg(unix)]
use std::sync::Arc;

/// Verify script `$in` and `$out` become literal shell variables in Ninja.
#[rstest]
fn script_dollar_in_out_are_doubled_as_shell_variables() -> Result<()> {
    let ninja = generate_posix(&graph(
        Recipe::Script {
            script: "printf '%s' $in > $out".into(),
        },
        "in",
        "out",
    )?)?;

    ensure!(
        ninja.contains("\\$$in") && ninja.contains("\\$$out"),
        "script shell variables must be doubled for Ninja:\n{ninja}"
    );
    Ok(())
}

/// Verify script `$in` and `$out` read child shell variables after Ninja execution.
#[cfg(unix)]
#[rstest]
fn script_dollar_in_out_reach_the_child_shell() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: declared-input\n    script: |\n      printf '%s:%s' \"$in\" \"$out\" > {{ outs }}\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let actual = ninja_output(
        &ninja,
        &[("in", "shell-input"), ("out", "shell-output")],
        Some(("declared-input", "declared-input-value")),
    )?;
    ensure!(
        actual == "shell-input:shell-output",
        "script shell variables must not lower to declared paths: {actual:?}"
    );
    Ok(())
}

/// Verify double-quoted script markers keep shell punctuation inert.
#[cfg(unix)]
#[rstest]
fn script_double_quoted_markers_quote_shell_punctuation() -> Result<()> {
    assert_script_output(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: 'foo;id'\n    script: \"printf '%s' \\\"{{ ins }}\\\" > {{ outs }}\"\n",
        ("foo;id", "script input"),
        "foo;id",
        "double-quoted script markers must not execute shell punctuation",
    )
}

/// Verify single-quoted script markers keep shell punctuation inert.
#[cfg(unix)]
#[rstest]
fn script_single_quoted_markers_quote_shell_punctuation() -> Result<()> {
    assert_script_output(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: 'foo;id'\n    script: \"printf '%s' '{{ ins }}' > '{{ outs }}'\"\n",
        ("foo;id", "script input"),
        "foo;id",
        "single-quoted script markers must not execute shell punctuation",
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
    let actual = ninja_output(&ninja, &[], None)?;
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
            isolation: netsuke::ast::CommandIsolation::Shared,
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
            isolation: netsuke::ast::CommandIsolation::Shared,
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
            isolation: netsuke::ast::CommandIsolation::Shared,
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

/// The literal byte sequence the reported trigger carries.
///
/// It begins with a backslash so `printf %b` reads `\047` as an octal
/// apostrophe; the remainder turns the reconstructed byte into a command
/// boundary and a redirect to `injected`. Only the Unix execution cases below
/// consume it, so it is gated with them rather than left dead on Windows.
#[cfg(unix)]
const BACKSLASH_PAYLOAD: &str = r"\047;>injected;#";

/// Run one generated script and return its byte-exact output and workspace.
#[cfg(unix)]
fn ninja_output_bytes(
    ninja: &str,
    input: Option<(&str, &str)>,
) -> Result<(Vec<u8>, NinjaWorkspace)> {
    let workspace = run_ninja_workspace(ninja, &[], input)?;
    let bytes = test_support::fs::read(workspace.path.join("out"))
        .context("read byte-exact script output")?;
    Ok((bytes, workspace))
}

/// Assert the transported script kept `expected` and executed no redirect.
#[cfg(unix)]
fn assert_payload_is_inert(
    actual: &[u8],
    workspace: &NinjaWorkspace,
    expected: &[u8],
) -> Result<()> {
    ensure!(
        actual == expected,
        "script bytes changed in transport: expected {:?}, got {:?}",
        String::from_utf8_lossy(expected),
        String::from_utf8_lossy(actual)
    );
    ensure!(
        !test_support::fs::exists(workspace.path.join("injected")),
        "transported script bytes executed the injected redirect"
    );
    Ok(())
}

/// Verify a script literal survives `printf %b` without becoming shell syntax.
#[cfg(unix)]
#[rstest]
fn script_source_backslash_payload_is_not_reinterpreted() -> Result<()> {
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: |\n      printf '%s' '{BACKSLASH_PAYLOAD}' > {{{{ outs }}}}\n"
    );
    let manifest = manifest::from_str(&source)?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let (actual, workspace) = ninja_output_bytes(&ninja, None)?;
    assert_payload_is_inert(&actual, &workspace, BACKSLASH_PAYLOAD.as_bytes())
}

/// Verify an input path survives each marker quoting context.
#[cfg(unix)]
#[rstest]
#[case::single_quoted("printf '%s' '{{ ins }}' > {{ outs }}")]
#[case::unquoted("printf '%s' {{ ins }} > {{ outs }}")]
#[case::double_quoted("printf '%s' \"{{ ins }}\" > {{ outs }}")]
fn input_path_backslash_payload_is_not_reinterpreted(#[case] script_line: &str) -> Result<()> {
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: '{BACKSLASH_PAYLOAD}'\n    script: |\n      {script_line}\n"
    );
    let manifest = manifest::from_str(&source)?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let (actual, workspace) =
        ninja_output_bytes(&ninja, Some((BACKSLASH_PAYLOAD, "input-contents")))?;
    assert_payload_is_inert(&actual, &workspace, BACKSLASH_PAYLOAD.as_bytes())
}

/// Verify a `shell_quote`d `env()` fallback reaches the final interpreter intact.
#[cfg(unix)]
#[rstest]
fn env_fallback_through_shell_quote_is_not_reinterpreted() -> Result<()> {
    // The fallback is a Jinja string literal, so it needs one extra layer of
    // backslash escaping before `shell_quote` sees the reported payload. This
    // is template-language escaping, not part of the script transport.
    let jinja_payload = BACKSLASH_PAYLOAD.replace('\\', "\\\\");
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: |\n      printf '%s\\n' {{{{ env('NETSUKE_ISSUE_967_UNSET', default='{jinja_payload}') | shell_quote }}}} > {{{{ outs }}}}\n"
    );
    let reader: EnvReader = Arc::new(|_| Err(EnvReadError::NotPresent));
    let manifest =
        manifest::from_str_with_env_and_policy(&source, &reader, &EnvAccessPolicy::default())?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let (actual, workspace) = ninja_output_bytes(&ninja, None)?;
    let expected = format!("{BACKSLASH_PAYLOAD}\n");
    assert_payload_is_inert(&actual, &workspace, expected.as_bytes())
}

/// Return printable ASCII with every Jinja delimiter pair removed.
#[cfg(unix)]
fn printable_ascii_without_jinja_delimiters() -> String {
    let mut text = (0x20u8..=0x7e).map(char::from).collect::<String>();
    for delimiter in ["{{", "}}", "{%", "%}", "{#", "#}"] {
        while let Some(index) = text.find(delimiter) {
            text.replace_range(index..index + delimiter.len(), "");
        }
    }
    text
}

/// Verify a quoted heredoc carries every backslash sequence byte for byte.
#[cfg(unix)]
#[rstest]
#[case::double_backslash(r"\\")]
#[case::literal_backslash_n(r"\n")]
#[case::literal_backslash_t(r"\t")]
#[case::literal_backslash_c(r"\c")]
#[case::literal_backslash_zero(r"\0")]
#[case::octal(r"\0101")]
#[case::hexadecimal(r"\x41")]
#[case::octal_apostrophe(r"\047")]
#[case::unicode(concat!(r"\", "u0041"))]
#[case::trailing_backslash("trailing\\")]
#[case::before_apostrophe(r"\'")]
#[case::before_double_quote(r#"\""#)]
#[case::before_backtick(r"\`")]
#[case::before_dollar(r"\$")]
fn quoted_heredoc_preserves_backslash_sequences(#[case] body: &str) -> Result<()> {
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: |\n      cat > {{{{ outs }}}} <<'EOF'\n      {body}\n      EOF\n"
    );
    let manifest = manifest::from_str(&source)?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let (actual, workspace) = ninja_output_bytes(&ninja, None)?;
    let expected = format!("{body}\n");
    ensure!(
        actual == expected.as_bytes(),
        "heredoc bytes changed in transport: expected {body:?}, got {:?}",
        String::from_utf8_lossy(&actual)
    );
    ensure!(
        !test_support::fs::exists(workspace.path.join("injected")),
        "heredoc bytes executed the injected redirect"
    );
    Ok(())
}

/// Verify the printable ASCII range survives the quoted-heredoc transport.
#[cfg(unix)]
#[rstest]
fn quoted_heredoc_preserves_printable_ascii() -> Result<()> {
    let body = printable_ascii_without_jinja_delimiters();
    let source = format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: |\n      cat > {{{{ outs }}}} <<'EOF'\n      {body}\n      EOF\n"
    );
    let manifest = manifest::from_str(&source)?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let (actual, workspace) = ninja_output_bytes(&ninja, None)?;
    let expected = format!("{body}\n");
    ensure!(
        actual == expected.as_bytes(),
        "printable ASCII changed in transport: expected {body:?}, got {:?}",
        String::from_utf8_lossy(&actual)
    );
    ensure!(
        !test_support::fs::exists(workspace.path.join("injected")),
        "printable ASCII bytes executed the injected redirect"
    );
    Ok(())
}
