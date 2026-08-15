//! Unit tests for runner path resolution, predicate helpers, and core helpers.

use super::*;
use crate::cli::{HelpArgs, HelpTopic};
use anyhow::{Result, ensure};
use rstest::rstest;
use std::cell::Cell;
use std::path::Path;
use std::path::PathBuf;
use test_support::{localizer_test_lock, set_en_localizer};

#[rstest]
#[case(None, "out.ninja", "out.ninja")]
#[case(Some("work"), "out.ninja", "work/out.ninja")]
#[case(Some("work"), "/tmp/out.ninja", "/tmp/out.ninja")]
fn resolve_output_path_respects_directory(
    #[case] directory: Option<&str>,
    #[case] input: &str,
    #[case] expected: &str,
) {
    let cli = Cli {
        directory: directory.map(PathBuf::from),
        ..Cli::default()
    };
    let resolved = resolve_output_path(&cli, Path::new(input));
    assert_eq!(resolved.as_ref(), Path::new(expected));
}

#[test]
fn help_targets_bypasses_ninja_program_resolution() -> Result<()> {
    let _lock = localizer_test_lock().map_err(|error| anyhow::anyhow!("{error}"))?;
    let _guard = set_en_localizer();
    let cli = Cli {
        file: PathBuf::from("missing-help-targets-manifest.yml"),
        command: Some(Commands::Help(HelpArgs {
            topic: Some(HelpTopic::Targets),
        })),
        ..Cli::default()
    };
    let resolver_called = Cell::new(false);

    let result =
        run_with_ninja_program_resolver(&cli, crate::output_prefs::resolve(None), None, || {
            resolver_called.set(true);
            PathBuf::from("ninja")
        });

    ensure!(result.is_err(), "missing help manifest should fail");
    ensure!(
        !resolver_called.get(),
        "help targets must not resolve the Ninja program"
    );
    Ok(())
}
