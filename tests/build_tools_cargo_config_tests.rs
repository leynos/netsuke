//! Contracts for the committed `.cargo/config.toml`.
//!
//! The recipes are covered by `build_tools_make_target_tests`; this suite is
//! about the file Cargo auto-discovers, which is what makes the build standard
//! the default for a bare `cargo` invocation rather than something a Make
//! target has to opt into.
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
/// accepts the key paths. A misspelled key parses perfectly and is then
/// silently ignored. `cargo config get` reports Cargo's own resolved view, so a
/// misplaced key shows up as a missing value instead of passing unnoticed. No
/// `--config` argument is passed: auto-discovery is the mechanism under test.
///
/// The Linux entry is queried through its parent table: `config get` cannot
/// address a key whose name is a quoted `cfg` expression.
#[rstest]
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

/// The configuration must name no codegen backend, for any profile.
///
/// This is a refusal rather than an omission, and it is deliberate. The
/// Cranelift backend cannot initiate a panic on any nightly tested: a panic
/// raised in a Cranelift-compiled frame aborts the process with "failed to
/// initiate panic, error 5" instead of unwinding, which takes down every
/// failing test, every `#[should_panic]`, and every `catch_unwind`. A profile
/// key here applies to every build in the repository, so adding one back has to
/// go through the evidence in the developers' guide rather than through a
/// one-line edit that looks like a speed-up.
#[test]
fn the_configuration_names_no_codegen_backend() -> Result<()> {
    let config: toml::Value = toml::from_str(&cargo_config()?)?;

    if let Some(profiles) = config.get("profile").and_then(toml::Value::as_table) {
        for (name, table) in profiles {
            ensure!(
                table.get("codegen-backend").is_none(),
                "profile `{name}` names a codegen backend; see \"The build standard\" in \
                 docs/developers-guide.md before adding one"
            );
        }
    }
    ensure!(
        config
            .get("unstable")
            .and_then(|table| table.get("codegen-backend"))
            .is_none(),
        "the configuration must not enable the codegen-backend unstable flag"
    );
    // The environment override is the documented escape hatch for a single
    // target, so the file must not need it either.
    ensure!(
        !cargo_config()?.contains("CARGO_PROFILE_DEV_CODEGEN_BACKEND"),
        "the configuration should not reference the backend override"
    );
    Ok(())
}
