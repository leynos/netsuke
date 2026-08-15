//! Compile-pass fixture for Netsuke's public cached configuration API.
//!
//! Cargo resolves `mockable` alongside `netsuke-build`, so this verifies the
//! public `mockable::Env` boundary exactly as an external embedder uses it.

use mockable::DefaultEnv;
use netsuke::{cli, cli_localization};
use std::sync::Arc;

fn compose_cached_configuration_flow() {
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (parsed, matches) =
        match cli::parse_with_localizer_from(["netsuke", "generate"], &localizer) {
            Ok(parsed) => parsed,
            Err(_) => return,
        };
    let env = DefaultEnv;

    drop(cli::resolve_merged_json_with_env(&parsed, &matches, &env));
    drop(cli::resolve_json_and_layers_with_env(&parsed, &matches, &env));
    let (result, outcome) = cli::resolve_json_and_layers_outcome_with_env(&parsed, &matches, &env);
    outcome.emit_diagnostics();
    let layers = outcome.into_layers();
    drop(result);
    drop(cli::merge_with_layers(&parsed, &matches, &env, layers));
    drop(cli::merge_with_config_and_env(&parsed, &matches, &env));
}

fn main() {
    let _ = compose_cached_configuration_flow;
}
