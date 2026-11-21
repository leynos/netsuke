//! Script recipe integration cases for Ninja dollar escaping.
//!
//! The byte-preservation cases for the POSIX `printf %b` transport live in
//! the [`transport`] submodule, the lowering and execution helpers in the
//! [`runner`] submodule, and the typed table data in the [`cases`] submodule,
//! so each file stays within the module size limit.

use super::*;

// Only the Unix-gated execution cases decode a Bash command line, so on Windows
// these items would be unused and `-D warnings` would reject the test build.
#[cfg(unix)]
use test_support::shell_transport::{INTERPRETER_PATH, decode_bash_transport};

#[cfg(unix)]
use cases::{Comment, HostileSite, Marker};
use cases::{ControlCharacter, UnsafePath};
#[cfg(unix)]
use runner::{GeneratedCommand, ScriptCase, generate_for, lower_script, run_script};

mod cases;

#[cfg(unix)]
mod runner;

mod transport;

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

/// Verify a script marker after an active escape fails closed.
///
/// A single backslash before the marker cannot be preserved: the first
/// character of the substituted path decides what the backslash escapes. The
/// generator must reject the script rather than execute an altered command.
#[rstest]
fn escaped_script_marker_is_rejected() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: '\"; touch injected; #'\n    script: 'printf %s escaped > \\{{ outs }}'\n",
    )?;
    let error = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix)
        .expect_err("a marker after an active escape must fail closed");
    ensure!(
        matches!(
            error,
            netsuke::ir::IrGenError::InvalidCommand { ref command, .. }
                if command.trim_end() == "printf %s escaped > \\__NETSUKE_OUTS_PLACEHOLDER__"
        ),
        "unexpected error: {error:?}"
    );
    Ok(())
}

/// Verify a double-quoted script marker cannot create a second POSIX command.
#[cfg(unix)]
#[rstest]
#[case::apostrophe(Comment::Apostrophe)]
#[case::double_quote(Comment::DoubleQuote)]
fn quoted_script_marker_cannot_inject_a_command_boundary(#[case] comment: Comment) -> Result<()> {
    let output_path = "x\"; touch injected; echo \"y";
    let case = ScriptCase {
        target: output_path.to_owned(),
        manifest: format!(
            "netsuke_version: '1.0.0'\ntargets:\n  - name: '{output_path}'\n    script: |\n      {}\n      echo \"{{{{ outs }}}}\"\n",
            comment.line()
        ),
    };
    let command = lower_script(&case)?;
    let run = run_script(&command)?;
    ensure!(
        run.output.stdout == format!("{output_path}\n").as_bytes(),
        "script marker must remain one argument, got: {}",
        String::from_utf8_lossy(&run.output.stdout)
    );
    ensure!(
        !run.workspace.path().join("injected").exists(),
        "double-quoted marker must not execute the injected touch command"
    );
    Ok(())
}

/// Verify a backtick inside `$()` cannot unquote a later script marker.
///
/// The closing backtick once restored its saved quote context into the
/// script-level field, so a marker after the substitution was encoded as if it
/// sat inside double quotes. For a path containing `;` the double-quoted
/// encoding leaves the separator executable while the unquoted encoding quotes
/// it, so the corruption is observable as a command boundary.
#[cfg(unix)]
#[rstest]
#[case::outputs(HostileSite::OutputPath, Marker::Outs)]
#[case::inputs(HostileSite::InputPath, Marker::Ins)]
fn backtick_inside_command_substitution_cannot_unquote_a_script_marker(
    #[case] site: HostileSite,
    #[case] marker: Marker,
) -> Result<()> {
    let source_line = site
        .source()
        .map_or_else(String::new, |path| format!("    sources: '{path}'\n"));
    let case = ScriptCase {
        target: site.target().to_owned(),
        manifest: format!(
            "netsuke_version: '1.0.0'\ntargets:\n  - name: '{}'\n{source_line}    script: |\n      printf '%s\\n' $(printf %s \"`printf x`\") {}\n",
            site.target(),
            marker.token()
        ),
    };
    let command = lower_script(&case)?;
    let run = run_script(&command)?;
    ensure!(
        run.output.stdout == b"x\na; touch injected; echo b\n",
        "the marker after $() must stay one argument, got: {}",
        String::from_utf8_lossy(&run.output.stdout)
    );
    ensure!(
        !run.workspace.path().join("injected").exists(),
        "a marker after $() must not execute the injected touch command"
    );
    Ok(())
}

