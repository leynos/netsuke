//! Heredoc delimiter integration cases for Ninja dollar escaping.

// Every case here runs a POSIX child shell, so the parent's bindings are only
// used on Unix; an ungated glob import would warn on Windows.
#[cfg(unix)]
use super::*;

// Verify a Unicode separator cannot move a marker into an expanding body.
//
// `/bin/sh` keeps a non-blank separator inside the delimiter word, so the
// fake `EOF` line and the marker after it stay in the heredoc body. If the
// tracker ends the body at that fake line, it lowers a path into expanding
// heredoc text, where backticks in the path remain executable.
#[cfg(unix)]
#[rstest]
fn unicode_separator_does_not_move_a_heredoc_marker_into_the_shell_body() -> Result<()> {
    let sentinel = "NETSUKE_TEST_SENTINEL";
    let input_path = format!("a`>{sentinel}`b");
    let terminator = "EOF\u{00A0}suffix";
    let manifest = manifest::from_str(&format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: '{input_path}'\n    script: |\n      cat <<{terminator} > {{{{ outs }}}}\n      body\n      EOF\n      {{{{ ins }}}}\n      {terminator}\n      printf '%s' after >> {{{{ outs }}}}\n"
    ))?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let workspace = NinjaWorkspace::create(&ninja)?;
    workspace
        .directory
        .write(input_path.as_str(), "input contents")
        .context("write the backtick-bearing input")?;

    run_ninja_target(&workspace, &[])?;
    ensure!(
        !workspace.directory.exists(sentinel),
        "the expanding heredoc body must not execute a filename command substitution"
    );
    let actual = workspace
        .directory
        .read_to_string("out")
        .context("read heredoc output")?;
    let expected = format!("body\nEOF\n{INS_TOKEN}\nafter");
    ensure!(
        actual == expected,
        "expected the shell's real heredoc boundaries, got {actual:?}"
    );
    Ok(())
}

// Verify a continued body line cannot end an unquoted heredoc early.
//
// The shell removes backslash-newline throughout an expanding body before it
// looks for the terminator, so `prefix\` joined to `EOF` is `prefixEOF` and
// keeps the body open. Ending the body at that physical line would lower the
// following marker into expanding heredoc text, where a backtick in the input
// path remains executable.
#[cfg(unix)]
#[rstest]
fn heredoc_body_continuation_does_not_expose_an_expanding_body() -> Result<()> {
    let sentinel = "NETSUKE_TEST_CONTINUATION_SENTINEL";
    let input_path = format!("a`>{sentinel}`b");
    let manifest = manifest::from_str(&format!(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    sources: '{input_path}'\n    script: |\n      cat <<EOF > {{{{ outs }}}}\n      prefix\\\n      EOF\n      {{{{ ins }}}}\n      EOF\n      printf '%s' after >> {{{{ outs }}}}\n"
    ))?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let workspace = NinjaWorkspace::create(&ninja)?;
    workspace
        .directory
        .write(input_path.as_str(), "input contents")
        .context("write the backtick-bearing input")?;

    run_ninja_target(&workspace, &[])?;
    ensure!(
        !workspace.directory.exists(sentinel),
        "the expanding heredoc body must not execute a filename command substitution"
    );
    let actual = workspace
        .directory
        .read_to_string("out")
        .context("read heredoc output")?;
    let expected = format!("prefixEOF\n{INS_TOKEN}\nafter");
    ensure!(
        actual == expected,
        "expected the shell's joined heredoc line, got {actual:?}"
    );
    Ok(())
}

// Verify an arithmetic left shift survives lowering and reaches the shell.
//
// A tracker that reads `<<` as a heredoc operator consumes the rest of the
// line as a delimiter, so the redirect and its marker never reach the shell.
// The assertion compares evaluated shell output, not just lowered text.
#[cfg(unix)]
#[test]
fn arithmetic_shift_reaches_the_generated_recipe() -> Result<()> {
    let manifest = manifest::from_str(
        "netsuke_version: '1.0.0'\ntargets:\n  - name: out\n    script: |\n      printf '%s' \"$((1 << 3))\" > {{ outs }}\n",
    )?;
    let ninja = generate_posix(&BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)?;

    let workspace = NinjaWorkspace::create(&ninja)?;
    run_ninja_target(&workspace, &[])?;
    let actual = workspace
        .directory
        .read_to_string("out")
        .context("read arithmetic output")?;
    ensure!(
        actual == "8",
        "expected the shell to evaluate the left shift, got {actual:?}"
    );
    Ok(())
}
