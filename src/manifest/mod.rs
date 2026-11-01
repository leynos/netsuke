//! Manifest loading helpers.
//!
//! Parse `Netsukefile` YAML before evaluating Jinja in supported fields.
//! Template helpers include environment reads and filesystem globs; malformed
//! values fail fast. [`ManifestValue`], [`ManifestMap`], [`ManifestName`], and
//! [`ManifestSource`] keep intermediate data and diagnostics domain-specific.
//! The optional `vars` section must be an object and cannot shadow `env` or
//! `glob`, because `MiniJinja` shares their global template namespace.

use crate::{
    ast::NetsukeManifest,
    localization::{self, keys},
};
use anyhow::Result;
use camino::Utf8PathBuf;
use minijinja::{Environment, UndefinedBehavior};
use serde::de::Error as _;

mod access_policy;
mod budget;
use budget::adapter::BudgetErrorExt;
pub(crate) use budget::adapter::budget_exhaustion_message;
mod diagnostics;
mod expand;
// `glob_paths` is the module's only boundary: every other item, including the
// `GlobEntryResult` alias, stays module-private. Denying `unreachable_pub`
// here rejects `pub` items unreachable from the crate root. The `glob_paths`
// re-export makes that item reachable and exempt; other `pub` items fail.
#[deny(unreachable_pub)]
mod glob;
mod hints;
mod jinja_macros;
mod load_stage;
mod loader_options;
mod loading;
mod parse_with_config;
mod path_loaders;
mod query;
mod registration;
mod render;
mod str_loaders;
mod test_surface;
/// JSON representation of a manifest node after YAML and Jinja evaluation.
pub type ManifestValue = serde_json::Value;
/// JSON object mapping string keys to manifest values.
pub type ManifestMap = serde_json::Map<String, ManifestValue>;
use self::jinja_macros::register_manifest_macros_with_budget;
pub use access_policy::{EnvAccessPolicy, EnvPolicyViolation};
pub use budget::ManifestBudgetLimits;
pub use diagnostics::{
    ManifestError, ManifestName, ManifestSource, map_data_error, map_yaml_error,
};
pub use env::{EnvReadError, EnvReader, ManifestEnvironment, process_env_reader};
pub(crate) use expand::expand_foreach_with_budget;
pub use glob::glob_paths;
pub use load_stage::ManifestLoadStage;
pub(crate) use loader_options::StdlibRegistration;
pub use loader_options::{ManifestLoadOptions, from_str_with_options};
use loading::{notify_stage, trace_expansion_report};
pub use parse_with_config::{from_str_with_env_and_config, from_str_with_env_and_policy};
pub use path_loaders::{
    from_path, from_path_with_policy, from_path_with_policy_and_env,
    from_path_with_policy_and_env_and_limits, from_path_with_policy_and_environment,
    from_path_with_policy_and_environment_and_limits, from_path_with_policy_and_limits,
};
#[cfg(test)]
pub(crate) use query::from_path_for_manifest_query;
pub(crate) use query::from_path_for_manifest_query_with_limits;
#[cfg(feature = "lint")]
pub(crate) use query::{LoadedManifest, from_path_for_manifest_query_with_source};
#[cfg(test)]
use registration::RESERVED_VAR_NAMES;
use registration::{localize_recipe_error, register_manifest_vars};
pub use render::render_manifest;
#[cfg(test)]
pub(crate) use str_loaders::from_str_with_limits;
pub use str_loaders::{from_str, from_str_with_env};
pub use test_surface::{
    OverlayCall, OverlayCallable, OverlayNameKind, TemplateOverlayError, TemplateOverlays,
    TestSurface,
};
pub(crate) use test_surface::{overlay_namespace, register_overlays};
#[cfg(test)]
use workspace::open_manifest_workspace;

/// Inputs to a manifest parse, bundled to keep the parameter list bounded.
struct ManifestParse<'a, 'name> {
    /// Name reported in diagnostics.
    name: &'name ManifestName,
    /// Manifest workspace root, anchoring relative `glob()` patterns; `None`
    /// falls back to the process current directory at the composition root.
    manifest_root: Option<Utf8PathBuf>,
    /// Capabilities decided for this load.
    registration: StdlibRegistration,
    /// Environment inputs backing the `env()` helper.
    environment: ManifestEnvironment<'a>,
    /// Resource ceilings resolved from trusted configuration before loading.
    budget_limits: ManifestBudgetLimits,
    /// Observes load phases without changing parsing behaviour.
    on_stage: Option<&'a mut dyn FnMut(ManifestLoadStage)>,
}

/// Decisions the render and expansion halves need from the registration.
#[derive(Clone, Copy, Debug)]
struct RenderDecisions {
    /// Render with the manifest-query render mode.
    uses_query_rendering: bool,
    /// Treat a refused helper as a conditional `when`, not a failure.
    classifies_query_refusals: bool,
    /// Record expansion and budget telemetry.
    records_expansion_and_budget_telemetry: bool,
}

impl RenderDecisions {
    /// Read every decision from the load's registration.
    const fn from_registration(registration: &StdlibRegistration) -> Self {
        Self {
            uses_query_rendering: registration.uses_query_rendering(),
            classifies_query_refusals: registration.classifies_query_refusals(),
            records_expansion_and_budget_telemetry: registration
                .records_expansion_and_budget_telemetry(),
        }
    }
}

