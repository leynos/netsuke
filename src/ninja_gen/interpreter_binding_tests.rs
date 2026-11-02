//! Verifies that rendered interpreter commands carry a trusted binding.
//!
//! Windows resolves an unqualified executable name through its search order,
//! so a plan that names `powershell.exe` can be shadowed by a workspace file.
//! These tests pin the renderer contract: a bound shell always emits a quoted
//! absolute executable, an unbound shell refuses to emit a command, and the
//! response-file transport keeps Ninja's `$rspfile` reference intact.

use super::{
    InterpreterBinding, MAX_POWER_SHELL_COMMAND_LINE, POWER_SHELL_ENCODED_ARGUMENTS,
    POWER_SHELL_RESPONSE_FILE_ARGUMENTS, RecipeShell, RenderedRecipeCommand,
    encode_power_shell_script, windows_argument,
};
use crate::ninja_gen::NinjaGenError;
use crate::ninja_gen::escape::ShellText;
use crate::ninja_gen::test_support::power_shell_interpreter;
use crate::recipe_shell::{InterpreterExecutable, InterpreterExecutableError};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use camino::Utf8PathBuf;
use rstest::rstest;

/// Build a PowerShell fixture whose path contains a space, Unicode and `$`.
fn adorned_power_shell_executable() -> InterpreterExecutable {
    let path = if cfg!(windows) {
        r"C:\Program Files\Netsuke \u{03bb}\$PowerShell\v1.0\powershell.exe"
    } else {
        "/fixtures/Program Files/Netsuke \u{03bb}/$PowerShell/v1.0/powershell.exe"
    };
    bound_executable(path)
}

/// Construct one executable from a platform absolute fixture path.
fn bound_executable(path: &str) -> InterpreterExecutable {
    InterpreterExecutable::from_absolute_path(Utf8PathBuf::from(path))
        .expect("fixture interpreter path should be an absolute non-NUL path")
}

/// Build a fixture whose executable path has `dollars` literal `$` characters.
fn dollar_laden_executable(dollars: usize) -> InterpreterExecutable {
    let path = if cfg!(windows) {
        format!(r"C:\netsuke\{}\powershell.exe", "$".repeat(dollars))
    } else {
        format!("/netsuke/{}/powershell.exe", "$".repeat(dollars))
    };
    bound_executable(&path)
}

