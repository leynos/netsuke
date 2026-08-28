//! Explicit configuration-selector resolution and `-C` anchoring.
//!
//! The selector query answers one question: which configuration file did the
//! operator select, and at what path? `--config` outranks `NETSUKE_CONFIG`;
//! an absolute winner is used unchanged, a relative winner is anchored to
//! `-C/--directory` when supplied (ADR-014), and without `-C` it stays as
//! written so it resolves against the process working directory at load time.
//! The query is pure and emits no tracing; discovery traces the result later.

use std::path::{Path, PathBuf};

use super::environment::EnvProvider;

/// Name of the environment variable that selects the configuration file.
///
/// Read as the primary selector after the `--config` CLI flag when a path is
/// not given explicitly.
pub(super) const CONFIG_ENV_VAR: &str = "NETSUKE_CONFIG";

/// Describes the result of the pure explicit-path selection query.
///
/// Records the winning selector, its optional path, and every environment
/// lookup evaluated to reach the decision, so a caller can emit diagnostics
/// afterwards without giving the query tracing side effects.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ConfigPathResolution {
    /// Configuration selector that resolved the path.
    pub(super) selector: &'static str,
    /// Bounded resolved configuration path, or `None` when unset.
    pub(super) path: Option<PathBuf>,
    /// Environment variables consulted during resolution, with their results.
    pub(super) environment_lookups: Vec<(&'static str, Option<PathBuf>)>,
}

/// Select an explicit config path, giving `--config` precedence over `env`.
///
/// A thin wrapper over [`resolve_config_selector_anchored`] for callers that
/// need only the winning path. Like that query it performs no tracing;
/// discovery returns bounded diagnostics for composition boundaries to emit
/// later.
///
/// Production code takes the richer [`ConfigPathResolution`] so it can trace
/// the environment lookups, leaving this as a convenience for precedence
/// tests.
#[cfg(test)]
pub(super) fn explicit_config_path_with_env(
    cli: &super::Cli,
    env: &impl EnvProvider,
) -> Option<PathBuf> {
    resolve_config_selector_anchored(cli.config.clone(), cli.directory.as_deref(), env).path
}

/// Anchor an explicit configuration selector to `-C/--directory`.
///
/// An absolute selector is returned unchanged: the operator pointed at an
/// exact file. A relative selector is joined onto `directory` when one is
/// supplied, so `--config relative.toml` with `-C project` loads
/// `project/relative.toml`. Without `-C` the selector is returned as written
/// and resolves against the process working directory at load time.
fn resolve_explicit_config_path(selector: PathBuf, directory: Option<&Path>) -> PathBuf {
    match directory {
        // An absolute selector is returned unchanged: the operator pointed at
        // an exact file, which `-C` never re-anchors.
        Some(dir) if !selector.is_absolute() => dir.join(selector),
        // Without `-C` the selector is returned as written and resolves
        // against the process working directory at load time.
        _ => selector,
    }
}

/// Select a config path from the CLI flag, then `NETSUKE_CONFIG` via `env`,
/// without `-C` anchoring.
///
/// Test-only convenience for tracing coverage of the unanchored selection
/// rule; production composition always calls the anchored variant with the
/// `Cli::directory` value. `cli_config` wins when present, in which case no
/// environment lookup is recorded because none is performed. This query emits
/// no tracing.
#[cfg(test)]
pub(super) fn resolve_config_selector(
    cli_config: Option<PathBuf>,
    env: &impl EnvProvider,
) -> ConfigPathResolution {
    resolve_config_selector_anchored(cli_config, None, env)
}

/// Select a config path from the CLI flag, then `NETSUKE_CONFIG` via `env`,
/// anchoring a relative winner to `directory` (`-C/--directory`).
///
/// `cli_config` wins when present, in which case no environment lookup is
/// recorded because none is performed. Anchoring applies to both selectors:
/// an absolute path is used unchanged, a relative path is joined onto
/// `directory` when supplied, and without a directory the path is kept as
/// written so it resolves against the process working directory at load time.
/// This query emits no tracing.
pub(super) fn resolve_config_selector_anchored(
    cli_config: Option<PathBuf>,
    directory: Option<&Path>,
    env: &impl EnvProvider,
) -> ConfigPathResolution {
    if let Some(path) = cli_config {
        return ConfigPathResolution {
            selector: "cli_flag",
            path: Some(resolve_explicit_config_path(path, directory)),
            environment_lookups: Vec::new(),
        };
    }

    let primary_path = env_config_path(env, CONFIG_ENV_VAR)
        .map(|path| resolve_explicit_config_path(path, directory));
    ConfigPathResolution {
        selector: primary_path.as_ref().map_or("none", |_| CONFIG_ENV_VAR),
        environment_lookups: vec![(CONFIG_ENV_VAR, primary_path.clone())],
        path: primary_path,
    }
}

/// Read a non-empty config path from `var_name` through `env`.
///
/// Returns `None` when the variable is unset or empty, so discovery still runs.
/// This query emits no tracing.
pub(super) fn env_config_path(env: &impl EnvProvider, var_name: &str) -> Option<PathBuf> {
    env.get(var_name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
