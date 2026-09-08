//! Manifest loading helpers.
//!
//! This module parses a `Netsukefile` without relying on a global Jinja
//! preprocessing pass. The YAML is parsed first and Jinja expressions are
//! evaluated only within string values or the `foreach` and `when` keys. It
//! exposes `env()` to read environment variables and `glob()` to expand
//! filesystem patterns during template evaluation. Both helpers fail fast when
//! inputs are missing or patterns are invalid.
//!
//! Consumers interact with the intermediate manifest through the re-exported
//! [`ManifestValue`] and [`ManifestMap`] aliases. Diagnostics wrap manifest
//! identifiers in [`ManifestName`] and YAML source strings in
//! [`ManifestSource`] so callers pass domain-specific types instead of raw
//! strings.
//!
//! The optional `vars` section must be a JSON object; lists and scalars fail
//! with `manifest.vars.not_object`; non-string or composite keys fail during
//! the initial `serde_saphyr` parse. Reserved `env`/`glob` names fail with
//! `manifest.vars.reserved_name` because `MiniJinja` shares a global namespace.

use crate::{
    ast::NetsukeManifest,
    localization::{self, keys},
    stdlib::{NetworkPolicy, StdlibConfig},
};
use anyhow::Result;
use minijinja::{Environment, UndefinedBehavior};
use serde::de::Error as _;
use std::{path::Path, sync::Arc};

mod budget;
mod diagnostics;
mod expand;
// `glob_paths` is the module's only boundary: every other item, including the
// `GlobEntryResult` alias, stays module-private. Denying `unreachable_pub`
// here rejects `pub` items that are still unreachable from the crate root, so
// the boundary cannot regress silently. The `glob_paths` re-export below makes
// that one item genuinely reachable and therefore exempt; anything else
// widened to `pub` fails the build.
#[deny(unreachable_pub)]
mod glob;
mod hints;
mod jinja_macros;
mod load_stage;
mod loading;
mod parse_with_config;
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
pub use env_reader::{EnvReadError, EnvReader, process_env_reader};
pub(crate) use expand::expand_foreach_with_budget;
pub use glob::glob_paths;
pub use load_stage::ManifestLoadStage;
use loading::{notify_stage, trace_expansion_report};
pub use parse_with_config::from_str_with_env_and_config;
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
/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
/// Inputs to a manifest parse, bundled to keep the parameter list bounded.
struct ManifestParse<'a> {
    /// Name reported in diagnostics.
    name: &'a ManifestName,
    /// Optional stdlib registration configuration.
    stdlib_registration: Option<StdlibRegistration>,
    /// Environment reader backing the `env()` helper.
    env_reader: &'a EnvReader,
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
fn from_str_named(
    yaml: &str,
    parse: ManifestParse<'_>,
    on_stage: &mut Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    let ManifestParse {
        name,
        stdlib_registration,
        env_reader,
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
            source: map_yaml_error(e, &ManifestSource::from(yaml), name),
            message: localization::message(keys::MANIFEST_PARSE),
        })?;
    let mut jinja = Environment::new();
    jinja.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose custom helpers to templates.
    let reader = Arc::clone(env_reader);
    jinja.add_function("env", move |var_name: String| {
        env_var_with(&var_name, |key| reader(key))
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
    from_str_named(
        yaml,
        ManifestParse {
            name: &ManifestName::new("Netsukefile"),
            stdlib_registration: None,
            env_reader,
            manifest_root: None,
            expansion_report_observer: Some(trace_expansion_report),
            budget_limits: ManifestBudgetLimits::default(),
        },
        &mut None,
    )
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
            manifest_root: None,
            expansion_report_observer: Some(trace_expansion_report),
            budget_limits,
        },
        &mut None,
    )
}
/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    from_path_with_policy(path, NetworkPolicy::default(), None)
}
/// Load a [`NetsukeManifest`] from the given file path using an explicit
/// network policy and an optional stage callback.
///
/// The callback, when provided, is invoked in order for each manifest stage.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
///
/// # Examples
///
/// ```rust,ignore
/// use netsuke::manifest;
/// use netsuke::stdlib::NetworkPolicy;
///
/// let policy = NetworkPolicy::default();
/// let manifest = manifest::from_path_with_policy("Netsukefile", policy, None);
/// assert!(manifest.is_ok());
/// ```
pub fn from_path_with_policy(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_policy_and_limits(path, policy, ManifestBudgetLimits::default(), on_stage)
}
/// Load a manifest with explicit network policy and resource ceilings.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed.
pub fn from_path_with_policy_and_limits(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    budget_limits: ManifestBudgetLimits,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_policy_and_env_and_limits(
        path,
        policy,
        &process_env_reader(),
        budget_limits,
        on_stage,
    )
}
/// Load a manifest with explicit network policy and environment reader.
///
/// This adapter boundary lets callers supply deterministic manifest variables
/// without mutating the process environment.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed.
///
/// # Examples
///
/// ```
/// use netsuke::{
///     ast::Recipe,
///     manifest::{EnvReadError, EnvReader, from_path_with_policy_and_env},
///     stdlib::NetworkPolicy,
/// };
/// use std::{io::Write, sync::Arc};
///
/// let mut file = tempfile::NamedTempFile::new().expect("create manifest");
/// write!(
///     file,
///     "netsuke_version: 1.0.0\ntargets:\n  - name: build\n    command: echo {{{{ env('PROFILE') }}}}\n"
/// )
/// .expect("write manifest");
/// let reader: EnvReader = Arc::new(|name| match name {
///     "PROFILE" => Ok("offline".to_owned()),
///     _ => Err(EnvReadError::NotPresent),
/// });
/// let policy = NetworkPolicy::default().deny_all_hosts();
/// let manifest = from_path_with_policy_and_env(file.path(), policy, &reader, None)
///     .expect("load manifest without network access");
///
/// assert!(matches!(
///     &manifest.targets[0].recipe,
///     Recipe::Command { command } if command.as_single() == Some("echo offline")
/// ));
/// ```
pub fn from_path_with_policy_and_env(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    env_reader: &EnvReader,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    from_path_with_policy_and_env_and_limits(
        path,
        policy,
        env_reader,
        ManifestBudgetLimits::default(),
        on_stage,
    )
}

/// Load a manifest with explicit policy, environment reader, and resource limits.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read, rendered, or parsed.
#[expect(
    clippy::too_many_arguments,
    reason = "This public compatibility entry point keeps policy, environment, budget, and stage-observer seams explicit."
)]
pub fn from_path_with_policy_and_env_and_limits(
    path: impl AsRef<Path>,
    policy: NetworkPolicy,
    env_reader: &EnvReader,
    budget_limits: ManifestBudgetLimits,
    on_stage: Option<&mut dyn FnMut(ManifestLoadStage)>,
) -> Result<NetsukeManifest> {
    query::from_path_with_policy_and_env_and_limits(
        path,
        policy,
        env_reader,
        budget_limits,
        on_stage,
    )
}
mod env_reader;
#[cfg(test)]
mod tests;
mod workspace;
