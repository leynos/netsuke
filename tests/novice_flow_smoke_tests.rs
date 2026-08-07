//! Smoke tests for newcomer-facing CLI flows.

// `bail!` is reached only from the Unix-only non-UTF-8 test, so the import is
// gated alongside `PathBuf`; left unconditional it would be an unused import on
// other targets, which `-D warnings` turns into a build failure.
#[cfg(unix)]
use anyhow::bail;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use tempfile::{TempDir, tempdir};
use test_support::check_ninja;
use test_support::fluent::normalize_fluent_isolates;
use test_support::netsuke::{run_netsuke_in, run_netsuke_in_with_env};

/// Captured output from a netsuke invocation, with normalized Fluent isolates.
struct CommandOutput {
    stdout: String,
    stderr: String,
    success: bool,
}

/// Run `netsuke` in `current_dir` with supplied args and optional `NINJA_ENV`.
///
/// Output is normalized to strip Fluent isolation markers.
fn run_netsuke(
    current_dir: &Path,
    args: &[&str],
    ninja_env: Option<&Path>,
) -> Result<CommandOutput> {
    // `NETSUKE_NINJA` is set as a string, so a lossy conversion would silently
    // point the child at a different executable; fail loudly instead.
    let ninja = ninja_env
        .map(|path| {
            path.to_str().with_context(|| {
                format!("ninja override path is not valid UTF-8: {}", path.display())
            })
        })
        .transpose()?;
    let run = match ninja {
        Some(path) => run_netsuke_in_with_env(current_dir, args, &[("NETSUKE_NINJA", path)])?,
        None => run_netsuke_in(current_dir, args)?,
    };
    Ok(CommandOutput {
        stdout: normalize_fluent_isolates(&run.stdout),
        stderr: normalize_fluent_isolates(&run.stderr),
        success: run.success,
    })
}

fn setup_minimal_workspace(context: &str) -> Result<TempDir> {
    let temp = tempdir().with_context(|| format!("create temp dir for {context}"))?;
    let manifest = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest)
        .with_context(|| format!("copy minimal manifest to {}", manifest.display()))?;
    Ok(temp)
}

fn assert_contains_all(haystack: &str, fragments: &[&str], label: &str) -> Result<()> {
    for fragment in fragments {
        ensure!(
            haystack.contains(fragment),
            "expected {label} to contain '{fragment}', got:\n{haystack}"
        );
    }
    Ok(())
}

/// A non-UTF-8 override must fail loudly rather than being mangled.
///
/// `NETSUKE_NINJA` is forwarded to the child as a string, so a lossy conversion
/// would silently point it at a different executable. Unix-only: constructing a
/// non-UTF-8 path requires POSIX byte semantics.
#[cfg(unix)]
#[test]
fn non_utf8_ninja_override_is_rejected() -> Result<()> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let workspace = setup_minimal_workspace("novice smoke non-UTF-8 ninja")?;
    let non_utf8 = PathBuf::from(OsString::from_vec(b"ninja-\xff".to_vec()));

    let Err(error) = run_netsuke(workspace.path(), &[], Some(non_utf8.as_path())) else {
        bail!("a non-UTF-8 ninja override should not be accepted")
    };

    let message = format!("{error:?}");
    ensure!(
        message.contains("not valid UTF-8"),
        "error should name the non-UTF-8 override, got {message}"
    );
    Ok(())
}

#[test]
fn first_run_without_args_succeeds_in_minimal_workspace() -> Result<()> {
    let workspace = setup_minimal_workspace("novice smoke first run")?;
    let (_ninja_dir, ninja_path) = check_ninja::fake_ninja_check_build_file()?;

    let output = run_netsuke(workspace.path(), &[], Some(ninja_path.as_path()))?;

    ensure!(
        output.success,
        "expected bare netsuke invocation to succeed"
    );
    assert_contains_all(&output.stderr, &["Stage 6/6", "Build complete."], "stderr")?;
    Ok(())
}

#[test]
fn missing_manifest_error_matches_documented_guidance() -> Result<()> {
    let workspace = tempdir().context("create temp dir for missing manifest smoke test")?;

    let output = run_netsuke(workspace.path(), &[], None)?;

    ensure!(!output.success, "expected bare netsuke invocation to fail");
    assert_contains_all(
        &output.stderr,
        &[
            "Manifest 'Netsukefile' not found in the current directory.",
            "Ensure the manifest exists or pass `--file` with the correct path.",
        ],
        "stderr",
    )?;
    Ok(())
}

#[rstest]
#[case::flag(&["--locale", "en-US", "--help"])]
#[case::subcommand(&["--locale", "en-US", "help"])]
fn help_entry_points_are_novice_friendly(#[case] args: &[&str]) -> Result<()> {
    let output = run_netsuke(Path::new("."), args, None)?;

    ensure!(output.success, "expected help entry point to succeed");
    assert_contains_all(
        &output.stdout,
        &[
            "Netsuke transforms",
            "YAML + Jinja",
            "build",
            "Build targets",
            "clean",
            "Remove build artefacts",
            "graph",
            "dependency graph",
            "generate",
            "Generate the Ninja manifest",
        ],
        "stdout",
    )?;
    Ok(())
}

#[test]
fn localized_help_still_flows_through_cli_localization() -> Result<()> {
    let output = run_netsuke(Path::new("."), &["--locale", "es-ES", "--help"], None)?;

    ensure!(output.success, "expected localized help to succeed");
    assert_contains_all(
        &output.stdout,
        &[
            "Netsuke transforma",
            "YAML + Jinja",
            "build",
            "Compila objetivos",
        ],
        "stdout",
    )?;
    Ok(())
}
