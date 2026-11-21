param(
    [Parameter(Mandatory)][string]$ExamplePath,
    [Parameter(Mandatory)][string]$BackupPath,
    [switch]$SimulateInterruption,
    [switch]$SimulateSpentBackup
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# The marker lines are the only channel back to the Rust test, and a staged
# path may hold non-ASCII characters. A redirected console otherwise writes the
# console code page, whose bytes are not UTF-8 and would decode to a different
# path. Pinning the writer to UTF-8 without a byte order mark keeps the round
# trip lossless whatever the host code page is.
$wrapperStdout = [System.IO.StreamWriter]::new(
    [Console]::OpenStandardOutput(),
    [System.Text.UTF8Encoding]::new($false)
)
$wrapperStdout.AutoFlush = $true
$wrapperSentinel = '%SystemRoot%\netsuke-path-sentinel'
$wrapperMutex = [System.Threading.Mutex]::new($false, 'Local\NetsukeWindowsPathSetupTest')
# Both are read by the outer finally, so neither may still be unset when the
# first statement inside the try throws: strict mode would report the missing
# variable instead of the failure that got there.
$wrapperMutexHeld = $false
$wrapperKey = $null
# Whether the example changed the key's value is not visible from the value it
# leaves: a change that restores the text that was already there looks exactly
# like no change at all, and the key's last-write time is too coarse to separate
# a change from the sentinel write the wrapper performs immediately before the
# example. `REG_NOTIFY_CHANGE_LAST_SET` reports a change to a value of the key,
# and a write that stores the text already present is not such a change, so the
# report establishes a value change rather than counting writes.
$wrapperWatchType = 'NetsukeRegistryWatch'
if (-not ($wrapperWatchType -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class NetsukeRegistryWatch
{
    private static readonly IntPtr CurrentUser = new IntPtr(unchecked((int)0x80000001));
    private const int NotifyAccess = 0x0010;
    private const int ChangeLastSet = 0x00000004;

    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern int RegOpenKeyEx(
        IntPtr key, string subKey, int options, int rights, out IntPtr opened);

    [DllImport("advapi32.dll")]
    private static extern int RegCloseKey(IntPtr key);

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern int RegNotifyChangeKeyValue(
        IntPtr key, bool watchSubtree, int filter, IntPtr changeEvent, bool asynchronous);

    public static IntPtr OpenUserEnvironment()
    {
        IntPtr opened;
        int status = RegOpenKeyEx(CurrentUser, "Environment", 0, NotifyAccess, out opened);
        if (status != 0) { throw new Win32Exception(status); }
        return opened;
    }

    public static void Watch(IntPtr key, IntPtr changeEvent)
    {
        int status = RegNotifyChangeKeyValue(
            key, false, ChangeLastSet, changeEvent, true);
        if (status != 0) { throw new Win32Exception(status); }
    }

    public static void Close(IntPtr key)
    {
        RegCloseKey(key);
    }
}
'@
}
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
    # A run killed before its finally block leaves a test-owned value behind:
    # either the bare sentinel a run killed before the example appended leaves,
    # or the sentinel followed by the one directory the backup recorded that
    # run would append. Both are restored from the backup. Any other value was
    # written by something else — including an entry another process added to
    # `PATH` after the interruption — so the backup is kept for an operator and
    # the run stops rather than overwriting that change.
    if ([System.IO.File]::Exists($BackupPath)) {
        $wrapperBackup = [System.IO.File]::ReadAllText($BackupPath) | ConvertFrom-Json
        $wrapperHasPath = $wrapperKey.GetValueNames() -contains 'Path'
        $wrapperCurrent = if ($wrapperHasPath) {
            [string]$wrapperKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
        } else {
            ''
        }
        $wrapperCurrentKind = if ($wrapperHasPath) {
            [string]$wrapperKey.GetValueKind('Path')
        } else {
            ''
        }
        # `-eq` compares strings case-insensitively, which is what a Windows
        # path comparison wants; it still rejects any other suffix, extension or
        # reordering of the value, so a concurrent edit cannot be absorbed.
        # A backup written before this property existed records no appended
        # directory; treating it as empty makes the appended value impossible
        # to match, so such a backup is refused rather than guessed at.
        $wrapperBackupAppended = if ($wrapperBackup.PSObject.Properties['Appended']) {
            [string]$wrapperBackup.Appended
        } else {
            ''
        }
        $wrapperSentinelValue = [string]$wrapperBackup.Sentinel
        $wrapperAppendedValue = "$wrapperSentinelValue;$wrapperBackupAppended"
        $wrapperIsTestOwned = $wrapperCurrent -eq $wrapperSentinelValue -or
            $wrapperCurrent -eq $wrapperAppendedValue
        # A run that died after restoring but before deleting the backup leaves
        # the registry exactly as the backup records the original. Recognising
        # that state and removing the completed backup is what stops a spent
        # backup from being refused for ever; existence, value and kind are all
        # compared, so an unrelated value is still refused below.
        $wrapperIsAlreadyRestored = if ([bool]$wrapperBackup.HadPath) {
            $wrapperHasPath -and
                $wrapperCurrent -eq [string]$wrapperBackup.Value -and
                $wrapperCurrentKind -eq [string]$wrapperBackup.Kind
        } else {
            -not $wrapperHasPath
        }
        if (-not $wrapperIsTestOwned -and -not $wrapperIsAlreadyRestored) {
            throw "The value recorded in $BackupPath cannot be restored safely: the stored PATH is neither $wrapperSentinelValue nor $wrapperAppendedValue, nor the value the backup records. Leaving the backup in place."
        }
        if ($wrapperIsTestOwned) {
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
    # Both documented fences append exactly this directory and nothing else, so
    # recording it here is what lets a later run recognise the state a killed
    # run leaves without accepting an arbitrary suffix. The harness always
    # exports the program files variables, so a missing one is a broken run
    # rather than a host without the variable.
    $wrapperProgramFiles = $env:ProgramW6432
    if ([string]::IsNullOrWhiteSpace($wrapperProgramFiles)) {
        throw 'ProgramW6432 is not set, so the directory an interrupted run appends cannot be recorded.'
    }
    $wrapperExpectedAppended = Join-Path $wrapperProgramFiles 'netsuke'
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
    $wrapperStdout.WriteLine("ORIGINAL=$wrapperOriginalValue")
    $wrapperBackupJson = [ordered]@{
        HadPath = $wrapperHadPath
        Value = $wrapperOriginalValue
        Kind = [string]$wrapperOriginalKind
        Sentinel = $wrapperSentinel
        Appended = $wrapperExpectedAppended
    } | ConvertTo-Json -Compress
    [System.IO.File]::WriteAllText($BackupPath, $wrapperBackupJson)
    try {
        $wrapperKey.SetValue(
            'Path',
            $wrapperSentinel,
            [Microsoft.Win32.RegistryValueKind]::ExpandString
        )
        if ($SimulateSpentBackup) {
            # A run killed between its restore and the delete of its backup
            # leaves the original value in place with the backup still on disk.
            # Restore here and end the process without removing the file, so
            # the next run meets a backup that already describes the state it
            # finds.
            if ($wrapperHadPath) {
                $wrapperKey.SetValue('Path', $wrapperOriginalValue, $wrapperOriginalKind)
            } else {
                $wrapperKey.DeleteValue('Path', $false)
            }
            $wrapperStdout.WriteLine("SPENT=$wrapperOriginalValue")
            $wrapperStdout.Flush()
            [System.Environment]::Exit(1)
        }
        if ($SimulateInterruption) {
            # Leave exactly what a run killed after the example appended leaves,
            # then end the process without unwinding so the restore never runs.
            # Exit skips the finally blocks, so the mutex is abandoned rather
            # than released — the path the next run's WaitOne catch exists for.
            $wrapperKey.SetValue(
                'Path',
                "$wrapperSentinel;$wrapperExpectedAppended",
                [Microsoft.Win32.RegistryValueKind]::ExpandString
            )
            $wrapperStdout.WriteLine("INTERRUPTED=$wrapperExpectedAppended")
            $wrapperStdout.Flush()
            [System.Environment]::Exit(1)
        }
        # The watch is armed after the sentinel write above and read before the
        # restore below, so the only write it can observe is the example's.
        $wrapperProcessPathBefore = $env:Path
        $wrapperWatchKey = [NetsukeRegistryWatch]::OpenUserEnvironment()
        $wrapperWatchEvent = [System.Threading.ManualResetEvent]::new($false)
        try {
            [NetsukeRegistryWatch]::Watch(
                $wrapperWatchKey,
                $wrapperWatchEvent.SafeWaitHandle.DangerousGetHandle()
            )
            try {
                . $ExamplePath
            }
            finally {
                # A throwing example skips everything after the dot-source, so
                # what the run observed is reported from here. The bounded wait
                # is what a run that wrote nothing spends: no notification can
                # arrive for a key that was never changed.
                $wrapperKeyChanged = $wrapperWatchEvent.WaitOne(
                    [TimeSpan]::FromSeconds(5)
                )
                $wrapperStdout.WriteLine("PROCESS_BEFORE=$wrapperProcessPathBefore")
                $wrapperStdout.WriteLine("PROCESS_AFTER=$env:Path")
                $wrapperStdout.WriteLine("KEYCHANGED=$wrapperKeyChanged")
            }
        }
        finally {
            $wrapperWatchEvent.Dispose()
            [NetsukeRegistryWatch]::Close($wrapperWatchKey)
        }
        $wrapperStored = [string]$wrapperKey.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
        $wrapperStdout.WriteLine("RESULT=$wrapperStored")
        $wrapperStdout.WriteLine("KIND=$($wrapperKey.GetValueKind('Path'))")
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
        $wrapperStdout.WriteLine("LEFTOVER=$wrapperLeftover")
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
        $wrapperStdout.WriteLine("RESTORED=$wrapperRestored")
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
