//! Packaging smoke tests that guard Cargo's publish boundary.
//!
//! These tests verify the packaged crate builds for publication and ensure
//! build-script sources remain in its manifest, where an omission would
//! otherwise fail only during release.

use netsuke::localization::locales::SUPPORTED_LOCALES;
use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use std::process::Command;

const REQUIRED_PACKAGED_FILES: [&str; 8] = [
    "build_l10n_audit/mod.rs",
    "build_l10n_audit/compare.rs",
    "build_l10n_audit/ftl.rs",
    "build_l10n_audit/keys.rs",
    "build_l10n_audit/scanner.rs",
    "build_l10n_audit/metadata.rs",
    "build.rs",
    "src/localization/keys.rs",
];

/// Every catalogue named by the locale registry must ship in the package;
/// omitting one would break the build-time audit for downstream builds.
fn required_catalogue_paths() -> Vec<String> {
    SUPPORTED_LOCALES
        .iter()
        .map(|entry| format!("locales/{}/messages.ftl", entry.tag()))
        .collect()
}
#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "locating build artefacts Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]
fn packaged_manifest_retains_build_script_sources() {
    let cargo_binary = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let publish_output = Command::new(&cargo_binary)
        .args(["publish", "--dry-run", "--allow-dirty", "-p", "netsuke"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap_or_else(|error| panic!("run cargo publish --dry-run: {error}"));

    assert!(
        publish_output.status.success(),
        "cargo publish --dry-run should succeed: {}",
        String::from_utf8_lossy(&publish_output.stderr)
    );

    let list_output = Command::new(cargo_binary)
        .args(["package", "--list", "--allow-dirty", "-p", "netsuke"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap_or_else(|error| panic!("run cargo package --list: {error}"));

    assert!(
        list_output.status.success(),
        "cargo package --list should succeed: {}",
        String::from_utf8_lossy(&list_output.stderr)
    );

    let packaged_manifest = String::from_utf8_lossy(&list_output.stdout);
    let packaged_paths = packaged_manifest
        .lines()
        .map(str::trim)
        .collect::<BTreeSet<_>>();

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

    assert!(
        packaged_paths.iter().all(|path| Path::new(path)
            .components()
            .all(|component| { component.as_os_str() != "ninja_env" })),
        "packaged manifest should not contain stale `ninja_env` paths"
    );
}
