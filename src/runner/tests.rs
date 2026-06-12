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

#[rstest]
fn generation_steps_run_without_reporter() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let manifest_path = temp.path().join("Netsukefile");
    test_support::fs::write(
        &manifest_path,
        "netsuke_version: \"1.0.0\"\ntargets:\n  - name: hello\n    command: echo hi\n",
    )?;
    let utf8_path = camino::Utf8PathBuf::from_path_buf(manifest_path)
        .map_err(|path| anyhow::anyhow!("non-UTF-8 temp path: {}", path.display()))?;

    // The pure pipeline composes without a runner status reporter.
    let manifest =
        generation::load_manifest(&utf8_path, crate::stdlib::NetworkPolicy::default(), None)?;
    let graph = generation::build_graph(&manifest)?;
    let (ninja_text, _) = generation::ninja_text(&graph)?.into_parts();
    anyhow::ensure!(
        ninja_text.contains("build hello:"),
        "expected generated Ninja to contain the hello build edge:\n{}",
        ninja_text
    );
    Ok(())
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
