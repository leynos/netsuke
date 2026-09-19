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

/// The configuration must name no codegen backend, by any of the routes open
/// to it.
///
/// This is a refusal rather than an omission, and it is deliberate. A panic
/// compiled by the Cranelift backend does not find the unwind handler it
/// should: measured on `nightly-2026-08-23`, `catch_unwind` fails to catch,
/// and a panic on a spawned thread runs off the end of the stack and aborts
/// with "failed to initiate panic, error 5". A bare `#[should_panic]` passes,
/// because libtest's outermost handler needs nothing in between to work, which
/// is why a probe limited to that case reads as a pass. A profile key here
/// applies to every build in the repository, so adding one back has to go
/// through the evidence in the developers' guide rather than through a
/// one-line edit that looks like a speed-up.
///
/// Three routes, because closing one alone leaves the other two open: a direct
/// `codegen-backend` key on a profile, a package override beneath one, and
/// `-Zcodegen-backend=` inside the `rustflags` the standard applies. The last
/// would be a backend change that never mentions the word "profile" at all.
#[test]
fn the_configuration_names_no_codegen_backend() -> Result<()> {
    let config: toml::Value = toml::from_str(&cargo_config()?)?;

    let profiles = first_profile_codegen_backend(&config);
    ensure!(
        profiles.is_none(),
        "profile `{}` names a codegen backend; see \"The build standard\" in \
         docs/developers-guide.md before adding one",
        profiles.unwrap_or_default()
    );
    let flags = rustflags_carrying_backend(&config);
    ensure!(
        flags.is_empty(),
        "rustflags name a codegen backend with `{flags:?}`; see \"The build standard\" in \
         docs/developers-guide.md before adding one"
    );
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

/// Returns the name of the first profile or package override that names a
/// codegen backend, if any.
///
/// Only the two key paths Cargo documents are consulted: `codegen-backend`
/// directly on a profile, and `codegen-backend` on an entry beneath a
/// profile's `package` table. Walking every nested table instead would let an
/// unrelated key that happens to share the name — a Cargo feature named
/// `codegen-backend`, say — be reported as a backend selection, which is a
/// false positive in the one test that must not have one.
///
/// `profile` may also nest `package` beneath `package`, which Cargo does not
/// document and this does not look for; a backend named that deep would be
/// invisible to Cargo too.
fn first_profile_codegen_backend(config: &toml::Value) -> Option<String> {
    let profiles = config.get("profile")?.as_table()?;
    for (name, table) in profiles {
        if table.get("codegen-backend").is_some() {
            return Some(name.clone());
        }
        let Some(overrides) = table.get("package").and_then(toml::Value::as_table) else {
            continue;
        };
        for (spec, override_table) in overrides {
            if override_table.get("codegen-backend").is_some() {
                return Some(format!("{name}.package.{spec}"));
            }
        }
    }
    None
}

/// Neither route a backend can be named by is closed while the other is open.
///
/// Driven with synthetic documents rather than the committed file, because the
/// committed file is the one that must contain no backend: a test that could
/// only ask it that question would report a pass whether or not the helpers
/// can tell the two apart. Cargo refuses to load a configuration that names
/// one, so these helpers are also the only place the detection can be
/// exercised at all.
#[test]
fn unit_a_named_codegen_backend_is_detected_by_either_route() -> Result<()> {
    let clean: toml::Value = toml::from_str("[build]\nrustflags = [\"-Zthreads=8\"]\n")?;
    ensure!(
        first_profile_codegen_backend(&clean).is_none()
            && rustflags_carrying_backend(&clean).is_empty(),
        "a configuration naming no backend should report none"
    );

    let direct: toml::Value = toml::from_str("[profile.dev]\ncodegen-backend = \"cranelift\"\n")?;
    ensure!(
        first_profile_codegen_backend(&direct).as_deref() == Some("dev"),
        "a profile key should be reported by its profile's name"
    );

    let nested: toml::Value =
        toml::from_str("[profile.release.package.\"*\"]\ncodegen-backend = \"cranelift\"\n")?;
    ensure!(
        first_profile_codegen_backend(&nested).as_deref() == Some("release.package.*"),
        "a package override should be reported by the path that reaches it"
    );

    let flags: toml::Value = toml::from_str(
        "[target.'cfg(target_os = \"linux\")']\nrustflags = [\"-Zcodegen-backend=cranelift\"]\n",
    )?;
    let found = rustflags_carrying_backend(&flags);
    // `first()` rather than an index: the pair count is asserted first, so a
    // mismatch reports the whole reading, and an indexing panic here would
    // report a slice bound instead of the flag that was found.
    ensure!(
        found.len() == 1,
        "exactly one rustflags source should name a backend, got {found:?}"
    );
    let (source, flag) = found.first().context("one source was reported")?;
    ensure!(
        flag == "-Zcodegen-backend=cranelift",
        "a backend selected from rustflags should be reported with its flag, got {flag:?}"
    );
    ensure!(
        source.contains("linux"),
        "the flag lives in the Linux table, so that is the source to report, got {source:?}"
    );
    Ok(())
}

/// A profile key is reported only where Cargo documents it.
///
/// The word is not reserved: a `[features]` entry or an unrelated nested table
/// may carry it, and reporting one of those as a backend selection would make
/// this the test that fails on a change that is fine. The check walks the two
/// key paths Cargo defines and stops there.
#[test]
fn unit_an_unrelated_codegen_backend_key_is_not_reported() -> Result<()> {
    let decoy: toml::Value = toml::from_str(
        "[features]\ncodegen-backend = [\"dep:something\"]\n\
         [profile.dev.package.netsuke-build]\nopt-level = 3\n",
    )?;
    ensure!(
        first_profile_codegen_backend(&decoy).is_none(),
        "only the two documented key paths are backend selections"
    );
    Ok(())
}

/// Returns every `rustflags` source that names a codegen backend, with the
/// flag it used.
///
/// A backend can be selected without a profile key at all, by putting
/// `-Zcodegen-backend=` into the flags every build already applies — which is
/// exactly where the standard's own flags live, so the check has to reach
/// there. Both sources are inspected: the Linux table and `[build]`, since
/// whichever matches is the one Cargo uses on that platform.
fn rustflags_carrying_backend(config: &toml::Value) -> Vec<(String, String)> {
    let sources = [
        ("build".to_owned(), config.get("build")),
        (
            r#"target.cfg(target_os = "linux")"#.to_owned(),
            config
                .get("target")
                .and_then(|table| table.get(r#"cfg(target_os = "linux")"#)),
        ),
    ];
    let mut offenders = Vec::new();
    for (name, table) in sources {
        // The closure binding is distinct from the loop's `table` on purpose:
        // `source` is the whole table, and this closure reaches one level in.
        let Some(flags) = table
            .and_then(|source| source.get("rustflags"))
            .and_then(toml::Value::as_array)
        else {
            continue;
        };
        for flag in flags.iter().filter_map(toml::Value::as_str) {
            if flag.contains("codegen-backend") {
                offenders.push((name.clone(), flag.to_owned()));
            }
        }
    }
    offenders
}
