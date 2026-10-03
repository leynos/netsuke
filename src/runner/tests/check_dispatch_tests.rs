//! Dispatch coverage for `netsuke check`.
//!
//! `check` analyses a manifest without building it, so it must be routed
//! before the runner resolves the Ninja program or the recipe shell.

use super::run_with_ninja_program_resolver;
use crate::cli::{CheckArgs, Cli, Commands};
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use std::cell::Cell;
use test_support::{localizer_test_lock, set_en_localizer};

/// Dispatch `check --explain` before resolving build-only dependencies.
#[test]
fn check_bypasses_ninja_program_and_recipe_shell_resolution() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_en_localizer();
    let cli = Cli {
        command: Some(Commands::Check(CheckArgs {
            explain: Some(String::new()),
            ..CheckArgs::default()
        })),
        ..Cli::default()
    };
    let resolver_called = Cell::new(false);

    run_with_ninja_program_resolver(&cli, crate::output_prefs::resolve(None), None, || {
        resolver_called.set(true);
        Utf8PathBuf::from("ninja")
    })?;

    ensure!(
        !resolver_called.get(),
        "check must not resolve the Ninja program or recipe shell"
    );
    Ok(())
}
