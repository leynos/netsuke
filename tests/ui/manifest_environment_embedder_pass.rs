//! Compile-pass fixture for public manifest environment loader APIs.
//!
//! The direct-rustc harness type-checks this as an external crate. It covers
//! the legacy loader adapters, the policy-aware environment bundle, and the
//! string-loading policy variant without reading a manifest at run time.

use netsuke::{
    manifest::{
        EnvAccessPolicy, EnvReadError, EnvReader, ManifestEnvironment, from_path_with_policy,
        from_path_with_policy_and_env, from_path_with_policy_and_environment,
        from_str_with_env_and_policy,
    },
    stdlib::NetworkPolicy,
};
use std::sync::Arc;

/// Reference the legacy and explicit loader APIs as an external embedder.
fn reference_manifest_loader_apis() {
    let reader: EnvReader = Arc::new(|_| Err(EnvReadError::NotPresent));
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());

    drop(from_path_with_policy(
        "Netsukefile",
        NetworkPolicy::default(),
        None,
    ));
    drop(from_path_with_policy_and_env(
        "Netsukefile",
        NetworkPolicy::default(),
        &reader,
        None,
    ));
    drop(from_path_with_policy_and_environment(
        "Netsukefile",
        NetworkPolicy::default(),
        &environment,
        None,
    ));
    drop(from_str_with_env_and_policy(
        "netsuke_version: 1.0.0\ntargets: []\n",
        &reader,
        &EnvAccessPolicy::default(),
    ));
}

fn main() {
    reference_manifest_loader_apis();
}
