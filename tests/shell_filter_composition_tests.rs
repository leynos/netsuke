//! OBL-COMPOSITION: the shell filters compose with the rest of the pipeline.
//!
//! Every other test in this area exercises an encoder in isolation. This one
//! renders a manifest whose recipe interpolates `shell_quote`/`shell_join`
//! output, lowers it through the recipe-marker machinery, generates Ninja text,
//! and — where the host provides an interpreter — runs that text and asserts on
//! the argv the interpreter actually produced.
//!
//! The obligation exists because two guards downstream are weaker than they
//! look. `is_valid_command_for_shell` returns `true` unconditionally for
//! PowerShell, so nothing on that path checks the generated text at all; and
//! the command-list renderer wraps each entry in a canonical `'...'` for its
//! `eval` payload, which is a *second* layer of POSIX quoting applied to text
//! that now routinely contains the first layer's quotes. Neither interaction is
//! visible to a test that calls the filter and compares strings.
//!
//! The three `RecipeShell` variants are not three interpreters. `Posix` runs
//! the recipe text directly, `Bash` wraps it in `bash.exe -e -c "…"`, and
//! `PowerShell` encodes it as base64 for `-EncodedCommand`. Only the `Posix`
//! transport is executable on this host, so the other two are asserted on
//! their *transport shape* — that the payload survives whichever encoding the
//! renderer chose — and the `Bash` payload is additionally decoded back to its
//! inner script and run under the host's own POSIX shell, which is a real
//! execution of the text `Bash` would hand to `bash.exe`.

#![cfg(unix)]

use anyhow::{Context, Result, bail, ensure};
use base64::Engine as _;
use camino::Utf8PathBuf;
use netsuke::{
    manifest,
    ninja_gen::{RecipeShell, generate_with_shell},
};
use rstest::rstest;
use std::process::Command;

/// The value the scalar composition cases quote.
///
/// It carries all three of the interactions this file exists to check: a space
/// (which forces the quoter to emit quotes at all), a single quote (which the
/// quoter escapes *into* the quoted form), and a dollar sign (which ADR-014
/// requires Ninja to see doubled).
const COMPOSED_VALUE: &str = "arg with 'quote' and $HOME";

/// The value used by the command-list cases.
///
/// A command-list entry is re-quoted for its `eval` payload, so the interesting
/// case is a value whose quoting produces quotes the wrapper must then escape.
const LIST_VALUE: &str = "list 'item' $HOME";

/// The sentinel the execution cases print before the argument under test.
///
/// A sentinel rather than plain stdout so that any shell chatter around the
/// recipe — a trap message, a wrapper diagnostic — is distinguishable from the
/// argument vector.
const SENTINEL: &str = "COMPOSITION_SENTINEL_9c1f";

/// The PowerShell prefix `ninja_gen` puts before the base64 payload.
const POWER_SHELL_PREFIX: &str = "powershell.exe -NoLogo -NoProfile -NonInteractive \
                                   -ExecutionPolicy Bypass -EncodedCommand ";

/// The `bash.exe -e -c ` prefix the Bash transport wraps its script in.
const BASH_PREFIX: &str = "bash.exe -e -c ";

