//! An external crate can construct loader options and load a manifest.
//!
//! This is OBL-12's embedder contract: every input `ManifestLoadOptions::full`
//! and `from_str_with_options` need is public, while `StdlibRegistration`
//! stays crate-private. Integration tests compile as their own crate, so an
//! unpublished constructor or accessor fails to compile here, exactly as it
//! would for a downstream consumer.
//!
//! # Why this is not a `tests/ui` fixture
//!
//! The existing UI harness invokes `rustc` directly with no dependency
//! resolution, so it cannot link `netsuke`; `trybuild` would resolve it but is
//! not a dependency (C7). Compiling this file as an integration test gives the
//! same external-crate visibility without either cost.

use netsuke::manifest::{
    EnvAccessPolicy, ManifestEnvironment, ManifestLoadOptions, ManifestName, from_str_with_options,
    process_env_reader,
};
use netsuke::stdlib::StdlibConfig;

/// A manifest that reads and acts on nothing.
const PURE_MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: pure\n",
    "    command: \"echo pure\"\n",
);

#[test]
fn embedder_loads_a_manifest_through_the_options_entry_point() {
    let reader = process_env_reader();
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
    let config = StdlibConfig::from_current_dir().expect("open the test workspace");
    let options = ManifestLoadOptions::full(config, environment);

    let manifest = from_str_with_options(PURE_MANIFEST, &ManifestName::new("Netsukefile"), options)
        .expect("load the embedder manifest");

    assert_eq!(manifest.targets.len(), 1);
}
