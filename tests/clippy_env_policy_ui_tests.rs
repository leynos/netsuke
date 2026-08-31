//! Contract coverage for the Clippy environment-mutation configuration.
//!
//! The configuration disallows every process-global environment mutation in
//! each workspace crate. The `lint-clippy` target invokes Clippy across the
//! workspace, all target kinds, and all features, covering production and
//! test compilation surfaces without duplicating Clippy's own lint tests.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use test_support::fs as test_fs;
use toml::Value;

/// List process-global environment mutations that the project forbids.
const FORBIDDEN_GLOBAL_ENV_MUTATIONS: [&str; 3] = [
    "std::env::set_var",
    "std::env::remove_var",
    "std::env::set_current_dir",
];

/// List per-crate Clippy configurations that enforce the policy.
const CLIPPY_POLICY_FILES: [&str; 2] = ["clippy.toml", "test_support/clippy.toml"];

/// Return a repository-relative path rooted at the workspace manifest.
fn repository_path(path: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

/// Return disallowed method paths from one Clippy configuration file.
fn disallowed_method_paths(policy_path: &Utf8Path) -> Result<Vec<String>> {
    let policy: Value = test_fs::read_to_string(policy_path)
        .with_context(|| format!("read Clippy policy at {policy_path}"))?
        .parse()
        .with_context(|| format!("parse Clippy policy at {policy_path}"))?;
    let methods = policy
        .get("disallowed-methods")
        .and_then(Value::as_array)
        .context("Clippy policy should declare disallowed-methods")?;
    Ok(methods
        .iter()
        .filter_map(|method| method.get("path").and_then(Value::as_str))
        .map(str::to_owned)
        .collect())
}

/// Keep every workspace Clippy policy configured for global mutation bans.
#[test]
fn clippy_configurations_disallow_every_global_environment_mutation() -> Result<()> {
    for policy_file in CLIPPY_POLICY_FILES {
        let policy_path = repository_path(policy_file);
        let paths = disallowed_method_paths(&policy_path)?;
        for required_path in FORBIDDEN_GLOBAL_ENV_MUTATIONS {
            ensure!(
                paths.iter().any(|path| path == required_path),
                "{policy_file} must disallow {required_path}, found {paths:?}"
            );
        }
    }
    Ok(())
}

/// Keep the Clippy gate scoped to every workspace target and feature.
#[test]
fn clippy_gate_covers_every_workspace_target_and_feature() -> Result<()> {
    let makefile = test_fs::read_to_string(repository_path("Makefile"))?;
    ensure!(
        makefile
            .contains("CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings"),
        "CLIPPY_FLAGS must cover every workspace target and feature with warnings denied"
    );
    ensure!(
        makefile.contains("$(CARGO) clippy $(CLIPPY_FLAGS)"),
        "lint-clippy must invoke Cargo Clippy with the workspace-wide contract"
    );
    Ok(())
}
