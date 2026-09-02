//! Packaging smoke tests that guard Cargo's publish boundary.
//!
//! These tests verify the packaged crate builds for publication and ensure
//! build-script sources remain in its manifest, where an omission would
//! otherwise fail only during release.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use netsuke::locale_catalogues::SUPPORTED_LOCALES;
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::process::Command;
use std::time::Instant;
use tempfile::TempDir;

const REQUIRED_PACKAGED_FILES: [&str; 9] = [
    "build_l10n_audit/mod.rs",
    "build_l10n_audit/compare.rs",
    "build_l10n_audit/ftl.rs",
    "build_l10n_audit/keys.rs",
    "build_l10n_audit/scanner.rs",
    "build_l10n_audit/byte_index.rs",
    "build_l10n_audit/metadata.rs",
    "build.rs",
    "src/localization/keys.rs",
];
const FORBIDDEN_PACKAGED_ROOTS: [&str; 2] = [".uv-cache", "test_support"];
/// Every catalogue named by the locale registry must ship in the package;
/// omitting one would break the build-time audit for downstream builds.
fn required_catalogue_paths() -> Vec<String> {
    SUPPORTED_LOCALES
        .iter()
        .map(|entry| format!("locales/{}/messages.ftl", entry.tag()))
        .collect()
}

/// Every README in the crate root must ship in the package; the localization
/// menu at the top of each edition links its siblings by relative path, so an
/// unpackaged translation leaves dead links for anyone reading the crate from
/// an unpacked or vendored source tree. Deriving the set from the crate root
/// rather than listing it here means a newly added translation is required to
/// be packaged without anyone remembering to update this test.
fn required_readme_paths() -> Result<Vec<String>> {
    let crate_root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the crate root")?;
    let mut readmes = Vec::new();
    for entry_result in crate_root.read_dir(".").context("read the crate root")? {
        let entry = entry_result.context("read a crate-root entry")?;
        let name = entry.file_name().context("read a crate-root entry name")?;
        if name.starts_with("README")
            && Utf8Path::new(&name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            readmes.push(name);
        }
    }
    readmes.sort();
    // A sweep that silently matched nothing would make the assertion vacuous.
    ensure!(
        !readmes.is_empty(),
        "the crate root should contain at least one README"
    );
    Ok(readmes)
}

/// Create a Cargo subprocess that writes build artefacts beneath `target_dir`.
fn cargo_subprocess(cargo_binary: &OsStr, target_dir: &TempDir) -> Command {
    let mut command = Command::new(cargo_binary);
    command.env("CARGO_TARGET_DIR", target_dir.path());
    command
}

/// Verify that the published package retains required build-script sources.
#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "locating build artefacts Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]
fn packaged_manifest_retains_build_script_sources() {
    let subscriber = tracing_subscriber::fmt().with_test_writer().finish();
    tracing::subscriber::with_default(subscriber, || {
        let cargo_binary = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let cargo_target_dir = tempfile::tempdir()
            .unwrap_or_else(|error| panic!("create isolated Cargo target directory: {error}"));
        let publish_started_at = Instant::now();
        let publish_output = cargo_subprocess(&cargo_binary, &cargo_target_dir)
            .args([
                "publish",
                "--dry-run",
                "--allow-dirty",
                "-p",
                "netsuke-build",
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap_or_else(|error| panic!("run cargo publish --dry-run: {error}"));
        tracing::info!(
            elapsed_seconds = publish_started_at.elapsed().as_secs_f64(),
            "cargo publish --dry-run completed"
        );

        assert!(
            publish_output.status.success(),
            "cargo publish --dry-run should succeed: {}",
            String::from_utf8_lossy(&publish_output.stderr)
        );

        let package_started_at = Instant::now();
        let list_output = cargo_subprocess(&cargo_binary, &cargo_target_dir)
            .args(["package", "--list", "--allow-dirty", "-p", "netsuke-build"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap_or_else(|error| panic!("run cargo package --list: {error}"));
        tracing::info!(
            elapsed_seconds = package_started_at.elapsed().as_secs_f64(),
            "cargo package --list completed"
        );

        assert!(
            list_output.status.success(),
            "cargo package --list should succeed: {}",
            String::from_utf8_lossy(&list_output.stderr)
        );

        let packaged_manifest = String::from_utf8_lossy(&list_output.stdout);
        let packaged_paths = packaged_manifest
            .lines()
            .map(|path| normalize_packaged_path(path.trim()))
            .collect::<BTreeSet<_>>();

        let readme_paths = required_readme_paths()
            .unwrap_or_else(|error| panic!("collect the crate-root READMEs: {error}"));
        assert_required_paths_present(&packaged_paths, &readme_paths);
        assert_forbidden_roots_absent(&packaged_paths);
    });
}

#[test]
fn cargo_subprocess_uses_the_given_target_directory() {
    let target_dir = tempfile::tempdir().expect("create isolated Cargo target directory");
    let command = cargo_subprocess(OsStr::new("cargo"), &target_dir);
    let configured_target_dir = command
        .get_envs()
        .find_map(|(key, value)| (key == OsStr::new("CARGO_TARGET_DIR")).then_some(value))
        .flatten();

    assert_eq!(configured_target_dir, Some(target_dir.path().as_os_str()));
}

/// Normalize Cargo's platform-native package-list separators for comparison.
fn normalize_packaged_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn assert_required_paths_present(packaged_paths: &BTreeSet<String>, readme_paths: &[String]) {
    for required_path in REQUIRED_PACKAGED_FILES {
        assert!(
            packaged_paths.contains(required_path),
            "packaged manifest should contain `{required_path}`"
        );
    }

    for required_path in required_catalogue_paths() {
        assert!(
            packaged_paths.contains(required_path.as_str()),
            "packaged manifest should contain `{required_path}`"
        );
    }

    for required_path in readme_paths {
        assert!(
            packaged_paths.contains(required_path.as_str()),
            "packaged manifest should contain `{required_path}`"
        );
    }
}

fn assert_forbidden_roots_absent(packaged_paths: &BTreeSet<String>) {
    for forbidden_root in FORBIDDEN_PACKAGED_ROOTS {
        // Name the offending entry: knowing only the forbidden root leaves the
        // reader grepping the packaged manifest by hand.
        // `cargo package --list` emits UTF-8 relative paths, so camino applies
        // here as it does elsewhere in the project, and comparing whole
        // components keeps neighbours such as `test_support-extra` allowed.
        let offender = packaged_paths.iter().find(|path| {
            Utf8Path::new(path)
                .components()
                .next()
                .is_some_and(|component| component.as_str() == forbidden_root)
        });
        assert!(
            offender.is_none(),
            "packaged manifest should not contain `{forbidden_root}`, found `{}`",
            offender.map(String::as_str).unwrap_or_default()
        );
    }

    assert!(
        packaged_paths.iter().all(|path| Utf8Path::new(path)
            .components()
            .all(|component| { component.as_str() != "ninja_env" })),
        "packaged manifest should not contain stale `ninja_env` paths"
    );
}

#[test]
fn normalized_packaged_path_accepts_windows_separator_spelling() {
    assert_eq!(
        normalize_packaged_path("build_l10n_audit\\mod.rs"),
        normalize_packaged_path("build_l10n_audit/mod.rs")
    );
}
