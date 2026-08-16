//! Compile-pass fixture for the cached configuration-discovery API.
//!
//! Compiled by `tests/command_env_ui_tests.rs` against the `netsuke` rlib with
//! `--emit=metadata`. It proves an external caller can inject configuration
//! environment access, retain a discovery outcome, emit diagnostics, transfer
//! its cached layers, and pass them to the full merge without rediscovery.

use netsuke::cli::{
    Cli, ConfigEnvProvider, merge_with_cached_file_layers,
    resolve_json_and_layers_outcome_with_env, resolve_json_and_layers_with_env,
};
use std::{ffi::OsString, sync::Arc};

struct EmbeddedConfigEnv;

impl ConfigEnvProvider for EmbeddedConfigEnv {
    fn get(&self, _: &str) -> Option<OsString> { None }
}

fn main() {
    let localizer = Arc::from(netsuke::cli_localization::build_localizer(None));
    let (cli, matches) =
        netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer).expect("parse args");
    let env = EmbeddedConfigEnv;

    let (_, outcome) = resolve_json_and_layers_with_env(&cli, &matches, &env);
    outcome.emit_diagnostics();
    let _ = merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers());

    let (_, outcome) = resolve_json_and_layers_outcome_with_env(&cli, &matches, &env);
    outcome.emit_diagnostics();
    let _ = merge_with_cached_file_layers(&cli, &matches, &env, outcome.into_layers());

    let _: Cli = Cli::default();
}
