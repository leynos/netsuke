//! The PowerShell harness that executes a documented Windows `PATH` fence.
//!
//! A fence is run inside the wrapper in [`wrapper`], which owns the user
//! environment registry key: it records the existing `Path` value, installs a
//! sentinel, dot-sources the documented script, reports what was stored and
//! its type, and restores the original value whatever the script does. The
//! child PowerShell gets a cleared environment with only the variables Windows
//! needs to start it, so nothing the host happens to export can decide which
//! directory the example resolves.
//!
//! The harness reads the parent environment through [`mockable::Env`], so the
//! process is the only ambient dependency and a test could supply fixed values
//! without mutating this one.

use std::{
    env,
    ffi::{OsStr, OsString},
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{Context, Result, bail, ensure};
use mockable::{DefaultEnv, Env};
use test_support::fs as test_fs;

use crate::documentation_examples::DocumentedExample;

pub mod wrapper;

/// Variables the wrapper inherits unchanged from the test process.
///
/// The list is closed rather than a filter over the ambient environment, so a
/// variable this test has never considered cannot decide what the example
/// does. Every entry is one Windows or .NET reads while starting a process or
/// resolving a per-user directory; none of them can name a Netsuke install.
const INHERITED_VARIABLES: [&str; 15] = [
    "SystemRoot",
    "windir",
    "SystemDrive",
    "COMSPEC",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_ARCHITEW6432",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "LOCALAPPDATA",
    "APPDATA",
    "ProgramData",
];

/// The parent environment the child environment is derived from.
///
/// A test drives the documented fence, so it has to observe the environment
/// the test process itself was given before it can decide what the child may
/// see. Reading those values once here, through the injected [`Env`], keeps
/// the process environment out of the individual helpers that consume it and
/// lets a test supply fixed values instead of mutating the process.
struct AmbientEnvironment {
    /// The parent `PATH`, used only to locate a PowerShell executable.
    path: Option<OsString>,
    /// `SystemRoot`, which locates both Windows PowerShell and the system
    /// directories the child's `PATH` names.
    system_root: Option<OsString>,
    /// Every variable from [`INHERITED_VARIABLES`] the parent defines.
    inherited: Vec<(OsString, OsString)>,
}

/// Read the parent environment once, at the test boundary.
///
/// [`Env::os_string`] is used rather than a string accessor because a real
/// `PATH` is not guaranteed to be valid Unicode; dropping such a value would
/// silently reduce the shells this harness can find.
fn ambient_environment(env: &dyn Env) -> AmbientEnvironment {
    AmbientEnvironment {
        path: env.os_string("PATH"),
        system_root: env.os_string("SystemRoot"),
        inherited: INHERITED_VARIABLES
            .iter()
            .copied()
            .filter_map(|name| {
                env.os_string(name)
                    .map(|value| (OsString::from(name), value))
            })
            .collect(),
    }
}

/// Write `example` beside the wrapper in `root` and run it.
///
/// # Errors
///
/// Returns an error when the example is not PowerShell, when a script cannot be
/// written, or when no PowerShell executable can be started.
pub fn run_example(root: &Path, example: &DocumentedExample) -> Result<Output> {
    run_script(root, example, None)
}

/// Run `example` after seeding the state an interrupted earlier run leaves.
///
/// A run killed between appending its directory and its `finally` block leaves
/// the sentinel *and* that directory in the stored `PATH`, with the backup
/// still recording the real value. Seeding that pair inside the wrapper, under
/// its mutex, lets the recovery path be observed without a second process
/// racing the first, and the seeded backup records the real value, so the
/// probe leaves the user's `PATH` exactly as it found it.
///
/// # Errors
///
/// Returns the same errors as [`run_example`].
pub fn run_example_after_interrupted_append(
    root: &Path,
    example: &DocumentedExample,
    appended: &Path,
) -> Result<Output> {
    run_script(root, example, Some(appended))
}

/// Write `example` and the wrapper into `root`, then run one wrapper process.
fn run_script(
    root: &Path,
    example: &DocumentedExample,
    interrupted: Option<&Path>,
) -> Result<Output> {
    ensure!(
        example.language == "powershell",
        "{} should be a PowerShell example, got {:?}",
        example.id,
        example.language
    );
    let example_script = root.join("example.ps1");
    test_fs::write(&example_script, &example.body)
        .with_context(|| format!("write {} to {}", example.id, example_script.display()))?;
    let wrapper = root.join("wrapper.ps1");
    test_fs::write(&wrapper, wrapper::WRAPPER).context("write the registry wrapper")?;
    run_wrapper(&wrapper, &example_script, root, interrupted)
}

/// Run the wrapper under the first PowerShell the host provides.
///
/// `-ExecutionPolicy Bypass` is scoped to this child process. Windows
/// PowerShell defaults to a restricted policy on client installations, which
/// would refuse a script this test has just written, and the policy is a
/// property of the shell rather than of the fence under test.
///
/// # Errors
///
/// Returns an error when the environment cannot be assembled or when every
/// candidate shell is missing or cannot be started.
fn run_wrapper(
    wrapper_script: &Path,
    example: &Path,
    program_files_root: &Path,
    interrupted: Option<&Path>,
) -> Result<Output> {
    let backup = backup_path();
    let ambient = ambient_environment(&DefaultEnv);
    let environment = controlled_environment(&ambient, program_files_root)?;
    let mut unavailable = Vec::new();
    for shell in shell_candidates(&ambient) {
        let mut command = Command::new(&shell);
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(wrapper_script)
            .arg("-ExamplePath")
            .arg(example)
            .arg("-BackupPath")
            .arg(&backup);
        if let Some(appended) = interrupted {
            command.arg("-InterruptedAppended").arg(appended);
        }
        let attempt = command
            .env_clear()
            .envs(environment.iter().map(|(name, value)| (name, value)))
            .output();
        match attempt {
            Ok(captured) => return Ok(captured),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                unavailable.push(format!("{}: {error}", shell.display()));
            }
            Err(error) => {
                return Err(error).with_context(|| format!("run {}", shell.display()));
            }
        }
    }
    bail!(
        "no PowerShell executable could be started; {}",
        if unavailable.is_empty() {
            "none was found".to_owned()
        } else {
            unavailable.join("; ")
        }
    )
}

