//! The manifest-query stdlib surface stays coherent across helpers.
//!
//! `netsuke help targets` registers a restricted stdlib
//! (`stdlib::register_manifest_query`) instead of the full one. Nothing
//! structurally compares the two surfaces, so a helper whose *arity* is
//! changed in one place and not the other would still render — with the wrong
//! diagnostic. These cases pin the contract for the helper that moved: a
//! disabled helper must report that it is disabled, not that its arguments were
//! wrong, and the helpers the query surface permits must still render.
//!
//! The assertions drive a real `netsuke` process rather than the registration
//! function directly: `register_manifest_query` is crate-private, and the
//! boundary a manifest author actually meets is the command. A target
//! `description` is rendered by both loaders, so it is the field that can
//! distinguish "disabled here" from "wrong arguments everywhere".

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use test_support::fs as test_fs;

/// The marker `src/stdlib/register.rs` attaches to every disabled helper.
const DISABLED_MARKER: &str = "is disabled while rendering";

/// Captured output of one query against `template` as a target description.
struct QueryRun {
    /// Whether the command succeeded.
    success: bool,
    /// Raw stdout, which carries the catalogue on success.
    stdout: String,
    /// Raw stderr, which carries the JSON diagnostic on failure.
    stderr: String,
}

/// Write a one-target manifest whose description is `template`.
fn write_description_manifest(template: &str) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let temp = tempfile::tempdir().context("create manifest-query workspace")?;
    let manifest_path = temp.path().join("Netsukefile");
    test_fs::write(
        &manifest_path,
        format!(
            concat!(
                "netsuke_version: \"1.0.0\"\n",
                "targets:\n",
                "  - name: discovery\n",
                "    description: \"{template}\"\n",
                "    command: echo discovery\n",
            ),
            template = template
        ),
    )
    .context("write manifest-query manifest")?;
    Ok((temp, manifest_path))
}

/// Run `netsuke --json help targets` against `template`.
fn run_query(template: &str) -> Result<QueryRun> {
    let (temp, manifest_path) = write_description_manifest(template)?;
    let output = assert_cmd::cargo::cargo_bin_cmd!("netsuke")
        .current_dir(temp.path())
        .arg("--json")
        .arg("--file")
        .arg(&manifest_path)
        .arg("help")
        .arg("targets")
        .output()
        .context("run netsuke --json help targets")?;
    Ok(QueryRun {
        success: output.status.success(),
        stdout: String::from_utf8(output.stdout).context("stdout should be valid UTF-8")?,
        stderr: String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?,
    })
}

/// Run `template` as a description and return the diagnostic's cause chain.
///
/// A failure is required: the argument-count regression this test exists to
/// exclude is raised while the manifest loads, so a successful run could never
/// be the interesting case.
fn query_manifest_causes(template: &str) -> Result<Vec<String>> {
    let run = run_query(template)?;
    ensure!(
        !run.success,
        "a disabled helper must fail the query: {}",
        run.stdout
    );
    let document: Value = serde_json::from_str(&run.stderr)
        .with_context(|| format!("stderr should be one JSON document: {}", run.stderr))?;
    let causes = document
        .pointer("/diagnostics/0/causes")
        .and_then(Value::as_array)
        .context("diagnostic should carry its cause chain")?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    Ok(causes)
}

/// Assert that `template` renders under the *full* stdlib.
///
/// This is the negative control for [`query_manifest_causes`]: the same
/// template goes through a build load, so a template that fails there too
/// would make the query assertions vacuous.
fn assert_full_stdlib_renders(template: &str) -> Result<()> {
    let (temp, manifest_path) = write_description_manifest(template)?;
    // `--file` is a global option, so it precedes the subcommand.
    let run = test_support::netsuke::run_netsuke_in(
        temp.path(),
        &[
            "--file",
            manifest_path
                .to_str()
                .context("manifest path should be UTF-8")?,
            "generate",
            "--output",
            "out.ninja",
        ],
    )?;
    ensure!(
        run.success,
        "the full stdlib should render {template:?}: {}",
        run.stderr
    );
    Ok(())
}

/// A disabled helper reports that it is disabled, not an argument error.
///
/// The `env` stub's arity changed when `default=` was added. Had the stub kept
/// its single-argument shape, `env('X', default='y')` would have failed with a
/// detail-free `too many arguments`, naming neither the helper nor the remedy.
/// This asserts the specific marker so the arity cannot silently drift again.
#[test]
fn query_surface_reports_env_as_disabled_with_its_keyword_argument() -> Result<()> {
    let causes = query_manifest_causes("{{ env('NETSUKE_QUERY_PROBE', default='y') }}")?;
    let reported = causes.join("\n");
    ensure!(
        reported.contains(DISABLED_MARKER),
        "env should report itself disabled: {causes:?}"
    );
    ensure!(
        reported.contains("env is disabled"),
        "the disabled diagnostic should name the helper: {causes:?}"
    );
    ensure!(
        !reported.contains("too many arguments") && !reported.contains("missing argument"),
        "the env stub must accept the keyword, not reject it by arity: {causes:?}"
    );
    Ok(())
}

/// The `env` stub keeps its disabled diagnostic without any argument at all.
#[test]
fn query_surface_reports_env_as_disabled_without_arguments() -> Result<()> {
    let causes = query_manifest_causes("{{ env('NETSUKE_QUERY_PROBE') }}")?;
    ensure!(
        causes.join("\n").contains("env is disabled"),
        "a bare env call should also report itself disabled: {causes:?}"
    );
    Ok(())
}

/// The negative control: the same template works under the full stdlib.
///
/// Without this, the assertions above would pass for a manifest that fails in
/// both loaders for an unrelated reason.
#[test]
fn the_full_stdlib_renders_the_same_env_call() -> Result<()> {
    assert_full_stdlib_renders("{{ env('NETSUKE_QUERY_PROBE', default='y') }}")
}

/// Helpers the query surface deliberately permits must render there.
///
/// `compact` arrives with EP-M2 and the shell helpers with EP-M4; naming the
/// currently permitted set here keeps the obligation honest as the surface
/// grows, since a helper added to only one registration path is exactly the
/// drift this contract exists to catch.
#[test]
fn query_surface_renders_its_permitted_helpers() -> Result<()> {
    for (template, expected) in [
        ("{{ ['b', 'a'] | sort | join(',') }}", "a,b"),
        ("{{ 'a b' | upper }}", "A B"),
        ("{{ ['a', 'a'] | unique | join(',') }}", "a"),
    ] {
        let run = run_query(template)?;
        ensure!(
            run.success,
            "the query surface should render {template:?}: {}",
            run.stderr
        );
        ensure!(
            run.stdout.contains(expected),
            "the query catalogue should contain {expected:?}: {}",
            run.stdout
        );
        assert_full_stdlib_renders(template)?;
    }
    Ok(())
}
