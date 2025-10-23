//! Tests for environment variable access via the Jinja `env()` helper.

use anyhow::{Context, Result, anyhow, ensure};
use netsuke::{ast::Recipe, manifest};
use rstest::rstest;
use serial_test::serial;
use std::ffi::OsStr;
use test_support::{env::VarGuard, manifest::manifest_yaml};

#[rstest]
#[case("NETSUKE_ENV_TEST", "world", "echo world")]
#[case("NETSUKE_ENV_EMPTY", "", "echo ")]
#[case(
    "NETSUKE_ENV_SPECIAL",
    "spaced value $with #symbols",
    "echo spaced value $with #symbols"
)]
#[serial]
fn env_var_renders_parameterized(
    #[case] env_key: &str,
    #[case] env_value: &str,
    #[case] expected_command: &str,
) -> Result<()> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: hello\n    command: \"echo {{{{ env('{env_key}') }}}}\"\n"
    ));
    let _guard = VarGuard::set(env_key, OsStr::new(env_value));
    let manifest = manifest::from_str(&yaml)?;
    let first = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    let Recipe::Command { command } = &first.recipe else {
        return Err(anyhow!("expected command recipe, got {:?}", first.recipe));
    };
    ensure!(
        command == expected_command,
        "expected command {expected_command}, got {command}"
    );
    Ok(())
}

#[rstest]
#[serial]
fn missing_env_var_errors() -> Result<()> {
    let key = "NETSUKE_ENV_MISSING";
    let _guard = VarGuard::unset(key);
    let yaml = manifest_yaml(
        "targets:\n  - name: hello\n    command: \"echo {{ env('NETSUKE_ENV_MISSING') }}\"\n",
    );
    match manifest::from_str(&yaml) {
        Ok(manifest) => Err(anyhow!(
            "expected missing env var to error, rendered manifest: {manifest:?}"
        )),
        Err(err) => {
            ensure!(
                err.chain()
                    .any(|e| e.to_string().to_lowercase().contains("is not set")),
                "unexpected error: {err}"
            );
            Ok(())
        }
    }
}

#[cfg(unix)]
#[rstest]
#[serial]
fn non_unicode_env_var_errors() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let key = "NETSUKE_ENV_NON_UNICODE";
    let yaml = manifest_yaml(
        "targets:\n  - name: hello\n    command: \"echo {{ env('NETSUKE_ENV_NON_UNICODE') }}\"\n",
    );

    // Construct a non-UTF-8 value: [0x66, 0xFF, 0x6F] ~= "f" + invalid + "o"
    let val = OsString::from_vec(vec![0x66, 0xFF, 0x6F]);
    let _guard = VarGuard::set(key, val.as_os_str());

    match manifest::from_str(&yaml) {
        Ok(manifest) => Err(anyhow!(
            "expected non-UTF-8 env var to error, rendered manifest: {manifest:?}"
        )),
        Err(err) => {
            ensure!(
                err.chain()
                    .any(|e| e.to_string().to_lowercase().contains("invalid utf-8")),
                "unexpected error: {err}"
            );
            Ok(())
        }
    }
}
