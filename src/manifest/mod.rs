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
    stdlib::StdlibConfig,
};
use anyhow::Result;
use minijinja::{Environment, UndefinedBehavior};
use serde::de::Error as _;
use std::sync::Arc;

mod budget;
pub(crate) mod budget_adapter;
use budget_adapter::{BudgetErrorExt, from_str_named};
mod diagnostics;
mod env_policy;
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
mod loading;
mod parse_with_config;
#[path = "path_loaders.rs"]
mod path_loaders;
mod query;
mod registration;
mod render;
/// JSON representation of a manifest node after YAML and Jinja evaluation.
pub type ManifestValue = serde_json::Value;
/// JSON object mapping string keys to manifest values.
pub type ManifestMap = serde_json::Map<String, ManifestValue>;
use self::{env_reader::env_var_with, jinja_macros::register_manifest_macros_with_budget};
pub use budget::ManifestBudgetLimits;
pub use diagnostics::{
    ManifestError, ManifestName, ManifestSource, map_data_error, map_yaml_error,
};
pub use env_policy::{EnvAccessPolicy, EnvPolicyViolation};
pub use env_reader::{EnvReadError, EnvReader, ManifestEnvironment, process_env_reader};
pub(crate) use expand::expand_foreach_with_budget;
pub use glob::glob_paths;
pub use load_stage::ManifestLoadStage;
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
#[cfg(test)]
use registration::RESERVED_VAR_NAMES;
use registration::{localize_recipe_error, register_manifest_vars};
pub use render::render_manifest;
#[cfg(test)]
use workspace::open_manifest_workspace;
/// Receives normal-loader reports; manifest queries supply `None` to stay
/// telemetry-free.
type ExpansionReportObserver = fn(&expand::ExpansionReport);

/// Inputs to a manifest parse, bundled to keep the parameter list bounded.
struct ManifestParse<'a> {
    /// Name reported in diagnostics.
    name: &'a ManifestName,
    /// Optional stdlib registration configuration.
    stdlib_registration: Option<StdlibRegistration>,
    /// Environment reader backing the `env()` helper.
    env_reader: &'a EnvReader,
    /// Access policy evaluated before the `env()` reader runs.
    env_access_policy: &'a EnvAccessPolicy,
    /// Manifest workspace root, anchoring relative `glob()` patterns; `None`
    /// falls back to the process current directory at the composition root.
    manifest_root: Option<camino::Utf8PathBuf>,
    /// Optional observer for reports produced by normal manifest loading.
    expansion_report_observer: Option<ExpansionReportObserver>,
    /// Resource ceilings resolved from trusted configuration before loading.
    budget_limits: ManifestBudgetLimits,
}

/// Selects the stdlib surface available while rendering a manifest.
enum StdlibRegistration {
    /// The complete stdlib used for a normal build manifest.
    Full(Box<StdlibConfig>),
    /// The read-only stdlib used to inspect manifest discovery metadata.
    ManifestQuery,
}