/// Repeat a path segment until the fixture path reaches `length` bytes.
fn long_executable(length: usize) -> InterpreterExecutable {
    let prefix = if cfg!(windows) { r"C:\" } else { "/" };
    let suffix = if cfg!(windows) {
        r"\powershell.exe"
    } else {
        "/powershell.exe"
    };
    let repeat = length.saturating_sub(prefix.len() + suffix.len());
    bound_executable(&format!("{prefix}{}{suffix}", "a".repeat(repeat)))
}

/// Rebuild the script text that the renderer encodes for one recipe.
fn encoded_payload(script: &str) -> String {
    let wrapped = format!(
        "$ErrorActionPreference = 'Stop'\n$LASTEXITCODE = 0\n{script}\nif ($LASTEXITCODE -ne 0) {{ exit $LASTEXITCODE }}"
    );
    encode_power_shell_script(&wrapped)
}

/// Decode one UTF-16LE Base64 payload back into script text.
fn decode_payload(payload: &str) -> String {
    let bytes = STANDARD.decode(payload).expect("payload should be Base64");
    let (pairs, remainder) = bytes.as_chunks::<2>();
    assert!(remainder.is_empty(), "payload should be UTF-16LE");
    let units = pairs
        .iter()
        .map(|[low, high]| u16::from(*low) | (u16::from(*high) << 8))
        .collect::<Vec<_>>();
    String::from_utf16(&units).expect("payload should be valid UTF-16")
}

/// Render one scalar recipe with the supplied binding.
fn render_scalar(
    script: &str,
    binding: InterpreterBinding<'_>,
) -> Result<RenderedRecipeCommand, NinjaGenError> {
    RecipeShell::PowerShell.command_value_with_interpreter(&ShellText::new(script.into()), binding)
}

/// The parent executable is quoted, absolute and Ninja-escaped.
#[test]
fn bound_power_shell_parent_is_a_quoted_absolute_executable() {
    let executable = adorned_power_shell_executable();
    let rendered = render_scalar("Write-Output ready", InterpreterBinding::Bound(&executable))
        .expect("bound PowerShell rendering should succeed")
        .command()
        .to_string();

    let escaped_path = windows_argument(executable.as_path().as_str()).replace('$', "$$");
    assert!(
        rendered.starts_with(&escaped_path),
        "parent should start with the bound executable: {rendered}"
    );
    assert!(
        rendered.contains("$$PowerShell"),
        "literal dollars in the path should be Ninja-escaped: {rendered}"
    );
    assert!(
        !rendered.contains("/$PowerShell"),
        "the plan should not carry a bare Ninja variable: {rendered}"
    );
    let payload = rendered
        .strip_prefix(&format!("{escaped_path}{POWER_SHELL_ENCODED_ARGUMENTS}"))
        .expect("rendered command should carry the fixed arguments");
    assert_eq!(
        decode_payload(payload),
        "$ErrorActionPreference = 'Stop'\n$LASTEXITCODE = 0\nWrite-Output ready\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"
    );
}

/// Scalar, script, shared-list and isolated-list recipes keep their payloads.
#[rstest]
#[case::scalar("Write-Output scalar")]
#[case::script("$value = 'line one'\nWrite-Output $value")]
fn bound_power_shell_payloads_are_unchanged(#[case] recipe: &str) {
    let executable = power_shell_interpreter();
    let rendered = render_scalar(recipe, InterpreterBinding::Bound(&executable))
        .expect("bound PowerShell rendering should succeed");
    let command = rendered.command().to_string();
    let prefix = format!(
        "{}{POWER_SHELL_ENCODED_ARGUMENTS}",
        windows_argument(executable.as_path().as_str())
    );
    let payload = command
        .strip_prefix(&prefix)
        .expect("rendered command should carry an encoded payload");
    assert_eq!(
        decode_payload(payload),
        decode_payload(&encoded_payload(recipe))
    );
}

/// Shared command lists render their joined script through the same binding.
#[test]
fn bound_power_shell_shared_lists_keep_every_entry() {
    let executable = power_shell_interpreter();
    let script = RecipeShell::PowerShell
        .command_list_script(&["Write-Output first".into(), "Write-Output second".into()])
        .expect("PowerShell should render command-list scripts");
    let rendered = render_scalar(&script, InterpreterBinding::Bound(&executable))
        .expect("bound shared-list rendering should succeed");
    let command = rendered.command().to_string();
    assert!(command.starts_with(&windows_argument(executable.as_path().as_str())));
    let payload = command
        .rsplit_once(POWER_SHELL_ENCODED_ARGUMENTS)
        .map(|(_, payload)| payload)
        .expect("parent should carry an encoded payload");
    let decoded = decode_payload(payload);
    assert!(decoded.contains("Write-Output first"));
    assert!(decoded.contains("Write-Output second"));
}

/// Isolated lists bind the parent but keep the child's `$PSHOME` launch.
#[test]
fn bound_isolated_power_shell_parent_keeps_pshome_children() {
    let executable = power_shell_interpreter();
    let script = RecipeShell::PowerShell
        .isolated_command_list_script(&["Write-Output first".into(), "Write-Output second".into()])
        .expect("isolated list should render")
        .expect("PowerShell route should be available");
    let rendered = RecipeShell::PowerShell
        .isolated_command_list_value(
            &ShellText::new(script),
            InterpreterBinding::Bound(&executable),
        )
        .expect("bound isolated-list rendering should succeed");
    let command = rendered.command().to_string();
    assert!(command.starts_with(&windows_argument(executable.as_path().as_str())));
    let payload = command
        .rsplit_once(POWER_SHELL_ENCODED_ARGUMENTS)
        .map(|(_, payload)| payload)
        .expect("parent should carry an encoded payload");
    let decoded = decode_payload(payload);
    assert_eq!(
        decoded
            .matches("$netsukeChildExecutable = \"$PSHOME\\powershell.exe\"")
            .count(),
        2
    );
    assert!(!command.contains("$PSHOME"));
}

/// An unbound interpreter refuses to render PowerShell or Bash commands.
#[rstest]
#[case::power_shell(RecipeShell::PowerShell)]
#[case::bash(RecipeShell::Bash)]
fn unbound_interpreter_bindings_reject_command_rendering(#[case] shell: RecipeShell) {
    let result = shell.command_value_with_interpreter(
        &ShellText::new("Write-Output ready".into()),
        InterpreterBinding::Unbound,
    );
    assert!(matches!(
        result,
        Err(NinjaGenError::InterpreterBindingRequired { shell: rejected }) if rejected == shell
    ));
}

/// POSIX rendering ignores the binding because Ninja launches the host shell.
#[test]
fn posix_rendering_ignores_the_interpreter_binding() {
    let executable = power_shell_interpreter();
    let rendered = RecipeShell::Posix
        .command_value_with_interpreter(
            &ShellText::new("echo ready".into()),
            InterpreterBinding::Bound(&executable),
        )
        .expect("POSIX rendering should succeed")
        .command()
        .to_string();
    assert_eq!(rendered, "echo ready");
}

/// The response file keeps the payload and its cleanup step unchanged.
#[test]
fn power_shell_response_file_keeps_payload_and_cleanup() {
    let executable = power_shell_interpreter();
    let recipe = "x".repeat(12_500);
    let rendered = render_scalar(&recipe, InterpreterBinding::Bound(&executable))
        .expect("oversized recipes should use a response file");
    assert!(matches!(
        &rendered,
        RenderedRecipeCommand::ResponseFile { .. }
    ));
    let expected_command = format!(
        "{}{POWER_SHELL_RESPONSE_FILE_ARGUMENTS}",
        windows_argument(executable.as_path().as_str())
    );
    assert_eq!(rendered.command().to_string(), expected_command);
    // Ninja expands `$rspfile` itself, so the binding must carry the single
    // dollar form; escaping it would pass a literal `$rspfile` to PowerShell.
    assert!(expected_command.contains("-File \"$rspfile\""));

    let content = rendered
        .response_file_content()
        .expect("response file should carry a bootstrap script")
        .to_string();
    let payload = encoded_payload(&recipe);
    assert!(
        content.contains(&payload),
        "response file should carry the unchanged Base64 payload"
    );
    assert!(content.contains("Remove-Item -LiteralPath $$PSCommandPath -Force"));
    assert!(content.contains("$$netsukePayload"));
}

/// Length accounting happens before Ninja escaping, so `$$` counts once.
#[test]
fn power_shell_length_uses_unescaped_utf16_units() {
    let executable = dollar_laden_executable(2_000);
    let path = executable.as_path().as_str();
    let unescaped_prefix = format!("{}{POWER_SHELL_ENCODED_ARGUMENTS}", windows_argument(path));
    let escaped_prefix = format!(
        "{}{POWER_SHELL_ENCODED_ARGUMENTS}",
        windows_argument(path).replace('$', "$$")
    );
    let total = |recipe_length: usize| {
        unescaped_prefix.encode_utf16().count().saturating_add(
            encoded_payload(&"x".repeat(recipe_length))
                .encode_utf16()
                .count(),
        )
    };
    let mut low = 0usize;
    let mut high = 64_000usize;
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if total(middle) <= MAX_POWER_SHELL_COMMAND_LINE {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let recipe = "x".repeat(low);
    assert!(total(low) <= MAX_POWER_SHELL_COMMAND_LINE);
    assert!(escaped_prefix.encode_utf16().count() > unescaped_prefix.encode_utf16().count());

    let rendered = render_scalar(&recipe, InterpreterBinding::Bound(&executable))
        .expect("recipe within the unescaped limit should render directly");
    assert!(matches!(rendered, RenderedRecipeCommand::Direct(_)));
}

/// A long bound executable path pushes the recipe onto the response file.
#[test]
fn long_bound_executable_selects_the_response_file_transport() {
    let executable = long_executable(32_600);
    let rendered = render_scalar("Write-Output ready", InterpreterBinding::Bound(&executable))
        .expect("long executable paths should still render");
    assert!(matches!(
        &rendered,
        RenderedRecipeCommand::ResponseFile { .. }
    ));
    assert!(rendered.command().to_string().encode_utf16().count() <= MAX_POWER_SHELL_COMMAND_LINE);
}

/// A response-file parent that still exceeds the limit reports a semantic error.
#[test]
fn oversized_response_file_parent_reports_a_semantic_error() {
    let executable = long_executable(33_000);
    let result = render_scalar("Write-Output ready", InterpreterBinding::Bound(&executable));
    assert!(matches!(
        result,
        Err(NinjaGenError::PowerShellParentCommandLineTooLong {
            actual_characters,
            maximum_characters: MAX_POWER_SHELL_COMMAND_LINE,
        }) if actual_characters > MAX_POWER_SHELL_COMMAND_LINE
    ));
}

/// Construction still rejects relative and NUL-bearing interpreter paths.
#[test]
fn interpreter_executable_rejects_unsafe_paths() {
    let relative = InterpreterExecutable::from_absolute_path(Utf8PathBuf::from("bin/bash"));
    assert!(matches!(
        relative,
        Err(InterpreterExecutableError::RelativePath { .. })
    ));
    let nul = InterpreterExecutable::from_absolute_path(if cfg!(windows) {
        Utf8PathBuf::from("C:\\bad\0\\powershell.exe")
    } else {
        Utf8PathBuf::from("/bad\0/powershell")
    });
    assert!(matches!(nul, Err(InterpreterExecutableError::NulCharacter)));
}
