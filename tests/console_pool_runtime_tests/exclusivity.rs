//! Verify that Ninja serializes independent console-pool recipes.

use super::{
    ChildCleanup, finish_successfully, netsuke_build, runtime_workspace, spawn_line_reader,
    wait_for_marker, workspace_dir,
};
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;

/// Keep two console recipes from overlapping under a parallel Ninja job limit.
#[test]
fn console_edges_are_exclusive_with_parallel_jobs_enabled() -> Result<()> {
    let Some(workspace) = runtime_workspace() else {
        return Ok(());
    };
    let root = workspace_dir(&workspace)?;
    let first_release = Utf8PathBuf::from(".release-one");
    let second_release = Utf8PathBuf::from(".release-two");
    write_console_exclusivity_manifest(&root)?;

    let mut command = netsuke_build(&workspace, &["--jobs", "2", "build", "one", "two"])?;
    let child = command
        .spawn()
        .context("spawn Netsuke console exclusivity build")?;
    let mut cleanup = ChildCleanup::new(
        child,
        root.try_clone()?,
        vec![first_release.clone(), second_release.clone()],
    );
    let stdout = cleanup
        .child
        .stdout
        .take()
        .context("Netsuke child should expose stdout")?;
    let (output, reader) = spawn_line_reader(stdout);

    let first_start = wait_for_marker(&output, "START:")?;
    let first_output = parse_start_output(&first_start)?;
    release_edge(&root, &first_output)?;
    let second_start = wait_for_marker(&output, "START:")?;
    let second_output = parse_start_output(&second_start)?;
    ensure!(
        first_output != second_output,
        "the two console edges should each start once"
    );
    release_edge(&root, &second_output)?;
    finish_successfully(&mut cleanup)?;
    reader
        .join()
        .map_err(|_| anyhow::anyhow!("stdout forwarding thread panicked"))?;
    for target in ["one", "two"] {
        ensure!(
            root.exists(target),
            "console edge {target} should complete after release"
        );
    }
    ensure!(
        !root.exists(".console-running"),
        "the shared sentinel should be removed after both recipes finish"
    );
    Ok(())
}

/// Write the shared-sentinel recipes for the console exclusivity scenario.
///
/// Keep this fixture private to the integration-test module because its marker
/// and release-file protocol is specific to the two edges under test.
///
/// # Errors
///
/// Returns an error when the capability directory cannot write the manifest.
fn write_console_exclusivity_manifest(root: &Dir) -> Result<()> {
    root.write(
        "Netsukefile",
        concat!(
            "netsuke_version: \"1.0.0\"\n",
            "targets:\n",
            "  - name: one\n",
            "    command: >-\n",
            "      mkdir .console-running 2>/dev/null || { echo COLLISION >&2; exit 1; };\n",
            "      printf 'START:one\\n';\n",
            "      while [ ! -f .release-one ]; do sleep 0.02; done;\n",
            "      rmdir .console-running; touch one; printf 'DONE:one\\n'\n",
            "    console: true\n",
            "  - name: two\n",
            "    command: >-\n",
            "      mkdir .console-running 2>/dev/null || { echo COLLISION >&2; exit 1; };\n",
            "      printf 'START:two\\n';\n",
            "      while [ ! -f .release-two ]; do sleep 0.02; done;\n",
            "      rmdir .console-running; touch two; printf 'DONE:two\\n'\n",
            "    console: true\n",
        ),
    )
    .context("write exclusive console manifest")?;
    Ok(())
}

/// Release one exclusivity-test recipe by its declared output name.
///
/// # Errors
///
/// Returns an error for unexpected output names or failed release-file writes.
fn release_edge(workspace: &Dir, output: &str) -> Result<()> {
    ensure!(
        matches!(output, "one" | "two"),
        "unexpected output marker {output}"
    );
    workspace
        .write(format!(".release-{output}"), b"release")
        .with_context(|| format!("release console edge {output}"))?;
    Ok(())
}

/// Extract and validate the target name printed by the shared recipe.
///
/// # Errors
///
/// Returns an error when the marker is absent or does not name a test output.
fn parse_start_output(line: &str) -> Result<String> {
    let output = line
        .split_once("START:")
        .map(|(_, output)| output.trim())
        .context("console recipe start marker should include its output")?;
    ensure!(
        matches!(output, "one" | "two"),
        "unexpected console output {output}"
    );
    Ok(output.to_owned())
}
