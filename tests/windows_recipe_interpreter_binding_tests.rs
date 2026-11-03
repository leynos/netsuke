//! Windows recipe interpreters must not be found through executable search.
//!
//! The security finding behind issue #980 is that a generated plan naming a
//! bare `powershell.exe` lets Windows resolve a workspace-controlled file
//! first. These cases arrange exactly that hazard — decoys in the working
//! directory and at the front of `PATH` — and require the plan to keep working
//! without ever launching one of them.
//!
//! Decoys are compiled Rust helpers rather than scripts because Windows only
//! participates in `CreateProcess` search when the file is an executable image.
//! Each decoy writes a distinct marker and exits with a distinct status, so a
//! regression is reported as a marker file or an odd exit code rather than as a
//! silent behaviour change.

#![cfg(windows)]

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use mockable::{DefaultEnv, Env};
use std::path::PathBuf;
use tempfile::TempDir;
use test_support::command_helper::compile_rust_helper;
use test_support::fs as test_fs;
use test_support::netsuke::run_netsuke_in_with_env;

/// Marker the `powershell.exe` decoy writes if Windows selects it.
const POWERSHELL_MARKER: &str = "decoy-powershell-marker.txt";

/// Marker the `bash.exe` decoy writes if Windows selects it.
const BASH_MARKER: &str = "decoy-bash-marker.txt";

/// Exit status of the `powershell.exe` decoy.
const POWERSHELL_DECOY_EXIT: i32 = 41;

/// Exit status of the `bash.exe` decoy.
const BASH_DECOY_EXIT: i32 = 42;

/// Length of the long recipe's padding payload.
///
/// The rendered PowerShell payload is UTF-16LE Base64, so roughly four thirds
/// of twice this length. The value is chosen to push the inline command past
/// Windows' 32,766 UTF-16 code-unit limit while leaving the response-file
/// parent command short, which is the transport this test must exercise.
const LARGE_RECIPE_PADDING: usize = 30_000;

/// Source of the decoy that masquerades as Windows PowerShell.
const POWERSHELL_DECOY_SOURCE: &str = concat!(
    "fn main() {\n",
    "    std::fs::write(\"decoy-powershell-marker.txt\", \"powershell decoy executed\")\n",
    "        .expect(\"write powershell decoy marker\");\n",
    "    std::process::exit(41);\n",
    "}\n",
);

/// Source of the decoy that masquerades as Bash.
const BASH_DECOY_SOURCE: &str = concat!(
    "fn main() {\n",
    "    if std::env::args().any(|argument| argument == \"--version\") {\n",
    "        println!(\"decoy bash 0.0\");\n",
    "        return;\n",
    "    }\n",
    "    std::fs::write(\"decoy-bash-marker.txt\", \"bash decoy executed\")\n",
    "        .expect(\"write bash decoy marker\");\n",
    "    std::process::exit(42);\n",
    "}\n",
);

/// Every decoy executable, grouped with the directory that contains it.
struct Decoys {
    /// Directory holding both decoys, placed first on the child's `PATH`.
    path_directory: TempDir,
    /// Executable that would answer an unqualified `powershell.exe` lookup.
    powershell: Utf8PathBuf,
    /// Executable that would answer an unqualified `bash.exe` lookup.
    bash: Utf8PathBuf,
}

/// Compile both decoys into one directory that callers put first on `PATH`.
///
/// # Errors
/// Return an error when the temporary directory or either helper cannot be
/// created.
fn compile_decoys() -> Result<Decoys> {
    let temp = tempfile::tempdir().context("create decoy directory")?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .map_err(|path| anyhow::anyhow!("decoy path {} is not UTF-8", path.display()))?;
    let dir = Dir::open_ambient_dir(&root, ambient_authority()).context("open decoy directory")?;
    let powershell = compile_rust_helper(&dir, &root, "powershell", POWERSHELL_DECOY_SOURCE)
        .context("compile powershell decoy")?;
    let bash = compile_rust_helper(&dir, &root, "bash", BASH_DECOY_SOURCE)
        .context("compile bash decoy")?;
    Ok(Decoys {
        path_directory: temp,
        powershell,
        bash,
    })
}

/// Return the workspace manifest with a short and a long recipe.
///
/// The long recipe keeps padding inside a PowerShell string literal rather than
/// a `#` comment: a YAML comment would be stripped before the recipe reached
/// the renderer, so the command would never become long.
fn manifest() -> String {
    let padding = "x".repeat(LARGE_RECIPE_PADDING);
    format!(
        "netsuke_version: \"1.0.0\"\n\
         targets:\n\
         \x20 - name: small\n\
         \x20   command: Set-Content -LiteralPath 'small output.txt' -Value 'small-ok'\n\
         \x20   description: short recipe\n\
         \x20 - name: large\n\
         \x20   command: Set-Content -LiteralPath 'large output.txt' -Value 'large-ok'; Write-Output '{padding}' | Out-Null\n\
         \x20   description: long recipe\n"
    )
}

