//! Tests for PowerShell-specific Ninja recipe rendering.

use super::*;

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};

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
    if bytes.len().rem_euclid(2) != 0 {
        bail!("PowerShell UTF-16 payload must contain an even number of bytes");
    }
    let units = bytes
        .chunks_exact(2)
        .map(|pair| {
            let [low, high]: [u8; 2] = pair
                .try_into()
                .context("PowerShell UTF-16 unit must contain two bytes")?;
            Ok(u16::from(low) | (u16::from(high) << 8))
        })
        .collect::<Result<Vec<_>>>()?;
    String::from_utf16(&units).context("decode PowerShell UTF-16 payload")
}

/// Verify that malformed odd-length UTF-16LE PowerShell payloads are rejected.
#[test]
fn decode_power_shell_script_rejects_odd_length_payloads() {
    let encoded = STANDARD.encode([0x41]);
    assert!(decode_power_shell_script(&encoded).is_err());
}