/// Locate `sh`, or `None` when the host has no POSIX shell.
fn posix_shell() -> Option<Utf8PathBuf> {
    for candidate in ["/bin/sh", "/usr/bin/sh", "/usr/local/bin/sh"] {
        let path = Utf8PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Run `script` under a real POSIX shell and return its stdout.
fn run_posix_shell(shell: &Utf8PathBuf, script: &str) -> Result<String> {
    let output = Command::new(shell.as_str())
        .arg("-c")
        .arg(script)
        .output()
        .with_context(|| format!("run {shell} -c {script}"))?;
    ensure!(
        output.status.success(),
        "{shell} exited with {} for {script}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).context("shell stdout should be valid UTF-8")
}

/// Prefix every line of `text` with `by` spaces.
fn indent(text: &str, by: usize) -> String {
    let pad = " ".repeat(by);
    text.lines().map(|line| format!("{pad}{line}\n")).collect()
}

/// The whole manifest source for one composition case.
///
/// `recipe` is the target's complete `command:` YAML, written at column zero
/// and indented here. The value reaches the template through a literal block
/// scalar, so no YAML escaping stands between the fixture and the filter, and
/// through a *variable* rather than a spliced literal, so no Jinja escaping
/// does either.
fn manifest_with(recipe: &str, value: &str) -> String {
    format!(
        "netsuke_version: \"1.0.0\"\nvars:\n  seam_value: |-\n{value}targets:\n  - name: out\n{recipe}    description: composition\n",
        value = indent(value, 4),
        recipe = indent(recipe, 4),
    )
}

/// The manifest for a scalar recipe whose command is `command`.
fn scalar_manifest(command: &str, value: &str) -> String {
    manifest_with(&format!("command: |\n  {command}\n"), value)
}

/// The manifest for a command-list recipe whose single entry is `entry`.
///
/// The entry is a *quoted* YAML scalar: a bare `true` would parse as a YAML
/// boolean and fail `StringOrList` deserialization, and a block scalar would
/// leave the list as one string. The double quotes keep the JSON-escaped `\n`
/// the recipe needs.
fn list_manifest(entry: &str, value: &str) -> String {
    manifest_with(
        &format!("command:\n  - \"{}\"\n", entry.replace('\\', "\\\\")),
        value,
    )
}

/// Lower and generate one manifest for `shell`, returning the Ninja text.
fn generate(source: &str, shell: RecipeShell) -> Result<String> {
    let parsed = manifest::from_str(source)?;
    let graph = netsuke::ir::BuildGraph::from_manifest_for_shell(&parsed, shell)?;
    Ok(generate_with_shell(&graph, shell)?)
}

/// The command binding the generated Ninja carries.
fn command_binding(ninja: &str) -> Result<String> {
    ninja
        .lines()
        .find_map(|line| line.strip_prefix("  command = ").map(str::to_owned))
        .context("generated Ninja should carry a command binding")
}

/// Recover the inner script of the `Bash` transport's `bash.exe -e -c "…"`.
///
/// This is a *decoder*, not a restatement of the encoder: it undoes the
/// Windows argument quoting so the result is the exact text `bash.exe` would
/// receive as its `-c` argument. Running that text under the host's own POSIX
/// shell is what makes the Bash arm an execution test rather than a shape
/// assertion.
fn decode_bash_transport(binding: &str) -> Result<String> {
    let argument = binding
        .strip_prefix(BASH_PREFIX)
        .with_context(|| format!("Bash transport should start with {BASH_PREFIX:?}: {binding}"))?;
    let inner = argument
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .with_context(|| {
            format!("Bash transport argument should be one quoted word: {argument}")
        })?;

    // Windows argument quoting: a backslash before a `"` is an escape, and the
    // run of backslashes before the closing quote is halved.
    let mut decoded = String::with_capacity(inner.len());
    let mut backslashes = 0usize;
    for character in inner.chars() {
        if character == '\\' {
            backslashes += 1;
            continue;
        }
        if character == '"' && backslashes % 2 == 1 {
            decoded.push_str(&"\\".repeat(backslashes / 2));
            decoded.push('"');
        } else {
            decoded.push_str(&"\\".repeat(backslashes));
            decoded.push(character);
        }
        backslashes = 0;
    }
    decoded.push_str(&"\\".repeat(backslashes / 2));
    Ok(decoded)
}

/// Decode the PowerShell transport's base64 payload back to its script.
///
/// The renderer prepends a fixed bootstrap; the payload after the prefix is
/// UTF-16LE base64. Decoding it is how the PowerShell arm checks that the
/// *filter's* output reached the interpreter, rather than only that the
/// renderer produced a well-formed command.
fn decode_power_shell_transport(binding: &str) -> Result<String> {
    let payload = binding
        .strip_prefix(POWER_SHELL_PREFIX)
        .with_context(|| format!("PowerShell transport should start with the prefix: {binding}"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .context("PowerShell payload should be valid base64")?;
    ensure!(
        bytes.len() % 2 == 0,
        "UTF-16LE payload should have an even byte count, got {}",
        bytes.len()
    );
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).context("PowerShell payload should be valid UTF-16")
}

/// Every scalar recipe, whatever its transport, carries the quoted value.
///
/// The assertion differs by transport because the transports do: `Posix` puts
/// the text on the command line, the other two hide it inside an encoding whose
/// only honest check is a decode.
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
#[case::power_shell(RecipeShell::PowerShell)]
fn scalar_recipes_carry_the_quoted_value(#[case] shell: RecipeShell) -> Result<()> {
    let source = scalar_manifest(
        &format!("printf '{SENTINEL}%s\\n' {{{{ seam_value | shell_quote }}}}"),
        COMPOSED_VALUE,
    );
    let ninja = generate(&source, shell)?;
    let binding = command_binding(&ninja)?;

    let script = match shell {
        RecipeShell::Posix => binding.clone(),
        RecipeShell::Bash => decode_bash_transport(&binding)?,
        RecipeShell::PowerShell => decode_power_shell_transport(&binding)?,
    };
    ensure!(
        script.contains(SENTINEL),
        "the recipe text should survive the {shell:?} transport: {script:?}"
    );
    ensure!(
        !script.contains(COMPOSED_VALUE),
        "the raw value leaked into the command unquoted: {script:?}"
    );
    Ok(())
}

/// The scalar recipe, run by a real interpreter, yields the exact argument.
///
/// `Posix` runs the binding text directly. `Bash` runs the inner script its own
/// transport would hand to `bash.exe`, decoded above. `PowerShell` has no
/// interpreter on this host, so it is exercised only through the decode.
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
fn scalar_recipes_execute_to_the_intended_argument(#[case] shell: RecipeShell) -> Result<()> {
    let Some(interpreter) = posix_shell() else {
        eprintln!("skipped: no POSIX shell on this host");
        return Ok(());
    };
    let source = scalar_manifest(
        &format!("printf '{SENTINEL}%s\\n' {{{{ seam_value | shell_quote }}}}"),
        COMPOSED_VALUE,
    );
    let ninja = generate(&source, shell)?;
    let binding = command_binding(&ninja)?;

    // Ninja resolves `$$` to `$` before the shell sees the text; running the
    // binding directly does not, so `$$` is collapsed here to model that step.
    let script = match shell {
        RecipeShell::Bash => decode_bash_transport(&binding)?,
        RecipeShell::Posix | RecipeShell::PowerShell => binding.clone(),
    }
    .replace("$$", "$");

    let observed = run_posix_shell(&interpreter, &script)?;
    ensure!(
        observed == format!("{SENTINEL}{COMPOSED_VALUE}\n"),
        "the interpreter read {observed:?} from {script:?}, expected one argument"
    );
    Ok(())
}

/// ADR-014's `$`-doubling is what preserves the value through Ninja.
///
/// The negative control for the case above. Ninja collapses `$$` to `$` before
/// the shell runs; text that carried the value's `$HOME` without doubling would
/// have Ninja consume it, and the recipe would see a different word. The
/// assertion is on that difference rather than on the text, so it fails if the
/// doubling is dropped *or* if the value stops containing a dollar.
#[rstest]
fn removing_the_dollar_doubling_changes_the_recipe_word() -> Result<()> {
    let Some(interpreter) = posix_shell() else {
        eprintln!("skipped: no POSIX shell on this host");
        return Ok(());
    };
    let source = scalar_manifest(
        &format!("printf '{SENTINEL}%s\\n' {{{{ seam_value | shell_quote }}}}"),
        COMPOSED_VALUE,
    );
    let ninja = generate(&source, RecipeShell::Posix)?;
    let binding = command_binding(&ninja)?;
    ensure!(
        binding.contains("$$"),
        "the generated Ninja should carry a doubled dollar for Ninja to collapse: {binding}"
    );

    let doubled = run_posix_shell(&interpreter, &binding.replace("$$", "$"))?;
    let undoubled = run_posix_shell(&interpreter, &binding)?;
    ensure!(
        doubled != undoubled,
        "collapsing the dollar changed nothing, so the control proves nothing"
    );
    ensure!(
        doubled == format!("{SENTINEL}{COMPOSED_VALUE}\n"),
        "the collapsed form should be the one that carries the value: {doubled:?}"
    );
    Ok(())
}

/// A command-list recipe survives its transport's entry wrapper.
///
/// The POSIX transports wrap each entry for `eval`, and that wrapper applies
/// `shell_single_quote` to the whole entry — so the filter's quotes become
/// *data* the wrapper must escape. PowerShell inlines the entries into its
/// script instead, with no second quoting layer; the shared assertion is that
/// the entry text survives whichever wrapping the transport chose.
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
#[case::power_shell(RecipeShell::PowerShell)]
fn command_list_recipes_carry_the_quoted_value(#[case] shell: RecipeShell) -> Result<()> {
    let source = list_manifest(
        &format!("printf '{SENTINEL}%s\\n' {{{{ seam_value | shell_quote }}}}"),
        LIST_VALUE,
    );
    let ninja = generate(&source, shell)?;
    let binding = command_binding(&ninja)?;

    let script = match shell {
        RecipeShell::Posix => binding.clone(),
        RecipeShell::Bash => decode_bash_transport(&binding)?,
        RecipeShell::PowerShell => decode_power_shell_transport(&binding)?,
    };
    if shell == RecipeShell::PowerShell {
        // PowerShell has no `eval` wrapper — and this branch is the reason the
        // assertion below is not shared: assuming one would be asserting a
        // renderer behaviour that does not exist.
        ensure!(
            !script.contains("Invoke-Expression"),
            "the PowerShell transport should inline entries rather than eval them: {script:?}"
        );
    } else {
        ensure!(
            script.contains("eval "),
            "a POSIX command-list recipe should reach the shell through eval: {script:?}"
        );
    }
    ensure!(
        script.contains(SENTINEL),
        "the entry text should survive wrapping: {script:?}"
    );
    ensure!(
        !script.contains(LIST_VALUE),
        "the raw value leaked into the entry unquoted: {script:?}"
    );
    Ok(())
}

/// A command-list entry executes to the intended argument.
#[rstest]
#[case::posix(RecipeShell::Posix)]
#[case::bash(RecipeShell::Bash)]
fn command_list_recipes_execute_to_the_intended_argument(#[case] shell: RecipeShell) -> Result<()> {
    let Some(interpreter) = posix_shell() else {
        eprintln!("skipped: no POSIX shell on this host");
        return Ok(());
    };
    let source = list_manifest(
        &format!("printf '{SENTINEL}%s\\n' {{{{ seam_value | shell_quote }}}}"),
        LIST_VALUE,
    );
    let ninja = generate(&source, shell)?;
    let binding = command_binding(&ninja)?;
    let script = match shell {
        RecipeShell::Bash => decode_bash_transport(&binding)?,
        RecipeShell::Posix | RecipeShell::PowerShell => binding.clone(),
    }
    .replace("$$", "$");

    let observed = run_posix_shell(&interpreter, &script)?;
    ensure!(
        observed == format!("{SENTINEL}{LIST_VALUE}\n"),
        "the interpreter read {observed:?} from {script:?}, expected the sentinel line"
    );
    Ok(())
}

/// `shell_join` reaches the lowered recipe as one argument per element.
///
/// The join filter's promise is a word list, so the composed assertion is that
/// the interpreter sees *several* arguments, not one concatenated string. A
/// join that quoted the whole list — or that quoted nothing — would fail here
/// while passing every isolated comparison.
#[rstest]
fn joined_output_lowers_to_separate_arguments() -> Result<()> {
    let Some(interpreter) = posix_shell() else {
        eprintln!("skipped: no POSIX shell on this host");
        return Ok(());
    };
    let values = ["alpha", "beta gamma", "delta"];
    let yaml_list = values
        .iter()
        .map(|value| format!("    - \"{value}\"\n"))
        .collect::<String>();
    let source = format!(
        "netsuke_version: \"1.0.0\"\nvars:\n  parts:\n{yaml_list}targets:\n  - name: out\n    command: |\n      printf '{SENTINEL}%s\\n' {{{{ parts | shell_join }}}}\n    description: composition\n",
    );
    let ninja = generate(&source, RecipeShell::Posix)?;
    let script = command_binding(&ninja)?.replace("$$", "$");

    let observed = run_posix_shell(&interpreter, &script)?;
    let expected = values
        .iter()
        .map(|value| format!("{SENTINEL}{value}\n"))
        .collect::<String>();
    ensure!(
        observed == expected,
        "the interpreter read {observed:?} from {script:?}, expected {expected:?}"
    );
    Ok(())
}

/// A value the filter refuses is refused *before* generation, not at the shell.
///
/// The composition has a boundary, and this is it: a value the loader cannot
/// represent raises a Netsuke error naming the construct rather than reaching
/// an interpreter. The control asserts on the error text so it fails if the
/// refusal ever becomes a silent pass-through.
#[rstest]
fn an_inadmissible_value_is_refused_before_generation() -> Result<()> {
    // A line feed cannot live in one Ninja command binding, and the filter
    // refuses it at the template rather than letting the renderer discover it.
    let source = manifest_with(
        &format!("command: |\n  printf '%s\\n' {{{{ seam_value | shell_quote }}}}\n"),
        "line one\nline two",
    );
    match generate(&source, RecipeShell::Posix) {
        Ok(ninja) => bail!("a line-feed-bearing value should not generate Ninja: {ninja}"),
        Err(error) => {
            let text = format!("{error:#}");
            ensure!(
                text.contains("netsuke::jinja::shell::unquotable"),
                "the refusal should carry the unquotable code: {text}"
            );
            // The message is filter-agnostic by design — the code above is
            // what identifies the source — but it must still tell the author
            // which construct was refused.
            ensure!(
                text.contains("line feed"),
                "the refusal should name the offending construct: {text}"
            );
        }
    }
    Ok(())
}