/// Locate a host executable by name on the inherited `PATH`.
fn host_executable(name: &str) -> Option<Utf8PathBuf> {
    let path = DefaultEnv.os_string("PATH")?;
    for entry in std::env::split_paths(&path) {
        let candidate = entry.join(name);
        if candidate.is_file() {
            return Utf8PathBuf::from_path_buf(candidate).ok();
        }
    }
    None
}

/// Report a skipped case, or fail when CI declared the dependency mandatory.
///
/// # Errors
/// Return an error only when `NETSUKE_REQUIRE_NINJA=1` turns the skip into a
/// failure.
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable prerequisite must be visible in the captured test output instead of passing silently"
)]
fn skip_or_require_ninja(reason: &str) -> Result<()> {
    if DefaultEnv
        .os_string("NETSUKE_REQUIRE_NINJA")
        .is_some_and(|value| value == "1")
    {
        bail!("Ninja is required for this test run: {reason}");
    }
    eprintln!("skipped: {reason}");
    Ok(())
}

/// Windows variables a child process needs before it can start an executable.
///
/// The runner helper clears the environment, which is right for hermetic
/// assertions about search paths but wrong for actually executing Windows
/// programs: without the system root DLL loading fails, and without a temp
/// directory `std::env::temp_dir()` falls back to `C:\Windows`. Neither
/// variable participates in executable search, which is all this test
/// constrains, so they pass through unmodified.
const INHERITED_WINDOWS_VARIABLES: [&str; 12] = [
    "SystemRoot",
    "windir",
    "SystemDrive",
    "TEMP",
    "TMP",
    "PATHEXT",
    "COMSPEC",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
];

/// Build the child environment that exposes the decoys.
fn decoy_environment(decoys: &Decoys, ninja: &Utf8PathBuf) -> Result<Vec<(String, String)>> {
    let real_path = DefaultEnv
        .string("PATH")
        .context("host PATH is required to run ninja")?;
    let decoy_path = decoys.path_directory.path().to_string_lossy();
    let path = std::env::join_paths(
        std::iter::once(PathBuf::from(decoy_path.as_ref()))
            .chain(std::env::split_paths(&real_path)),
    )
    .context("assemble child PATH")?
    .to_string_lossy()
    .into_owned();
    let mut environment: Vec<(String, String)> = Vec::new();
    for name in INHERITED_WINDOWS_VARIABLES {
        if let Some(value) = DefaultEnv.string(name) {
            environment.push((name.to_owned(), value));
        }
    }
    environment.push(("PATH".to_owned(), path));
    environment.push(("NETSUKE_NINJA".to_owned(), ninja.to_string()));
    Ok(environment)
}

/// Borrow an environment table as the string pairs the runner expects.
fn borrowed_environment(environment: &[(String, String)]) -> Vec<(&str, &str)> {
    environment
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect()
}

/// Assert that a decoy marker was never written.
///
/// # Errors
/// Return an error naming the marker that appeared.
fn assert_marker_absent(workspace: &std::path::Path, marker: &str) -> Result<()> {
    ensure!(
        !test_fs::exists(workspace.join(marker)),
        "Windows selected a decoy executable: {marker} exists"
    );
    Ok(())
}

/// A PowerShell build ignores decoys in the working directory and on `PATH`.
///
/// The short recipe exercises the inline `-EncodedCommand` transport; the long
/// recipe exceeds the Windows command-line limit and exercises the response
/// file transport. Both must complete with the real Windows PowerShell while
/// neither decoy runs.
#[test]
fn windows_power_shell_binding_ignores_executable_search_decoys() -> Result<()> {
    let Some(ninja) = host_executable("ninja.exe") else {
        return skip_or_require_ninja("no ninja.exe on PATH");
    };
    let workspace = tempfile::tempdir().context("create workspace")?;
    let decoys = compile_decoys()?;

    // The working-directory decoy is what a bare name would find after the
    // application directory; the PATH decoy covers the final search step.
    test_fs::copy(&decoys.powershell, workspace.path().join("powershell.exe"))
        .context("copy powershell decoy into the working directory")?;
    test_fs::copy(&decoys.bash, workspace.path().join("bash.exe"))
        .context("copy bash decoy into the working directory")?;
    test_fs::write(workspace.path().join("Netsukefile"), manifest())
        .context("write Windows recipe manifest")?;

    let environment = decoy_environment(&decoys, &ninja)?;
    let borrowed = borrowed_environment(&environment);
    let run = run_netsuke_in_with_env(workspace.path(), &["build", "small", "large"], &borrowed)?;
    ensure!(
        run.success,
        "Windows PowerShell recipe failed with decoys present\n\
         --- stdout ---\n{}\n\
         --- stderr ---\n{}",
        run.stdout,
        run.stderr
    );

    let small = test_fs::read_to_string(workspace.path().join("small output.txt"))?;
    ensure!(
        small.trim() == "small-ok",
        "the short recipe produced {small:?}"
    );
    let large = test_fs::read_to_string(workspace.path().join("large output.txt"))?;
    ensure!(
        large.trim() == "large-ok",
        "the long recipe produced {large:?}"
    );
    assert_marker_absent(workspace.path(), POWERSHELL_MARKER)?;
    assert_marker_absent(workspace.path(), BASH_MARKER)
}

