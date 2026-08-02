//! Packaging smoke tests that guard Cargo's publish boundary.
//!
//! These tests verify the packaged crate builds for publication and ensure
//! build-script sources remain in its manifest, where an omission would
//! otherwise fail only during release.

#![expect(
    clippy::disallowed_methods,
    reason = "locating build artefacts Cargo reports through the environment; there is no seam to inject and no process state to isolate"
)]

use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use std::process::Command;

const REQUIRED_PACKAGED_FILES: [&str; 5] = [
    "build_l10n_audit.rs",
    "build.rs",
    "src/localization/keys.rs",
    "locales/en-US/messages.ftl",
    "locales/es-ES/messages.ftl",
];

#[test]
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

    assert!(
        packaged_paths.iter().all(|path| Path::new(path)
            .components()
            .all(|component| { component.as_os_str() != "ninja_env" })),
        "packaged manifest should not contain stale `ninja_env` paths"
    );
}