/// Parse, render, and validate a manifest with injected loading boundaries.
///
/// Render Jinja values, anchor relative `glob()` patterns at `manifest_root`,
/// notify the optional expansion observer, and validate rendered recipes.
fn evaluate_manifest(
    yaml: &str,
    parse: ManifestParse<'_>,
    on_stage: &mut Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    let ManifestParse {
        name,
        stdlib_registration,
        env_reader,
        env_access_policy,
        manifest_root,
        expansion_report_observer,
        budget_limits,
    } = parse;
    let is_manifest_query = matches!(stdlib_registration, Some(StdlibRegistration::ManifestQuery));
    notify_stage(on_stage, ManifestLoadStage::InitialYamlParsing);
    let budget = budget::ManifestBudget::new(budget_limits)?;
    budget
        .charge_source(yaml.len(), budget::ManifestBudgetStage::Source)
        .map_err(|exhaustion| exhaustion.into_error(minijinja::ErrorKind::InvalidOperation))?;
    let mut doc: ManifestValue =
        serde_saphyr::from_str(yaml).map_err(|e| ManifestError::Parse {
            source: map_yaml_error(&e, &ManifestSource::from(yaml), name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;
    let mut jinja = Environment::new();
    jinja.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose custom helpers to templates.
    let reader = Arc::clone(env_reader);
    let policy_for_env_lookup = env_access_policy.clone();
    jinja.add_function("env", move |var_name: String| {
        env_var_with(&var_name, &policy_for_env_lookup, |key| reader(key))
    });
    let glob_base = glob::GlobBaseCache::new(manifest_root);
    jinja.add_function("glob", move |pattern: String| {
        let expansion = glob::expand_manifest_template_glob(&pattern, &glob_base)?;
        expansion.into_template_paths(&pattern)
    });
    let _stdlib_state = match stdlib_registration {
        Some(StdlibRegistration::Full(config)) => {
            crate::stdlib::register_with_config(&mut jinja, *config)
        }
        Some(StdlibRegistration::ManifestQuery) => {
            Ok(crate::stdlib::register_manifest_query(&mut jinja))
        }
        None => crate::stdlib::register(&mut jinja),
    }?;
    register_manifest_vars(&doc, &mut jinja, name)?;
    notify_stage(on_stage, ManifestLoadStage::TemplateExpansion);
    register_manifest_macros_with_budget(&doc, &mut jinja, &budget)?;
    let expansion_report = expand_foreach_with_budget(&mut doc, &jinja, &budget)?;
    if let Some(observe_expansion_report) = expansion_report_observer {
        observe_expansion_report(&expansion_report);
    }
    notify_stage(on_stage, ManifestLoadStage::FinalRendering);
    let manifest: NetsukeManifest =
        serde_json::from_value(doc).map_err(|error| ManifestError::Parse {
            source: map_data_error(localize_recipe_error(error), name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;
    let rendered_manifest = if is_manifest_query {
        render::render_manifest_for_manifest_query_with_budget(manifest, &jinja, &budget)?
    } else {
        render::render_manifest_with_budget(manifest, &jinja, &budget)?
    };
    rendered_manifest
        .validate_recipes()
        .map_err(|detail| ManifestError::Parse {
            source: map_data_error(serde_json::Error::custom(detail), name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;
    Ok(rendered_manifest)
}
/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    from_str_with_env(yaml, &process_env_reader())
}
/// Parse a manifest string with an explicit environment reader.
///
/// Lets a caller — in practice a test — drive the `env()` helper without
/// touching the process environment.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
///
/// # Examples
///
/// ```
/// use netsuke::{
///     ast::Recipe,
///     manifest::{EnvReadError, EnvReader, from_str_with_env},
/// };
/// use std::sync::Arc;
///
/// let reader: EnvReader = Arc::new(|name| match name {
///     "PROFILE" => Ok("release".to_owned()),
///     _ => Err(EnvReadError::NotPresent),
/// });
/// let yaml = concat!(
///     "netsuke_version: 1.0.0\n",
///     "targets:\n",
///     "  - name: build\n",
///     "    command: echo {{ env('PROFILE') }}\n",
/// );
/// let manifest = from_str_with_env(yaml, &reader).expect("parse manifest");
///
/// assert!(matches!(
///     &manifest.targets[0].recipe,
///     Recipe::Command { command } if command.as_single() == Some("echo release")
/// ));
/// ```
pub fn from_str_with_env(yaml: &str, env_reader: &EnvReader) -> Result<NetsukeManifest> {
    let env_access_policy = EnvAccessPolicy::default();
    from_str_with_env_and_policy(yaml, env_reader, &env_access_policy)
}
/// Parse a manifest string with explicit resource ceilings for focused tests.
///
/// # Errors
///
/// Returns an error if parsing, expansion, or rendering exhausts a limit.
#[cfg(test)]
pub(crate) fn from_str_with_limits(
    yaml: &str,
    budget_limits: ManifestBudgetLimits,
) -> Result<NetsukeManifest> {
    from_str_named(
        yaml,
        ManifestParse {
            name: &ManifestName::new("Netsukefile"),
            stdlib_registration: None,
            env_reader: &process_env_reader(),
            env_access_policy: &EnvAccessPolicy::default(),
            manifest_root: None,
            expansion_report_observer: Some(trace_expansion_report),
            budget_limits,
        },
        &mut None,
    )
}
mod env_reader;
mod env_telemetry;
pub use env_telemetry::{ENV_LOOKUP_OUTCOME_VALUES, ENV_LOOKUP_TOTAL};
#[cfg(test)]
mod tests;
mod workspace;