/// The generated plan binds Windows PowerShell by an absolute quoted path.
#[test]
fn windows_power_shell_plan_carries_the_trusted_absolute_binding() -> Result<()> {
    let workspace = tempfile::tempdir().context("create workspace")?;
    let decoys = compile_decoys()?;
    test_fs::copy(&decoys.powershell, workspace.path().join("powershell.exe"))
        .context("copy powershell decoy into the working directory")?;
    test_fs::write(workspace.path().join("Netsukefile"), manifest())
        .context("write Windows recipe manifest")?;

    let run = run_netsuke_in_with_env(workspace.path(), &["generate"], &[])?;
    ensure!(run.success, "generate failed: {}", run.stderr);
    let command = run
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("  command = "))
        .context("generated Ninja should carry a command binding")?;
    ensure!(
        command.starts_with('"'),
        "the PowerShell parent should be quoted: {command}"
    );
    ensure!(
        command.contains("WindowsPowerShell") && command.contains("powershell.exe\""),
        "the PowerShell parent should be the system-directory executable: {command}"
    );
    ensure!(
        !command.starts_with("powershell.exe") && !command.starts_with("\"powershell.exe\""),
        "the plan must not resolve PowerShell through executable search: {command}"
    );
    Ok(())
}

/// Bash generation fails rather than emitting an unbound plan.
///
/// The empty `PATH` is deliberate: a `bash.exe` under test control would be
/// selected by the documented `PATH` policy, so its presence would not model a
/// missing runtime.
#[test]
fn windows_bash_generation_requires_a_resolved_runtime() -> Result<()> {
    let workspace = tempfile::tempdir().context("create workspace")?;
    test_fs::write(workspace.path().join("Netsukefile"), manifest())
        .context("write Windows recipe manifest")?;
    let empty_path_directory = tempfile::tempdir().context("create empty PATH directory")?;
    let empty_path = empty_path_directory.path().to_string_lossy().into_owned();
    let plan = workspace.path().join("unbound-plan.ninja");
    let plan_text = plan.to_string_lossy().into_owned();
    let environment = vec![
        ("NETSUKE_WINDOWS_SHELL", "bash"),
        ("PATH", empty_path.as_str()),
    ];

    let run = run_netsuke_in_with_env(
        workspace.path(),
        &["generate", "--output", plan_text.as_str()],
        &environment,
    )?;
    ensure!(
        !run.success,
        "Bash generate unexpectedly succeeded: {}",
        run.stdout
    );
    ensure!(
        run.stderr.contains("PATH contains no bash.exe"),
        "the diagnostic should name the missing runtime: {}",
        run.stderr
    );
    ensure!(
        !test_fs::exists(&plan),
        "a failed Bash generate must not emit an unbound plan"
    );
    assert_marker_absent(workspace.path(), BASH_MARKER)
}

/// A real Bash on `PATH` is bound into the plan by its absolute path.
#[test]
#[expect(
    clippy::print_stderr,
    reason = "test harness: an unavailable prerequisite must be visible in the captured test output instead of passing silently"
)]
fn windows_bash_plan_binds_the_absolute_path() -> Result<()> {
    let Some(bash) = host_executable("bash.exe") else {
        eprintln!("skipped: no real bash.exe on PATH");
        return Ok(());
    };
    let workspace = tempfile::tempdir().context("create workspace")?;
    test_fs::write(workspace.path().join("Netsukefile"), manifest())
        .context("write Windows recipe manifest")?;

    // Only the real Bash directory is exposed, so no decoy can be selected.
    let bash_directory = bash
        .parent()
        .context("bash.exe should have a parent directory")?
        .to_string();
    let environment = vec![
        ("NETSUKE_WINDOWS_SHELL", "bash"),
        ("PATH", bash_directory.as_str()),
    ];
    let run = run_netsuke_in_with_env(workspace.path(), &["generate"], &environment)?;
    ensure!(run.success, "Bash generate failed: {}", run.stderr);
    let expected = format!("\"{bash}\" -e -c ");
    ensure!(
        run.stdout.contains(&expected),
        "the plan should bind Bash by its absolute path: {}",
        run.stdout
    );
    Ok(())
}

/// The decoys carry distinct markers and exit statuses.
///
/// Running the Bash decoy here proves the helper compiled and that a decoy
/// launched by mistake would be identifiable independently of the PowerShell
/// one.
#[test]
fn bash_decoy_exits_with_its_distinct_status() -> Result<()> {
    let decoys = compile_decoys()?;
    let status = std::process::Command::new(decoys.bash.as_std_path())
        .status()
        .context("run bash decoy")?;
    ensure!(
        status.code() == Some(BASH_DECOY_EXIT),
        "bash decoy exited with {status:?}"
    );
    ensure!(
        BASH_DECOY_EXIT != POWERSHELL_DECOY_EXIT,
        "decoys should have distinct exit statuses"
    );
    Ok(())
}
