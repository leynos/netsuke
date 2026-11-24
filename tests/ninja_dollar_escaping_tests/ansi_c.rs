//! Public-lowering regressions for markers inside Bash ANSI-C quoting.
//!
//! Bash reads `$'…'` as an ANSI-C string, where a backslash escapes the next
//! character. The ordinary single-quoted path encoding is therefore unsafe
//! inside that region, and lowering must reject the marker before any Ninja
//! file is generated. These cases exercise the manifest boundary so the rule is
//! pinned at the surface users reach, not only inside the scanner.

use super::*;

use netsuke::ir::IrGenError;
#[cfg(unix)]
use netsuke::ninja_gen::{InterpreterExecutable, generate_with_interpreter};
#[cfg(unix)]
use test_support::fs as test_fs;
use test_support::manifest::manifest_yaml;
#[cfg(unix)]
use test_support::shell_transport::{INTERPRETER_PATH, decode_bash_transport};

/// The quote-breaking path used as the target's output name.
const ANSI_C_LEADING_PATH: &str = "\\'; touch injected; #";

/// The recipe command produced by manifest rendering before interpolation.
const REJECTED_TEMPLATE: &str = "printf %s $'__NETSUKE_OUTS_PLACEHOLDER__'";

/// Lower a manifest body for one recipe shell and report the generated graph.
fn lower_for(shell: RecipeShell, body: &str) -> Result<BuildGraph> {
    let yaml = manifest_yaml(body);
    let manifest = manifest::from_str(&yaml)?;
    Ok(BuildGraph::from_manifest_for_shell(&manifest, shell)?)
}

/// Report the rejected command carried by an invalid-command error.
///
/// Lowering is reported through the test's opaque error type, so the
/// interpolation error is recovered before its command is inspected.
fn rejected_command(error: &anyhow::Error) -> Option<&str> {
    match error.downcast_ref::<IrGenError>()? {
        IrGenError::InvalidCommand { command, .. } => Some(command.trim_end()),
        _ => None,
    }
}

/// The manifest sections a rejection case supplies around its target.
struct TargetSections<'a> {
    /// The target's recipe declaration, already indented for the list entry.
    recipe: &'a str,
    /// Top-level sections, such as `rules:`, that follow the targets list.
    extra: &'a str,
}

/// Build a single-target manifest around one recipe declaration.
///
/// The target names the quote-breaking path and takes no sources, so the path
/// reaches the recipe only through the output marker and the target cannot
/// depend on itself.
fn target_manifest(TargetSections { recipe, extra }: TargetSections<'_>) -> String {
    let escaped_path = ANSI_C_LEADING_PATH.replace('\\', "\\\\");
    format!("targets:\n  - name: \"{escaped_path}\"\n{recipe}{extra}")
}

/// Reject a marker lowered anywhere inside an ANSI-C string.
///
/// The trigger covers a quote-breaking output name across a scalar command, a
/// named rule's command list, and a script. Both recipe shells are exercised
/// because the traversal cannot know which interpreter runs the recipe.
#[rstest]
#[case::scalar_command("    command: |\n      printf %s $'{{ outs }}'\n", "")]
#[case::rule_command_list(
    "    rule: build\n",
    "rules:\n  - name: build\n    command:\n      - |\n        printf %s $'{{ outs }}'\n"
)]
#[case::script("    script: |\n      printf %s $'{{ outs }}'\n", "")]
fn markers_inside_ansi_c_strings_are_rejected(
    #[case] recipe: &str,
    #[case] extra: &str,
) -> Result<()> {
    for shell in [RecipeShell::Posix, RecipeShell::Bash] {
        let error = lower_for(shell, &target_manifest(TargetSections { recipe, extra }))
            .expect_err("lowering must reject a marker inside ANSI-C quoting");
        ensure!(
            rejected_command(&error) == Some(REJECTED_TEMPLATE),
            "unexpected error for {shell:?}: {error:?}"
        );
    }
    Ok(())
}

