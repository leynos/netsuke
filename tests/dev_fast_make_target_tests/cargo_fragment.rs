//! What the `dev-fast` Cargo fragment means, and that Cargo agrees.
//!
//! The Make target contracts next door assert that each recipe passes the
//! fragment by path; they cannot say whether the file says what it is meant to.
//! These tests close that gap from both ends: one asks Cargo for its own
//! resolved view of the configuration, and the other reads the fragment
//! directly.
//!
//! Neither exercises code generation. Proving Cranelift and `mold` are
//! genuinely used needs both installed, which `make dev-fast-check` gates at
//! the point of use.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::process::Command;
use test_support::dev_fast::{DEV_FAST_CONFIG_PATH, dev_fast_config};

/// Ask Cargo what the fragment means, rather than only what it contains.
///
/// Parsing the TOML proves the file is well-formed; it cannot prove Cargo
/// accepts the key paths. A backend nested under the wrong table, or a
/// misspelled key, parses perfectly and is then silently ignored. `cargo config
/// get` reports Cargo's own resolved view, so a misplaced key shows up as a
/// missing value instead of passing unnoticed.
///
/// The Linux entry is queried through its parent table: `config get` cannot
/// address a key whose name is a quoted `cfg` expression.
#[rstest]
#[case::dev_profile_backend(
    "profile.dev.codegen-backend",
    "profile.dev.codegen-backend = \"cranelift\""
)]
#[case::unstable_flag("unstable.codegen-backend", "unstable.codegen-backend = true")]
#[case::linux_rustflags(
    "target",
    "target.'cfg(target_os = \"linux\")'.rustflags = [\"-Clink-arg=-fuse-ld=mold\"]"
)]
fn cargo_resolves_the_fragment_to_the_intended_settings(
    #[case] query: &str,
    #[case] expected: &str,
) -> Result<()> {
    let output = Command::new(env!("CARGO"))
        .args(["--config", DEV_FAST_CONFIG_PATH, "-Zunstable-options"])
        .args(["config", "get", query])
        .output()
        .with_context(|| format!("ask cargo for {query}"))?;
    let reported = String::from_utf8_lossy(&output.stdout);

    ensure!(
        output.status.success(),
        "cargo should resolve `{query}`, got `{}`",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        reported.contains(expected),
        "cargo should report `{expected}`, got `{reported}`"
    );
    Ok(())
}

/// The recipes pass the fragment by path, so a test asserting only that the
/// path appears would still pass if the file lost its contents. Read it.
#[test]
fn the_cargo_fragment_selects_cranelift_and_mold() -> Result<()> {
    let fragment: toml::Value = toml::from_str(&dev_fast_config()?)?;

    ensure!(
        fragment
            .get("unstable")
            .and_then(|table| table.get("codegen-backend"))
            == Some(&toml::Value::Boolean(true)),
        "the fragment must enable the codegen-backend unstable flag, got `{fragment}`"
    );
    ensure!(
        fragment
            .get("profile")
            .and_then(|table| table.get("dev"))
            .and_then(|table| table.get("codegen-backend"))
            .and_then(toml::Value::as_str)
            == Some("cranelift"),
        "the dev profile must select Cranelift, got `{fragment}`"
    );
    // Release artefacts must never inherit the backend, so no other profile may
    // name one.
    let profiles = fragment
        .get("profile")
        .and_then(toml::Value::as_table)
        .context("the fragment must configure a profile")?;
    for (name, table) in profiles {
        ensure!(
            name == "dev" || table.get("codegen-backend").is_none(),
            "only the dev profile may select a backend, but `{name}` does"
        );
    }

    let linux = fragment
        .get("target")
        .and_then(|table| table.get(r#"cfg(target_os = "linux")"#))
        .and_then(|table| table.get("rustflags"))
        .and_then(toml::Value::as_array)
        .context("the fragment must gate rustflags behind the Linux cfg")?;
    ensure!(
        linux
            .iter()
            .filter_map(toml::Value::as_str)
            .any(|flag| flag.contains("-fuse-ld=mold")),
        "the Linux target must select mold, got `{linux:?}`"
    );
    Ok(())
}
