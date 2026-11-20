//! The PowerShell harness that executes a documented Windows `PATH` fence.
//!
//! A fence is run inside a wrapper that owns the user environment registry
//! key: it records the existing `Path` value, installs a sentinel, dot-sources
//! the documented script, reports what was stored and its type, and restores
//! the original value whatever the script does. The child PowerShell gets a
//! cleared environment with only the variables Windows needs to start it, so
//! nothing the host happens to export can decide which directory the example
//! resolves.
//!
//! The harness lives in a directory module because the wrapper is long enough
//! that keeping it beside the assertions would push the test binary past the
//! repository's file-size cap.

use std::{
    env,
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{Context, Result, bail, ensure};
use test_support::fs as test_fs;

use crate::documentation_examples::DocumentedExample;

/// Sentinel user `PATH` value every run starts from.
///
/// It is an expandable string, so the example's promise to preserve the value's
/// type is observable: a script that rewrote `PATH` as a plain string would
/// fail the assertion in the test binary even though the directory was added.
pub const SENTINEL_PATH: &str = "%SystemRoot%\\netsuke-path-sentinel";

/// The wrapper that surrounds the documented script.
///
/// It passes `-ExamplePath` to the script, owns the registry key for the
/// duration of the run, and reports the observations the assertions use.
///
/// Its variables carry a `$wrapper` prefix because the documented script is
/// dot-sourced into this scope so that its failures propagate unchanged. A
/// script that declares `$environmentKey` — both documented fences do — would
/// otherwise replace the wrapper's handle, and the `Dispose()` call at the end
/// of the script would close the key the wrapper still needs to read.
///
/// Two runs must not interleave. Each run replaces the user `PATH` with
/// `SENTINEL_PATH`, a fixed value, so a run that started while another was in
/// flight would capture the sentinel as its original and could put it back
/// over the real value. A named mutex serialises them; the backup file, which
/// is written before the sentinel and removed after the restore, lets a later
/// run put the real value back when a wrapper is killed before its `finally`
/// block runs. The file is named by the caller because the recovery has to
/// outlive the temp directory of the run that wrote it.
pub const WRAPPER: &str = r#"
param(
    [Parameter(Mandatory)][string]$ExamplePath,
    [Parameter(Mandatory)][string]$BackupPath
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$wrapperSentinel = '%SystemRoot%\netsuke-path-sentinel'
$wrapperMutex = [System.Threading.Mutex]::new($false, 'Local\NetsukeWindowsPathSetupTest')
# Both are read by the outer finally, so neither may still be unset when the
# first statement inside the try throws: strict mode would report the missing
# variable instead of the failure that got there.
$wrapperMutexHeld = $false
$wrapperKey = $null
try {
    # An abandoned mutex means its owner was killed, which is the case the
    # backup file exists for; WaitOne throws but acquires the mutex anyway.
    $wrapperMutexHeld = $true
    try {
        $wrapperMutexHeld = $wrapperMutex.WaitOne([TimeSpan]::FromMinutes(10))
    }
    catch [System.Threading.AbandonedMutexException] {
        $wrapperMutexHeld = $true
    }
    if (-not $wrapperMutexHeld) {
        throw 'Timed out waiting for another documented PATH run to finish.'
    }
    $wrapperKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
    if ($null -eq $wrapperKey) {
        throw 'User environment registry key not found.'
    }
    # A run killed before its finally block leaves the sentinel behind. Repair
    # that before capturing anything, so this run starts from the real value
    # rather than adopting the sentinel as its own.
    if ([System.IO.File]::Exists($BackupPath)) {
        $wrapperBackup = [System.IO.File]::ReadAllText($BackupPath) | ConvertFrom-Json
        $wrapperCurrent = if ($wrapperKey.GetValueNames() -contains 'Path') {
            [string]$wrapperKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
        } else {
            ''
        }
        if ($wrapperCurrent -eq [string]$wrapperBackup.Sentinel) {
            if ($wrapperBackup.HadPath) {
                $wrapperKey.SetValue(
                    'Path',
                    [string]$wrapperBackup.Value,
                    [Microsoft.Win32.RegistryValueKind]$wrapperBackup.Kind
                )
            } else {
                $wrapperKey.DeleteValue('Path', $false)
            }
        }
        Remove-Item -LiteralPath $BackupPath -Force
    }
    $wrapperHadPath = $wrapperKey.GetValueNames() -contains 'Path'
    $wrapperOriginalValue = if ($wrapperHadPath) {
        [string]$wrapperKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
    } else {
        ''
    }
    $wrapperOriginalKind = if ($wrapperHadPath) {
        $wrapperKey.GetValueKind('Path')
    } else {
        [Microsoft.Win32.RegistryValueKind]::ExpandString
    }
    Write-Output "ORIGINAL=$wrapperOriginalValue"
    $wrapperBackupJson = [ordered]@{
        HadPath = $wrapperHadPath
        Value = $wrapperOriginalValue
        Kind = [string]$wrapperOriginalKind
        Sentinel = $wrapperSentinel
    } | ConvertTo-Json -Compress
    [System.IO.File]::WriteAllText($BackupPath, $wrapperBackupJson)
    try {
        $wrapperKey.SetValue(
            'Path',
            $wrapperSentinel,
            [Microsoft.Win32.RegistryValueKind]::ExpandString
        )
        . $ExamplePath
        $wrapperStored = [string]$wrapperKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
        Write-Output "RESULT=$wrapperStored"
        Write-Output "KIND=$($wrapperKey.GetValueKind('Path'))"
    }
    finally {
        # Record what the script left behind before restoring it. A run that
        # threw never reaches the RESULT line, so LEFTOVER is the only evidence
        # of whether it changed the value on the way out.
        $wrapperLeftover = if ($wrapperKey.GetValueNames() -contains 'Path') {
            [string]$wrapperKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
        } else {
            ''
        }
        Write-Output "LEFTOVER=$wrapperLeftover"
        if ($wrapperHadPath) {
            $wrapperKey.SetValue('Path', $wrapperOriginalValue, $wrapperOriginalKind)
        } else {
            $wrapperKey.DeleteValue('Path', $false)
        }
        $wrapperRestored = if ($wrapperKey.GetValueNames() -contains 'Path') {
            [string]$wrapperKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
        } else {
            ''
        }
        Write-Output "RESTORED=$wrapperRestored"
        Remove-Item -LiteralPath $BackupPath -Force -ErrorAction SilentlyContinue
    }
}
finally {
    if ($null -ne $wrapperKey) {
        $wrapperKey.Dispose()
    }
    if ($wrapperMutexHeld) {
        $wrapperMutex.ReleaseMutex()
    }
    $wrapperMutex.Dispose()
}
"#;

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

/// Write `example` beside the wrapper in `root` and run it.
///
/// # Errors
///
/// Returns an error when the example is not PowerShell, when a script cannot be
/// written, or when no PowerShell executable can be started.
pub fn run_example(root: &Path, example: &DocumentedExample) -> Result<Output> {
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
    test_fs::write(&wrapper, WRAPPER).context("write the registry wrapper")?;
    run_wrapper(&wrapper, &example_script, root)
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
fn run_wrapper(wrapper: &Path, example: &Path, program_files_root: &Path) -> Result<Output> {
    let backup = backup_path();
    let environment = controlled_environment(program_files_root)?;
    let mut unavailable = Vec::new();
    for shell in shell_candidates() {
        let attempt = Command::new(&shell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(wrapper)
            .arg("-ExamplePath")
            .arg(example)
            .arg("-BackupPath")
            .arg(&backup)
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
fn shell_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.extend(find_on_path("pwsh.exe"));
    if let Some(system_root) = env::var_os("SystemRoot") {
        candidates.push(
            PathBuf::from(system_root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe"),
        );
    }
    candidates.extend(find_on_path("powershell.exe"));
    candidates
}

/// Return the first `executable` on the ambient `PATH`, if there is one.
fn find_on_path(executable: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?)
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
fn controlled_environment(program_files_root: &Path) -> Result<Vec<(OsString, OsString)>> {
    let system_root = env::var_os("SystemRoot")
        .context("SystemRoot should be set so the wrapper's system PATH can be built")?;
    let mut environment: Vec<(OsString, OsString)> = INHERITED_VARIABLES
        .iter()
        .filter_map(|name| env::var_os(name).map(|value| (OsString::from(name), value)))
        .collect();
    let system32 = PathBuf::from(&system_root).join("System32");
    let path = env::join_paths([
        system32.clone(),
        PathBuf::from(&system_root),
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
