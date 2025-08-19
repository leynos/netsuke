//! Tests for environment variable access via the Jinja `env()` helper.

use netsuke::{ast::Recipe, manifest};
use rstest::rstest;
use serial_test::serial;
use std::ffi::OsStr;
use test_support::env::VarGuard;

fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}

#[rstest]
#[case("NETSUKE_ENV_TEST", "world", "echo world")]
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
) {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: hello\n    command: \"echo {{{{ env('{env_key}') }}}}\"\n"
    ));
    let _guard = VarGuard::set(env_key, OsStr::new(env_value));
    let manifest = manifest::from_str(&yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, expected_command);
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
}

#[rstest]
#[serial]
fn missing_env_var_errors() {
    let key = "NETSUKE_ENV_MISSING";
    let _guard = VarGuard::unset(key);
    let yaml = manifest_yaml(
        "targets:\n  - name: hello\n    command: \"echo {{ env('NETSUKE_ENV_MISSING') }}\"\n",
    );
    assert!(manifest::from_str(&yaml).is_err());
}

#[cfg(unix)]
#[rstest]
#[serial]
fn non_unicode_env_var_errors() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let key = "NETSUKE_ENV_NON_UNICODE";
    let yaml = manifest_yaml(
        "targets:\n  - name: hello\n    command: \"echo {{ env('NETSUKE_ENV_NON_UNICODE') }}\"\n",
    );

    // Construct a non-UTF-8 value: [0x66, 0xFF, 0x6F] ~= "f" + invalid + "o"
    let val = OsString::from_vec(vec![0x66, 0xFF, 0x6F]);
    let _guard = VarGuard::set(key, val.as_os_str());

    let err = manifest::from_str(&yaml).expect_err("parse should fail on non-UTF-8");
    assert!(
        err.chain()
            .any(|e| e.to_string().to_lowercase().contains("invalid utf-8")),
        "unexpected error: {err}"
    );
}
