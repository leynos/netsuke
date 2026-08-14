//! Manifest string parsing with explicit stdlib configuration.
//!
//! The `from_str`-family entrypoints in the parent module parse with the
//! default stdlib registration. This module hosts the variant that also
//! injects a caller-owned [`StdlibConfig`], so tests can pin behaviour such
//! as `command_available` resolution without mutating the process
//! environment.

use super::{EnvReader, ManifestName, ManifestParse, from_str_named};
use crate::{ast::NetsukeManifest, stdlib::StdlibConfig};
use anyhow::Result;

/// Parse a manifest string with an explicit environment reader and stdlib
/// configuration.
///
/// Combines the [`EnvReader`] of
/// [`crate::manifest::from_str_with_env`] with a full
/// [`StdlibConfig`], so a caller — in practice a test — can pin stdlib
/// behaviour such as `command_available` resolution without touching the
/// process environment or `PATH`. For instance,
/// [`StdlibConfig::with_path_override`] substitutes the host `PATH` the
/// helper searches.
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
///     manifest::{EnvReadError, EnvReader, from_str_with_env_and_config},
///     stdlib::StdlibConfig,
/// };
/// use std::sync::Arc;
///
/// let reader: EnvReader = Arc::new(|name| match name {
///     "PROFILE" => Ok("release".to_owned()),
///     _ => Err(EnvReadError::NotPresent),
/// });
/// let config = StdlibConfig::from_current_dir()
///     .expect("construct stdlib config")
///     .with_path_override("");
/// let yaml = concat!(
///     "netsuke_version: 1.0.0\n",
///     "targets:\n",
///     "  - name: build\n",
///     "    command: echo {{ env('PROFILE') }}\n",
/// );
/// let manifest =
///     from_str_with_env_and_config(yaml, &reader, config).expect("parse manifest");
///
/// assert!(matches!(
///     &manifest.targets[0].recipe,
///     Recipe::Command { command } if command.as_single() == Some("echo release")
/// ));
/// ```
pub fn from_str_with_env_and_config(
    yaml: &str,
    env_reader: &EnvReader,
    stdlib_config: StdlibConfig,
) -> Result<NetsukeManifest> {
    from_str_named(
        yaml,
        ManifestParse {
            name: &ManifestName::new("Netsukefile"),
            stdlib_config: Some(stdlib_config),
            env_reader,
        },
        &mut None,
    )
}
