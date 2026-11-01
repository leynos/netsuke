//! Manifest string loaders and their compatibility adapters.
//!
//! Every entry point here parses a manifest already held in memory, so none of
//! them opens a workspace. [`from_str_with_env`] delegates to the
//! options-carrying core with a caller-supplied reader; [`from_str`] is the
//! ambient adapter over it, and `from_str_with_limits` is the test-only adapter
//! that narrows the resource ceilings. The variants that also inject a
//! [`StdlibConfig`](crate::stdlib::StdlibConfig) live in
//! [`super::parse_with_config`]; the file-path equivalents live in
//! [`super::path_loaders`].

use super::{EnvAccessPolicy, EnvReader, from_str_with_env_and_policy, process_env_reader};
#[cfg(test)]
use super::{
    ManifestBudgetLimits, ManifestEnvironment, ManifestLoadOptions, ManifestName,
    from_str_with_options,
};
use crate::ast::NetsukeManifest;
use anyhow::Result;

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
///     Recipe::Command { command, .. } if command.as_single() == Some("echo release")
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
    let reader = process_env_reader();
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::ambient(environment).with_budget_limits(budget_limits),
    )
}