/// Locate a real Bash, or `None` when the host provides none.
///
/// Bash is the only interpreter that gives `$'…'` its ANSI-C meaning, so the
/// execution cases below cannot fall back to `sh`, whose reading of the same
/// text differs.
#[cfg(unix)]
fn bash_shell() -> Option<&'static str> {
    ["/bin/bash", "/usr/bin/bash"]
        .into_iter()
        .find(|candidate| Path::new(candidate).is_file())
}

/// Report that this host provides no Bash, then end the case.
///
/// The ANSI-C execution cases have no subject without Bash, so the skip is
/// visible in the captured output rather than a silent pass.
#[cfg(unix)]
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable interpreter must be visible in the captured test output instead of the case passing silently"
)]
fn skip_without_bash() {
    eprintln!("skipped: no Bash on this host");
}

/// Apply the legacy ordinary single-quoted path encoding to `path`.
///
/// This is the encoding the fix rejects inside ANSI-C: it rewrites every
/// apostrophe as `'"'"'`, which only closes and reopens ordinary quotes. The
/// helper is deliberately the historical algorithm rather than a call into the
/// library, so the regression pins the defect instead of the current encoder.
#[cfg(unix)]
fn legacy_single_quote_encoding(path: &str) -> String {
    path.replace('\'', "'\"'\"'")
}

/// Show that the legacy encoding is not safe inside ANSI-C quoting.
///
/// Bash closes an ANSI-C string at an unescaped apostrophe. A path ending a
/// backslash immediately before the rewritten apostrophe therefore escapes
/// that apostrophe, closes on a later one, and leaves the remaining text to be
/// parsed in a different quote state. `shlex` accepts the line, which is why
/// the post-substitution guard cannot be the defence: only rejecting the
/// marker before encoding is.
///
/// The positive control passes the same path as one literal argument, so the
/// failure below is attributable to the encoding rather than to Bash itself.
#[cfg(unix)]
#[rstest]
fn legacy_single_quote_encoding_is_not_valid_ansi_c() -> Result<()> {
    let Some(bash) = bash_shell() else {
        skip_without_bash();
        return Ok(());
    };
    let encoded = legacy_single_quote_encoding(ANSI_C_LEADING_PATH);
    let line = format!("printf '[%s]\\n' $'{encoded}'");
    ensure!(
        shlex::split(&line).is_some(),
        "the post-substitution guard accepts this line, so it cannot be the defence"
    );

    let workspace = tempfile::tempdir()?;
    let legacy = Command::new(bash)
        .args(["-c", &line])
        .current_dir(workspace.path())
        .env_clear()
        .env("PATH", host_path()?)
        .output()
        .context("run the legacy encoding under Bash")?;
    ensure!(
        !legacy.status.success(),
        "Bash must not accept the legacy encoding as one word: {line}"
    );
    ensure!(
        legacy.stdout.is_empty(),
        "a rejected line must produce no output, got: {}",
        String::from_utf8_lossy(&legacy.stdout)
    );
    ensure!(
        !test_fs::exists(workspace.path().join("injected")),
        "the legacy encoding must not run the injected command"
    );

    let control = Command::new(bash)
        .args(["-c", "printf '[%s]\\n' \"$1\"", "_", ANSI_C_LEADING_PATH])
        .current_dir(workspace.path())
        .env_clear()
        .env("PATH", host_path()?)
        .output()
        .context("run the literal-argument control under Bash")?;
    ensure!(
        control.status.success()
            && control.stdout == format!("[{ANSI_C_LEADING_PATH}]\n").as_bytes(),
        "the positive control must pass the path through unchanged: {}",
        String::from_utf8_lossy(&control.stderr)
    );
    Ok(())
}

/// The source path used by the executable ANSI-C cases.
///
/// It carries an apostrophe, a backslash, a separator, and a comment marker,
/// so an unquoted insertion would split the word or execute the `touch`.
#[cfg(unix)]
const ANSI_C_SOURCE_PATH: &str = "a'b\\c; touch injected; #";

/// Render one executable case and fail unless its generated command is bound.
#[cfg(unix)]
fn executable_case(shell: RecipeShell) -> Result<(String, String)> {
    let manifest = manifest::from_str(&ansi_c_manifest())?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, shell)?;
    let ninja = generate_executable(shell, &graph)?;
    let binding = command_binding(&ninja)?;
    let script = transport_script(shell, &binding)?;
    Ok((ninja, script))
}

