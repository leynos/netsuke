//! Contracts for the template overlays a test load may register.
//!
//! An overlay is the only way a `netsuke test` load can answer a helper the
//! surface withholds. Two claims follow, and each has its own module.
//!
//! The namespace rules are load-bearing rather than cosmetic: an overlay that
//! took `env`, a loader-owned `__netsuke_` global, a manifest `vars` key, or a
//! macro name would silently reinterpret a name the manifest or the loader
//! already gives meaning to. `namespaces` pins those rejections.
//!
//! A legitimate overlay must be visible everywhere expansion looks —
//! `foreach`, `when`, and rendered fields — and a replaced helper must still be
//! reachable through [`OverlayCall::shadowed`]. `registration` pins that.

mod namespaces;
mod registration;

use anyhow::{Context as _, Result};
use minijinja::value::Value;

use crate::ast::{NetsukeManifest, Recipe};
use crate::manifest::{
    ManifestLoadOptions, ManifestName, OverlayCall, OverlayCallable, TemplateOverlays, TestSurface,
    from_str_with_options,
};
use crate::stdlib::StdlibConfig;

/// Open the stdlib configuration the test workspace supplies.
fn config() -> StdlibConfig {
    StdlibConfig::from_current_dir().expect("the test workspace must open")
}

/// Load `yaml` under a test surface carrying `overlays`.
fn load_with(yaml: &str, overlays: TemplateOverlays) -> Result<NetsukeManifest> {
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::test(TestSurface::new(config()).with_overlays(overlays)),
    )
}

/// Load `yaml` under a test surface with no overlays.
fn load(yaml: &str) -> Result<NetsukeManifest> {
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::test(TestSurface::new(config())),
    )
}

/// Register one overlay whose dispatch ignores its call and returns `value`.
fn constant_overlay(name: &str, value: Value) -> TemplateOverlays {
    TemplateOverlays::default().with_function(
        name,
        OverlayCallable::new(move |_call: &OverlayCall<'_, '_>| Ok(value.clone())),
    )
}

/// Return the names of every target in `manifest`.
fn target_names(manifest: &NetsukeManifest) -> Vec<String> {
    manifest
        .targets
        .iter()
        .flat_map(|target| target.name.to_string_vec())
        .collect()
}

/// Return the single rendered command of a loaded probe manifest.
fn rendered_command(manifest: &NetsukeManifest) -> Result<&str> {
    let target = manifest
        .targets
        .first()
        .context("the probe has one target")?;
    match &target.recipe {
        Recipe::Command { command, .. } => command
            .as_single()
            .context("the probe command must render to one string"),
        other => anyhow::bail!("the probe target is not a command: {other:?}"),
    }
}
