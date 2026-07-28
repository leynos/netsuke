//! Packaging smoke tests that guard the `Cargo.toml` include list.
//!
//! These tests ensure build-script sources remain in the published crate,
//! where an omitted manifest entry would otherwise fail only during release.

use std::collections::BTreeSet;
use std::env;
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
    let output = Command::new(cargo_binary)
        .args(["package", "--list", "--allow-dirty", "-p", "netsuke"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap_or_else(|error| panic!("run cargo package --list: {error}"));

    assert!(
        output.status.success(),
        "cargo package --list should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let packaged_manifest = String::from_utf8_lossy(&output.stdout);
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
        packaged_paths
            .iter()
            .all(|path| !path.contains("ninja_env")),
        "packaged manifest should not contain stale `ninja_env` paths"
    );
}
