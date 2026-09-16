//! Contracts for the committed `.cargo/config.toml`.
//!
//! The recipes are covered by `build_tools_make_target_tests`; this suite is about
//! the file Cargo auto-discovers, which is what makes the build standard the
//! default for a bare `cargo` invocation rather than something a Make target
//! has to opt into.
#![cfg(all(unix, target_os = "linux"))]

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::process::Command;
use test_support::build_tools::cargo_config;

/// `[build] rustflags` and the Linux table must carry the same non-linker
/// flags, because Cargo replaces one source with the other rather than merging
/// them: a flag named only in `[build]` silently vanishes on Linux.
#[test]
fn the_configuration_repeats_every_shared_flag_in_both_rustflags_sources() -> Result<()> {
    let config: toml::Value = toml::from_str(&cargo_config()?)?;
    let read = |value: Option<&toml::Value>| -> Vec<String> {
        value
            .and_then(toml::Value::as_array)
            .map(|array| {
                array
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let build = read(config.get("build").and_then(|table| table.get("rustflags")));
    let linux = read(
        config
            .get("target")
            .and_then(|table| table.get(r#"cfg(target_os = "linux")"#))
            .and_then(|table| table.get("rustflags")),
    );

    ensure!(!build.is_empty(), "[build] rustflags must not be empty");
    for flag in &build {
        ensure!(
            linux.contains(flag),
            "`{flag}` is named in [build] rustflags but not in the Linux table, so Linux \
             builds would silently lose it"
        );
    }
    ensure!(
        linux.iter().any(|flag| flag.contains("-fuse-ld=mold")),
        "the Linux table must add mold, got `{linux:?}`"
    );
    Ok(())
}

/// Ask Cargo what the configuration means, rather than only what it contains.
///
/// Parsing the TOML proves the file is well-formed; it cannot prove Cargo
/// accepts the key paths. A backend nested under the wrong table, or a
/// misspelled key, parses perfectly and is then silently ignored. `cargo config
/// get` reports Cargo's own resolved view, so a misplaced key shows up as a
/// missing value instead of passing unnoticed. No `--config` argument is
/// passed: auto-discovery is the mechanism under test.
///
/// The Linux entry is queried through its parent table: `config get` cannot
/// address a key whose name is a quoted `cfg` expression.
#[rstest]
#[case::dev_profile_backend(
    "profile.dev.codegen-backend",
    "profile.dev.codegen-backend = \"cranelift\""
)]
#[case::release_profile_backend(
    "profile.release.codegen-backend",
    "profile.release.codegen-backend = \"llvm\""
)]
#[case::unstable_flag("unstable.codegen-backend", "unstable.codegen-backend = true")]
#[case::build_rustflags("build.rustflags", "-Zthreads=8")]
#[case::linux_rustflags("target", "-Clink-arg=-fuse-ld=mold")]
fn cargo_resolves_the_committed_configuration_to_the_intended_settings(
    #[case] query: &str,
    #[case] expected: &str,
) -> Result<()> {
    let output = Command::new(env!("CARGO"))
        .arg("-Zunstable-options")
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

/// Only the two profiles Cargo's other profiles inherit from may name a
/// backend, and each must name the right one: `test` inherits `dev` and `bench`
/// inherits `release`, so those two settings decide every build in the
/// workspace.
#[test]
fn only_the_dev_and_release_profiles_select_a_backend() -> Result<()> {
    let config: toml::Value = toml::from_str(&cargo_config()?)?;
    let profiles = config
        .get("profile")
        .and_then(toml::Value::as_table)
        .context("the configuration must configure a profile")?;

    for (name, table) in profiles {
        let backend = table.get("codegen-backend").and_then(toml::Value::as_str);
        let expected = match name.as_str() {
            "dev" => Some("cranelift"),
            "release" => Some("llvm"),
            _ => None,
        };
        ensure!(
            backend == expected,
            "profile `{name}` should select {expected:?}, got {backend:?}"
        );
    }
    ensure!(
        profiles.contains_key("dev") && profiles.contains_key("release"),
        "both the dev and release profiles must state a backend explicitly"
    );
    ensure!(
        config
            .get("unstable")
            .and_then(|table| table.get("codegen-backend"))
            == Some(&toml::Value::Boolean(true)),
        "the configuration must enable the codegen-backend unstable flag"
    );
    Ok(())
}
