//! The PowerShell wrapper that surrounds a documented Windows `PATH` fence.
//!
//! The wrapper owns the user environment registry key for the duration of one
//! run: it records the existing value, installs a sentinel, dot-sources the
//! documented script, reports what was stored and its type, and restores the
//! original value whatever the script does. It also repairs the state a killed
//! run leaves, and can seed that state for the recovery test.
//!
//! The text lives in its own module because it is long enough that keeping it
//! beside the Rust harness would push both past the repository's file-size cap.

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
///
/// `-InterruptedAppended` seeds that killed state for the recovery test. The
/// seeding happens under the mutex, and the backup it writes records the real
/// value, so a seeded run leaves the user's `PATH` exactly as it found it.
pub const WRAPPER: &str = r#"
param(
    [Parameter(Mandatory)][string]$ExamplePath,
    [Parameter(Mandatory)][string]$BackupPath,
    [string]$InterruptedAppended = ''
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
    # The recovery test has to start from the state a killed run leaves, so it
    # is seeded here, after the mutex is held and before recovery reads the
    # file. Recording the value that is really there means the seeded run
    # restores the user's own PATH rather than a stand-in for it.
    if ($InterruptedAppended) {
        $wrapperSeededHadPath = $wrapperKey.GetValueNames() -contains 'Path'
        $wrapperSeededValue = if ($wrapperSeededHadPath) {
            [string]$wrapperKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
        } else {
            ''
        }
        $wrapperSeededKind = if ($wrapperSeededHadPath) {
            [string]$wrapperKey.GetValueKind('Path')
        } else {
            'ExpandString'
        }
        $wrapperSeeded = [ordered]@{
            HadPath = $wrapperSeededHadPath
            Value = $wrapperSeededValue
            Kind = $wrapperSeededKind
            Sentinel = $wrapperSentinel
        } | ConvertTo-Json -Compress
        [System.IO.File]::WriteAllText($BackupPath, $wrapperSeeded)
        $wrapperKey.SetValue(
            'Path',
            "$wrapperSentinel;$InterruptedAppended",
            [Microsoft.Win32.RegistryValueKind]::ExpandString
        )
        Write-Output "SEEDED=$wrapperSeededValue"
    }
    # A run killed before its finally block leaves a test-owned value behind:
    # either the bare sentinel, or the sentinel followed by the directory its
    # example appended. Both are this harness's own state and both are restored
    # from the backup. Any other value was written by something else, so the
    # backup is kept for an operator and the run stops rather than capturing
    # that value as the user's own.
    if ([System.IO.File]::Exists($BackupPath)) {
        $wrapperBackup = [System.IO.File]::ReadAllText($BackupPath) | ConvertFrom-Json
        $wrapperCurrent = if ($wrapperKey.GetValueNames() -contains 'Path') {
            [string]$wrapperKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
        } else {
            ''
        }
        $wrapperSentinelPrefix = [string]$wrapperBackup.Sentinel + ';'
        $wrapperIsTestOwned = $wrapperCurrent -eq [string]$wrapperBackup.Sentinel -or
            $wrapperCurrent.StartsWith(
                $wrapperSentinelPrefix,
                [System.StringComparison]::OrdinalIgnoreCase
            )
        if (-not $wrapperIsTestOwned) {
            throw "The value recorded in $BackupPath cannot be restored safely: the stored PATH is neither $wrapperSentinelPrefix nor the sentinel. Leaving the backup in place."
        }
        if ($wrapperBackup.HadPath) {
            $wrapperKey.SetValue(
                'Path',
                [string]$wrapperBackup.Value,
                [Microsoft.Win32.RegistryValueKind]$wrapperBackup.Kind
            )
        } else {
            $wrapperKey.DeleteValue('Path', $false)
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
