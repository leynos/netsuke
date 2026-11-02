//! Namespace rules an overlay name must respect.
//!
//! Every rejection row loads the same manifest, so the only thing that differs
//! between rows is the overlay name. The accepted rows prove the check is a
//! namespace rule rather than a blanket refusal: `glob` is registered by the
//! loader, and replacing it is exactly why overlays exist.

use anyhow::{Context as _, Result, ensure};
use minijinja::value::Value;
use rstest::rstest;

use super::{constant_overlay, load_with, target_names};
use crate::manifest::{OverlayNameKind, TemplateOverlayError};

/// The manifest every rejection row loads.
///
/// It declares a `vars` key and a macro so each namespace has a name to
/// collide with, and one target so the load has something to render.
const NAMESPACED_MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "vars:\n",
    "  greeting: hello\n",
    "macros:\n",
    "  - signature: \"shout(text)\"\n",
    "    body: \"{{ text | upper }}\"\n",
    "targets:\n",
    "  - name: probe\n",
    "    command: \"echo probe\"\n",
);

/// Return the typed collision a load reported, if any.
fn collision(error: &anyhow::Error) -> Option<&TemplateOverlayError> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<TemplateOverlayError>())
}

/// Assert a load rejected `name` because it collided with `kind`.
fn assert_collision(name: &str, kind: OverlayNameKind) -> Result<()> {
    let overlays = constant_overlay(name, Value::from(true));
    let error = load_with(NAMESPACED_MANIFEST, overlays)
        .err()
        .with_context(|| format!("the overlay `{name}` must be rejected"))?;
    match collision(&error) {
        Some(TemplateOverlayError::NameCollision {
            name: reported,
            kind: reported_kind,
        }) => {
            ensure!(
                reported == name,
                "the load named `{reported}` rather than `{name}`"
            );
            ensure!(
                *reported_kind == kind,
                "`{name}` was classified {reported_kind:?}, not {kind:?}"
            );
        }
        other => anyhow::bail!("`{name}` carried no typed collision: {other:?}"),
    }
    let text = error.to_string();
    ensure!(
        text.contains("netsuke::manifest::overlay::collision"),
        "the diagnostic did not lead with the collision code: {text}"
    );
    ensure!(
        text.contains(name),
        "the diagnostic did not name `{name}`: {text}"
    );
    Ok(())
}

/// An overlay may not take the loader's environment seam.
#[test]
fn overlay_named_env_is_rejected() -> Result<()> {
    assert_collision("env", OverlayNameKind::EnvironmentSeam)
}

/// An overlay may not take a global the loader owns by prefix.
#[test]
fn overlay_named_with_the_internal_prefix_is_rejected() -> Result<()> {
    assert_collision(
        "__netsuke_manifest_macro_imports",
        OverlayNameKind::Internal,
    )
}

/// An overlay may not take a name the manifest's `vars` section declares.
#[test]
fn overlay_named_after_a_manifest_variable_is_rejected() -> Result<()> {
    assert_collision("greeting", OverlayNameKind::ManifestVariable)
}

/// An overlay may not take a name the manifest's `macros` section declares.
#[test]
fn overlay_named_after_a_manifest_macro_is_rejected() -> Result<()> {
    assert_collision("shout", OverlayNameKind::ManifestMacro)
}

/// An overlay may take a name the loader does not reserve.
///
/// `glob()` is loader-owned but not reserved: standing in for it is the reason
/// the overlay mechanism exists. A name nothing claims collides with nothing.
/// Both must load, or the rejection rows above would be indistinguishable from
/// a blanket refusal.
#[rstest]
#[case::loader_helper("glob")]
#[case::fresh_name("brand_new_helper")]
fn overlay_may_take_an_unreserved_name(#[case] name: &str) -> Result<()> {
    let manifest = load_with(
        NAMESPACED_MANIFEST,
        constant_overlay(name, Value::from(true)),
    )?;
    ensure!(
        target_names(&manifest) == ["probe"],
        "the load did not complete: {:?}",
        target_names(&manifest)
    );
    Ok(())
}