/// The file that records the value to restore when a run is killed outright.
///
/// It lives in the system temporary directory rather than the run's own
/// workspace, because the next run has to find it after the workspace of the
/// run that wrote it has been deleted.
fn backup_path() -> PathBuf {
    env::temp_dir().join("netsuke-windows-path-setup-backup.json")
}

/// Return the PowerShell executables this test can drive, most preferred
/// first.
///
/// The paths are absolute because the wrapper runs with a cleared
/// environment: a bare name would be resolved through a `PATH` the child never
/// sees. The documented shell is PowerShell 7, but Windows PowerShell ships
/// with every Windows image and drives the same registry APIs, so a host
/// without `pwsh` still exercises the script.
fn shell_candidates(ambient: &AmbientEnvironment) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let path = ambient.path.as_deref();
    candidates.extend(path.and_then(|value| find_on_path(value, "pwsh.exe")));
    if let Some(system_root) = ambient.system_root.as_deref() {
        candidates.push(
            PathBuf::from(system_root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe"),
        );
    }
    candidates.extend(path.and_then(|value| find_on_path(value, "powershell.exe")));
    candidates
}

/// Return the first `executable` on `path`, if there is one.
fn find_on_path(path: &OsStr, executable: &str) -> Option<PathBuf> {
    env::split_paths(path)
        .filter(|directory| !directory.as_os_str().is_empty())
        .map(|directory| directory.join(executable))
        .find(|candidate| candidate.is_file())
}

/// Return the environment the wrapper runs with.
///
/// The child inherits nothing from the test process except the variables
/// Windows needs to start it, and `PATH` names only system directories: a real
/// Netsuke installation elsewhere on the host can therefore never satisfy the
/// example's `Get-Command` check, and the staged directory is the only place
/// the run can find an executable. The three program files variables point at
/// the temporary tree so neither script can resolve the host's real
/// directories.
///
/// # Errors
///
/// Returns an error when `SystemRoot` is unset, because both the system `PATH`
/// and the Windows PowerShell location are derived from it.
fn controlled_environment(
    ambient: &AmbientEnvironment,
    program_files_root: &Path,
) -> Result<Vec<(OsString, OsString)>> {
    let system_root = ambient
        .system_root
        .as_deref()
        .context("SystemRoot should be set so the wrapper's system PATH can be built")?;
    let mut environment = ambient.inherited.clone();
    let system32 = PathBuf::from(system_root).join("System32");
    let path = env::join_paths([
        system32.clone(),
        PathBuf::from(system_root),
        system32.join("Wbem"),
        system32.join("WindowsPowerShell").join("v1.0"),
    ])
    .context("join the wrapper's system PATH")?;
    environment.push((OsString::from("PATH"), path));
    environment.push((
        OsString::from("PATHEXT"),
        OsString::from(".COM;.EXE;.BAT;.CMD"),
    ));
    for name in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        environment.push((
            OsString::from(name),
            program_files_root.as_os_str().to_owned(),
        ));
    }
    Ok(environment)
}