/// Inputs that shape the template environment for one load.
struct TemplateRequest<'name, 'a> {
    /// Name reported in diagnostics.
    name: &'name ManifestName,
    /// Root anchoring relative `glob()` patterns.
    manifest_root: Option<Utf8PathBuf>,
    /// Capabilities decided for this load.
    registration: StdlibRegistration,
    /// Environment inputs backing the `env()` helper.
    environment: &'a ManifestEnvironment<'a>,
    /// Resource ceilings resolved before loading.
    budget_limits: ManifestBudgetLimits,
}

/// Everything needed to expand and render an evaluated manifest.
struct ExpansionRequest<'name, 'a, 'stage> {
    /// YAML tree produced by the template-environment stage.
    doc: ManifestValue,
    /// Template environment built for this load.
    jinja: Environment<'static>,
    /// Shared resource accounting for this evaluation.
    budget: &'a budget::ManifestBudget,
    /// Decisions the registration supplies to this half of the load.
    decisions: RenderDecisions,
    /// Name reported in diagnostics.
    name: &'name ManifestName,
    /// Observes load phases without changing parsing behaviour.
    on_stage: &'a mut Option<&'stage mut dyn FnMut(ManifestLoadStage)>,
}

/// Parse, render, and validate a manifest with injected loading boundaries.
///
/// Render Jinja values, anchor relative `glob()` patterns at `manifest_root`,
/// record telemetry for full loads, and validate rendered recipes.
fn evaluate_manifest(yaml: &str, parse: ManifestParse<'_, '_>) -> Result<NetsukeManifest> {
    let ManifestParse {
        name,
        manifest_root,
        registration,
        environment,
        budget_limits,
        mut on_stage,
    } = parse;
    let decisions = RenderDecisions::from_registration(&registration);
    let request = TemplateRequest {
        name,
        manifest_root,
        registration,
        environment: &environment,
        budget_limits,
    };
    let (doc, jinja, budget) = build_template_environment(yaml, request, &mut on_stage)?;
    expand_and_render(ExpansionRequest {
        doc,
        jinja,
        budget: &budget,
        decisions,
        name,
        on_stage: &mut on_stage,
    })
}

/// Build the template environment and evaluate the manifest's YAML tree.
///
/// This covers the loader's parse, registration, and macro steps; expansion and
/// rendering are left to [`expand_and_render`] so each half stays small enough
/// to review on its own.
fn build_template_environment(
    yaml: &str,
    request: TemplateRequest<'_, '_>,
    on_stage: &mut Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<(ManifestValue, Environment<'static>, budget::ManifestBudget)> {
    let TemplateRequest {
        name,
        manifest_root,
        registration,
        environment,
        budget_limits,
    } = request;
    notify_stage(on_stage, ManifestLoadStage::InitialYamlParsing);
    let budget = budget::ManifestBudget::new(budget_limits)?;
    budget
        .charge_source(yaml.len(), budget::ManifestBudgetStage::Source)
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    let doc: ManifestValue = serde_saphyr::from_str(yaml).map_err(|e| ManifestError::Parse {
        source: map_yaml_error(&e, &ManifestSource::from(yaml), name),
        message: localization::message(keys::MANIFEST_PARSE),
    })?;
    let mut jinja = Environment::new();
    jinja.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose custom helpers to templates.
    loader_options::register_host_helpers(&mut jinja, &registration, environment, manifest_root);
    // `register_stdlib` consumes the registration, so the overlays it carries
    // are taken first. Cloning them copies only the `Arc` handles and the name
    // map; the dispatch closures stay shared.
    let overlays = registration.overlays().cloned();
    let _stdlib_state = loader_options::register_stdlib(&mut jinja, registration)?;
    // Computed before the vars are registered: the manifest's own keys are part
    // of the namespace an overlay must not take, and the same value answers the
    // reserved-name check inside `register_manifest_vars`.
    let namespace = overlay_namespace(&doc);
    register_manifest_vars(&doc, &mut jinja, name, &namespace)?;
    notify_stage(on_stage, ManifestLoadStage::TemplateExpansion);
    register_manifest_macros_with_budget(&doc, &mut jinja, &budget)?;
    // Last, so an overlay can replace any helper the steps above installed.
    register_overlays(&mut jinja, overlays.as_ref(), &namespace)?;
    Ok((doc, jinja, budget))
}

/// Expand `foreach` and `when`, then deserialize and render the manifest.
fn expand_and_render(request: ExpansionRequest<'_, '_, '_>) -> Result<NetsukeManifest> {
    let ExpansionRequest {
        mut doc,
        jinja,
        budget,
        decisions,
        name,
        on_stage,
    } = request;
    let expansion_report = expand_foreach_with_budget(
        &mut doc,
        &jinja,
        budget,
        decisions.classifies_query_refusals,
    )?;
    if decisions.records_expansion_and_budget_telemetry {
        trace_expansion_report(&expansion_report);
    }
    notify_stage(on_stage, ManifestLoadStage::FinalRendering);
    let manifest: NetsukeManifest =
        serde_json::from_value(doc).map_err(|error| ManifestError::Parse {
            source: map_data_error(localize_recipe_error(error), name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;
    let rendered_manifest = if decisions.uses_query_rendering {
        render::render_manifest_for_manifest_query_with_budget(manifest, &jinja, budget)?
    } else {
        render::render_manifest_with_budget(manifest, &jinja, budget)?
    };
    rendered_manifest
        .validate_recipes()
        .map_err(|detail| ManifestError::Parse {
            source: map_data_error(serde_json::Error::custom(detail), name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;
    Ok(rendered_manifest)
}
mod env;
pub use env::{ENV_LOOKUP_OUTCOME_VALUES, ENV_LOOKUP_TOTAL};
#[cfg(test)]
mod tests;
mod workspace;