/// Verify a backtick escaped inside a nested `$()` frame stays literal.
///
/// The backtick body's escape rule is applied to the whole body before the
/// body is parsed, so it reaches a backtick inside a `$()` frame and its
/// quotes there: the escaped backtick is data, not the closer. A marker after
/// the substitution must still be lowered for its own region, and a hostile
/// path must stay one argument.
#[cfg(unix)]
#[rstest]
fn escaped_backtick_inside_nested_frame_keeps_the_region_open() -> Result<()> {
    let case = ScriptCase {
        target: "out".to_owned(),
        manifest: "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: 'a; touch injected; echo b'\n    script: |\n      printf '<%s>\\n' `printf %s \"$(printf %s '\\`')\"` {{ ins }}\n".to_owned(),
    };
    let command = lower_script(&case)?;
    let run = run_script(&command)?;
    ensure!(
        run.output.stdout == b"<`>\n<a; touch injected; echo b>\n",
        "the escaped backtick must stay literal and the marker stay one argument, got: {}",
        String::from_utf8_lossy(&run.output.stdout)
    );
    ensure!(
        !run.workspace.path().join("injected").exists(),
        "the marker after the substitution must not execute the injected touch command"
    );
    Ok(())
}

/// Verify a no-break space before `#` cannot turn a quoted marker into syntax.
///
/// A POSIX shell ends a word only at blank, newline, or an operator, so the
/// no-break space is word data: `#` starts a comment only after a separator,
/// and the quote that follows it opens a double-quoted region. The hostile
/// output path inside that region must therefore be encoded for a quoted site.
/// A scanner that treated the no-break space as whitespace would make the quote
/// inert and insert the path for an unquoted site, so the recipe would no
/// longer echo exactly the path and the shell could run the injected `touch`.
#[cfg(unix)]
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
fn non_ascii_space_before_hash_keeps_the_marker_quoted(#[case] shell: RecipeShell) -> Result<()> {
    let output_path = "x\"; touch injected; echo \"y";
    let manifest = manifest::from_str(&format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: '{output_path}'\n    script: |\n      echo x\u{00a0}#\"\n      echo {{{{ outs }}}}\"\n"
    ))?;
    let ninja = generate_for(
        shell,
        &BuildGraph::from_manifest_for_shell(&manifest, shell)?,
    )?;
    let binding = ninja_commands(&ninja, output_path)?;
    let script = match shell {
        // `ninja -t commands` terminates its output with a newline, which is
        // outside the transport's quoted argument.
        RecipeShell::Bash => decode_bash_transport(INTERPRETER_PATH, binding.trim_end())?,
        RecipeShell::Posix | RecipeShell::PowerShell => binding,
    };

    let run = run_script(&GeneratedCommand::new(script))?;
    let expected = "x\u{00a0}#\necho x\"; touch injected; echo \"y\n";
    ensure!(
        run.output.stdout == expected.as_bytes(),
        "a no-break space must not end the word before '#', got: {}",
        String::from_utf8_lossy(&run.output.stdout)
    );
    ensure!(
        !run.workspace.path().join("injected").exists(),
        "the quote after a non-separator must keep the marker from executing the injected touch"
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

/// Verify a `$()` that closes before its backtick is rejected at lowering.
///
/// The saved backtick context belongs to the `$()` region that opened it. Once
/// that region has closed there is no correct region to restore, so lowering
/// must fail closed rather than let a later marker inherit corrupted state.
#[rstest]
fn mismatched_backtick_and_command_substitution_nesting_is_rejected() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: in\n    script: |\n      printf '%s' $(printf %s `printf x) printf y` {{ ins }}\n",
    )?;
    let error = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::Posix)
        .expect_err("a $() that closes before its backtick must fail closed");
    ensure!(
        matches!(error, netsuke::ir::IrGenError::InvalidCommand { .. }),
        "unexpected error: {error:?}"
    );
    Ok(())
}

/// Verify that command control characters cannot inject Ninja syntax.
#[rstest]
#[case::newline(ControlCharacter::Newline)]
#[case::carriage_return(ControlCharacter::CarriageReturn)]
fn command_control_characters_are_rejected(#[case] character: ControlCharacter) -> Result<()> {
    let result = generate_posix(&graph(
        Recipe::Command {
            command: character.command().into(),
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
#[case::dollar(UnsafePath::Dollar)]
#[case::colon(UnsafePath::Colon)]
#[case::pipe(UnsafePath::Pipe)]
#[case::tab(UnsafePath::Tab)]
#[case::nul(UnsafePath::Nul)]
#[case::carriage_return(UnsafePath::CarriageReturn)]
#[case::newline(UnsafePath::Newline)]
fn unsafe_paths_are_rejected(#[case] path: UnsafePath) -> Result<()> {
    let result = generate_posix(&graph(
        Recipe::Command {
            command: format!("cat {INS_TOKEN} > {OUTS_TOKEN}").into(),
            isolation: netsuke::ast::CommandIsolation::Shared,
        },
        path.text(),
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
