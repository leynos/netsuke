//! Tests for PowerShell-specific Ninja recipe rendering.

use super::*;

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Render one oversized PowerShell recipe and assert Ninja owns its response-file lifecycle.
fn assert_large_recipe_uses_ninja_response_file(
    recipe: Recipe,
    expected_script: &str,
) -> Result<()> {
    let action = Action {
        recipe,
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let mut rendered = String::new();
    NamedAction {
        id: "large_power_shell",
        action: &action,
        shell: RecipeShell::PowerShell,
    }
    .write_into(&mut rendered)?;
    ensure!(rendered.contains("-File \"$rspfile\""));
    ensure!(!rendered.contains("-EncodedCommand"));
    ensure!(rendered.contains("rspfile = $out.netsuke-large_power_shell.ps1"));
    ensure!(rendered.contains("rspfile_content = "));
    ensure!(!rendered.contains(expected_script));
    let response_file_script = rendered
        .lines()
        .find_map(|line| line.strip_prefix("  rspfile_content = "))
        .context("oversized PowerShell command should provide Ninja response-file content")?;
    let script = decode_response_file_payload(response_file_script)?;
    ensure!(script.starts_with("$ErrorActionPreference = 'Stop'\n"));
    ensure!(script.contains(expected_script));
    ensure!(
        rendered.contains("Netsuke could not decode the PowerShell response file"),
        "the response-file bootstrap must report decoding failures clearly"
    );
    ensure!(
        rendered.contains("if ($$LASTEXITCODE -ne 0) { exit $$LASTEXITCODE }"),
        "the response-file bootstrap must propagate the decoded script's native failure"
    );
    ensure!(
        rendered.contains(". ([ScriptBlock]::Create($$netsukeScript))"),
        "the response-file bootstrap must run the decoded script in its scope"
    );
    ensure!(
        rendered.contains("Remove-Item -LiteralPath $$PSCommandPath -Force"),
        "the response-file bootstrap must clean itself after any recipe outcome"
    );
    Ok(())
}

/// Verify that oversized scalar, script, and list recipes use Ninja response files.
#[test]
fn large_power_shell_recipes_use_ninja_response_files() -> Result<()> {
    let padding = "x".repeat(12_500);
    assert_large_recipe_uses_ninja_response_file(
        Recipe::Command {
            command: StringOrList::String(format!("# {padding}\nWrite-Output large-scalar")),
        },
        "Write-Output large-scalar",
    )?;
    assert_large_recipe_uses_ninja_response_file(
        Recipe::Script {
            script: format!("# {padding}\nWrite-Output large-script"),
        },
        "Write-Output large-script",
    )?;
    assert_large_recipe_uses_ninja_response_file(
        Recipe::Command {
            command: StringOrList::List(vec![
                format!("# {padding}\n$env:NETSUKE_LARGE = 'shared'"),
                "if ($env:NETSUKE_LARGE -ne 'shared') { exit 1 }; Write-Output large-list".into(),
            ]),
        },
        "Write-Output large-list",
    )
}

/// Verify that distinct rules receive distinct Ninja-owned response-file names.
#[test]
fn large_power_shell_rules_use_distinct_response_files() -> Result<()> {
    let action = Action {
        recipe: Recipe::Command {
            command: StringOrList::String("x".repeat(12_500)),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let mut first = String::new();
    NamedAction {
        id: "first_large_power_shell",
        action: &action,
        shell: RecipeShell::PowerShell,
    }
    .write_into(&mut first)?;
    let mut second = String::new();
    NamedAction {
        id: "second_large_power_shell",
        action: &action,
        shell: RecipeShell::PowerShell,
    }
    .write_into(&mut second)?;
    ensure!(first.contains("rspfile = $out.netsuke-first_large_power_shell.ps1"));
    ensure!(second.contains("rspfile = $out.netsuke-second_large_power_shell.ps1"));
    Ok(())
}

/// Verify that PowerShell command lists preserve state and native failure checks.
#[test]
fn power_shell_command_lists_preserve_state_and_stop_on_native_failure() -> Result<()> {
    let action = command_action(StringOrList::List(vec![
        "$env:NETSUKE_ORDER = 'first'".into(),
        "if ($env:NETSUKE_ORDER -ne 'first') { exit 9 }; cmd.exe /c exit 0".into(),
    ]));
    let mut rendered = String::new();
    NamedAction {
        id: "power_shell_list",
        action: &action,
        shell: RecipeShell::PowerShell,
    }
    .write_into(&mut rendered)?;
    let encoded = rendered
        .lines()
        .find_map(|line| line.split("-EncodedCommand ").nth(1))
        .context("PowerShell command should include an encoded script")?
        .trim();
    let script = decode_power_shell_script(encoded)?;
    ensure!(script.contains("$env:NETSUKE_ORDER = 'first'"));
    ensure!(script.contains("$LASTEXITCODE = 0"));
    ensure!(script.contains("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"));
    ensure!(
        !rendered.contains("NETSUKE_ORDER"),
        "Ninja must not parse the PowerShell variable expression: {rendered}"
    );
    Ok(())
}

/// Verify that PowerShell lowering preserves native escaped-quote syntax and path literals.
#[test]
fn power_shell_recipes_quote_apostrophe_paths_with_power_shell_literals() -> Result<()> {
    let manifest = crate::manifest::render_manifest(
        crate::manifest::from_str(
            "netsuke_version: '1.0.0'\ntargets:\n  - name: output's-file\n    sources: source's-file\n    command: 'Write-Output \"a`\"b\"; Copy-Item {{ ins }} {{ outs }}'\n",
        )?,
        &minijinja::Environment::new(),
    )?;
    let graph = BuildGraph::from_manifest_for_shell(&manifest, RecipeShell::PowerShell)?;
    let mut rendered = String::new();
    generate_into_with_shell(&graph, &mut rendered, RecipeShell::PowerShell)?;
    let encoded = rendered
        .lines()
        .find_map(|line| line.split("-EncodedCommand ").nth(1))
        .context("PowerShell command should include an encoded script")?
        .trim();
    let script = decode_power_shell_script(encoded)?;
    ensure!(
        script.contains("Copy-Item 'source''s-file' 'output''s-file'"),
        "PowerShell must receive apostrophe-safe path literals:\n{script}"
    );
    ensure!(
        script.contains("Write-Output \"a`\"b\""),
        "PowerShell lowering must preserve native escaped double quotes:\n{script}"
    );
    Ok(())
}

/// Verify that PowerShell scripts bypass the POSIX script wrapper.
#[test]
fn power_shell_scripts_do_not_use_the_posix_script_wrapper() -> Result<()> {
    let action = Action {
        recipe: Recipe::Script {
            script: "$edition = $PSVersionTable.PSEdition\nWrite-Output $edition".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let mut rendered = String::new();
    NamedAction {
        id: "power_shell_script",
        action: &action,
        shell: RecipeShell::PowerShell,
    }
    .write_into(&mut rendered)?;
    let encoded = rendered
        .split("-EncodedCommand ")
        .nth(1)
        .context("PowerShell command should include an encoded script")?
        .trim();
    let script = decode_power_shell_script(encoded)?;
    ensure!(script.contains("$edition = $PSVersionTable.PSEdition"));
    ensure!(script.contains("Write-Output $edition"));
    ensure!(
        !script.contains("/bin/sh"),
        "PowerShell script must not traverse the POSIX script wrapper: {script}"
    );
    Ok(())
}

/// Decode the UTF-16LE PowerShell payload emitted in a Ninja command binding.
fn decode_power_shell_script(encoded: &str) -> Result<String> {
    let bytes = STANDARD
        .decode(encoded)
        .context("decode PowerShell command payload")?;
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        bail!("PowerShell UTF-16 payload must contain an even number of bytes");
    }
    let units = pairs
        .iter()
        .map(|[low, high]| u16::from(*low) | (u16::from(*high) << 8))
        .collect::<Vec<_>>();
    String::from_utf16(&units).context("decode PowerShell UTF-16 payload")
}

/// Decode the Base64 UTF-16LE recipe payload embedded in a Ninja response script.
fn decode_response_file_payload(response_file_script: &str) -> Result<String> {
    let bootstrap = response_file_script.replace("$$", "$");
    let encoded = bootstrap
        .strip_prefix("$netsukePayload = '")
        .and_then(|content| {
            content.strip_suffix(
                "'; $netsukeScript = try { [Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($netsukePayload)) } catch { throw \"Netsuke could not decode the PowerShell response file: $($_.Exception.Message)\" }; try { . ([ScriptBlock]::Create($netsukeScript)) } finally { Remove-Item -LiteralPath $PSCommandPath -Force }; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }",
            )
        })
        .context("response file should contain the PowerShell Base64 bootstrap")?;
    decode_power_shell_script(encoded)
}

/// Verify that malformed odd-length UTF-16LE PowerShell payloads are rejected.
#[test]
fn decode_power_shell_script_rejects_odd_length_payloads() {
    let encoded = STANDARD.encode([0x41]);
    assert!(decode_power_shell_script(&encoded).is_err());
}
