//! Unit tests for runner path resolution, predicate helpers, and core helpers.

use super::*;
use rstest::rstest;
use std::path::PathBuf;

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
#[case(OutputMode::Standard, true, false)]
#[case(OutputMode::Standard, false, true)]
#[case(OutputMode::Accessible, true, true)]
#[case(OutputMode::Accessible, false, true)]
fn force_text_task_updates_when_required(
    #[case] mode: OutputMode,
    #[case] stdout_is_tty: bool,
    #[case] expected: bool,
) {
    assert_eq!(
        should_force_text_task_updates(mode, stdout_is_tty),
        expected
    );
}

#[rstest]
fn generation_steps_run_without_reporter() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::write(
        &manifest_path,
        "netsuke_version: \"1.0.0\"\ntargets:\n  - name: hello\n    command: echo hi\n",
    )?;
    let utf8_path = camino::Utf8PathBuf::from_path_buf(manifest_path)
        .map_err(|path| anyhow::anyhow!("non-UTF-8 temp path: {}", path.display()))?;

    // The pure pipeline composes without any StatusReporter in sight.
    let manifest =
        generation::load_manifest(&utf8_path, crate::stdlib::NetworkPolicy::default(), None)?;
    let graph = generation::build_graph(&manifest)?;
    let ninja = generation::ninja_text(&graph)?;
    anyhow::ensure!(
        ninja.as_str().contains("build hello:"),
        "expected generated Ninja to contain the hello build edge:\n{}",
        ninja.as_str()
    );
    Ok(())
}
