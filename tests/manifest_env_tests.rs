//! Tests for environment variable access via the Jinja `env()` helper.

use netsuke::{ast::Recipe, manifest};
use rstest::rstest;
use serial_test::serial;
use std::ffi::OsStr;
use test_support::env_lock::EnvLock;

fn set_var(key: &str, value: &OsStr) {
    // SAFETY: `EnvLock` serialises mutations.
    unsafe { std::env::set_var(key, value) };
}

fn remove_var(key: &str) {
    // SAFETY: `EnvLock` serialises mutations.
    unsafe { std::env::remove_var(key) };
}

fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}

#[rstest]
#[serial]
fn env_var_renders() {
    let key = "NETSUKE_ENV_TEST";
    let yaml = manifest_yaml(
        "targets:\n  - name: hello\n    command: \"echo {{ env('NETSUKE_ENV_TEST') }}\"\n",
    );
    {
        let _lock = EnvLock::acquire();
        set_var(key, OsStr::new("world"));
    }
    let manifest = manifest::from_str(&yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, "echo world");
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
    {
        let _lock = EnvLock::acquire();
        remove_var(key);
    }
}

#[rstest]
#[serial]
fn env_var_with_special_chars_renders() {
    let key = "NETSUKE_ENV_SPECIAL";
    let value = "spaced value $with #symbols";
    let yaml = manifest_yaml(
        "targets:\n  - name: hello\n    command: \"echo {{ env('NETSUKE_ENV_SPECIAL') }}\"\n",
    );
    {
        let _lock = EnvLock::acquire();
        set_var(key, OsStr::new(value));
    }
    let manifest = manifest::from_str(&yaml).expect("parse");
    let first = manifest.targets.first().expect("target");
    if let Recipe::Command { command } = &first.recipe {
        assert_eq!(command, &format!("echo {value}"));
    } else {
        panic!("Expected command recipe, got: {:?}", first.recipe);
    }
    {
        let _lock = EnvLock::acquire();
        remove_var(key);
    }
}

#[rstest]
#[serial]
fn missing_env_var_errors() {
    let key = "NETSUKE_ENV_MISSING";
    {
        let _lock = EnvLock::acquire();
        remove_var(key);
    }
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

    {
        let _lock = EnvLock::acquire();
        // Construct a non-UTF-8 value: [0x66, 0xFF, 0x6F] ~= "f" + invalid + "o"
        let val = OsString::from_vec(vec![0x66, 0xFF, 0x6F]);
        set_var(key, val.as_os_str());
    }

    let err = manifest::from_str(&yaml).expect_err("parse should fail on non-UTF-8");
    assert!(
        err.chain()
            .any(|e| e.to_string().to_lowercase().contains("invalid utf-8")),
        "unexpected error: {err}"
    );

    {
        let _lock = EnvLock::acquire();
        remove_var(key);
    }
}
