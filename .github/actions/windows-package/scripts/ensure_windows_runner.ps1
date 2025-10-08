[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

if ($env:RUNNER_OS -ne 'Windows') {
    throw "windows-package action must run on a Windows runner. Current OS: $($env:RUNNER_OS)"
}