/// The command text Ninja holds for one generated build edge.
#[cfg(unix)]
struct CommandBinding<'a>(&'a str);

#[cfg(unix)]
impl<'a> CommandBinding<'a> {
    /// Borrow the command text carried by this binding.
    const fn as_str(&self) -> &'a str {
        self.0
    }
}

/// Extract the command binding from generated Ninja text.
#[cfg(unix)]
fn command_binding(ninja: &str) -> Result<CommandBinding<'_>> {
    ninja
        .lines()
        .find_map(|line| line.strip_prefix("  command = ").map(CommandBinding))
        .context("generated Ninja should carry one command binding")
}

/// Lower and render the executable ANSI-C manifest for `shell`.
///
/// The recipe uses `$'a b'`: an escape-free ANSI-C string, which is the shape
/// lowering accepts. A string containing a backslash is refused outright,
/// because its extent differs between the ANSI-C and POSIX readings. The
/// marker follows the closed region, so it takes the unquoted encoding.
#[cfg(unix)]
fn ansi_c_manifest() -> String {
    concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: out\n",
        "    sources: \"a'b\\\\c; touch injected; #\"\n",
        "    command: |\n",
        "      printf '<%s>' $'a b' {{ ins }} > {{ outs }}\n"
    )
    .to_owned()
}

/// Generate Ninja text for `shell`, binding the fixture interpreter for Bash.
#[cfg(unix)]
fn generate_executable(shell: RecipeShell, graph: &BuildGraph) -> Result<String> {
    match shell {
        RecipeShell::Bash => {
            let interpreter =
                InterpreterExecutable::from_absolute_path(Utf8PathBuf::from(INTERPRETER_PATH))?;
            Ok(generate_with_interpreter(graph, shell, &interpreter)?)
        }
        RecipeShell::Posix => Ok(generate_with_shell(graph, shell)?),
        RecipeShell::PowerShell => bail!("this case covers only the POSIX lexis"),
    }
}

/// Recover the script the selected transport would execute.
#[cfg(unix)]
fn transport_script(shell: RecipeShell, binding: &CommandBinding<'_>) -> Result<String> {
    // `$$` is the generated-file spelling of one shell dollar; Ninja collapses
    // it before the child shell runs, which running the text directly must
    // model.
    let command = binding.as_str();
    let script = match shell {
        RecipeShell::Bash => decode_bash_transport(INTERPRETER_PATH, command)?,
        RecipeShell::Posix | RecipeShell::PowerShell => command.to_owned(),
    };
    Ok(script.replace("$$", "$"))
}

/// Keep ANSI-C text before a marker intact through real Bash execution.
///
/// The command route is run as Bash would receive it: directly for `Posix` and
/// after decoding its transport for `Bash`. Both must produce the ANSI-C word
/// and the source path as separate arguments, byte for byte, with no injected
/// command.
#[cfg(unix)]
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
fn ansi_c_text_before_a_marker_survives_real_bash(#[case] shell: RecipeShell) -> Result<()> {
    let Some(bash) = bash_shell() else {
        skip_without_bash();
        return Ok(());
    };
    let (_ninja, script) = executable_case(shell)?;
    ensure!(
        script.contains("$'a b'"),
        "the ANSI-C word must survive lowering, got: {script}"
    );

    let workspace = tempfile::tempdir()?;
    let output = Command::new(bash)
        .args(["-ec", &script])
        .current_dir(workspace.path())
        .env_clear()
        .env("PATH", host_path()?)
        .output()
        .context("run the generated recipe under Bash")?;
    ensure!(
        output.status.success(),
        "generated recipe failed for {shell:?}: {}\nscript: {script}",
        String::from_utf8_lossy(&output.stderr)
    );
    let written =
        test_fs::read_to_string(workspace.path().join("out")).context("read the recipe output")?;
    ensure!(
        written == format!("<a b><{ANSI_C_SOURCE_PATH}>"),
        "the ANSI-C word and path must stay separate arguments, got: {written:?}"
    );
    ensure!(
        !test_fs::exists(workspace.path().join("injected")),
        "the path following the ANSI-C word must not execute its separator"
    );
    Ok(())
}
