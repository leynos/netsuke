//! Compile-pass fixture for public manifest environment loader APIs.
//!
//! The direct-rustc harness type-checks this as an external crate. It covers
//! the legacy loader adapters and the policy-aware environment bundle without
//! reading a manifest at run time.

use netsuke::{
    manifest::{
        EnvAccessPolicy, EnvReadError, EnvReader, ManifestEnvironment,
        from_path_with_policy, from_path_with_policy_and_env,
        from_path_with_policy_and_environment,
    },
    stdlib::NetworkPolicy,
};
use std::sync::Arc;

/// Reference the legacy and explicit loader APIs as an external embedder.
fn reference_manifest_loader_apis() {
    let reader: EnvReader = Arc::new(|_| Err(EnvReadError::NotPresent));
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());

    let _ = from_path_with_policy("Netsukefile", NetworkPolicy::default(), None);
    let _ = from_path_with_policy_and_env("Netsukefile", NetworkPolicy::default(), &reader, None);
    let _ = from_path_with_policy_and_environment(
        "Netsukefile",
        NetworkPolicy::default(),
        &environment,
        None,
    );
}

fn main() { reference_manifest_loader_apis(); }
