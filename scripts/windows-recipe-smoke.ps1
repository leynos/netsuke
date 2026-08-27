<#
.SYNOPSIS
Exercises the Windows legacy-recipe contract from a native PowerShell session.

.DESCRIPTION
Runs a fixture through a supplied Netsuke executable. The caller is expected to
use PowerShell Core (`pwsh`), while the fixture proves that Ninja starts Windows
PowerShell (`powershell.exe`) for legacy recipes.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Netsuke,

    [Parameter(Mandatory)]
    [string]$Manifest
)

$ErrorActionPreference = 'Stop'
$Netsuke = (Resolve-Path -LiteralPath $Netsuke).Path
$Manifest = (Resolve-Path -LiteralPath $Manifest).Path

function Assert-Equal {
    param(
        [Parameter(Mandatory)]
        [string]$Actual,

        [Parameter(Mandatory)]
        [string]$Expected,

        [Parameter(Mandatory)]
        [string]$Message
    )

    if ($Actual -ne $Expected) {
        throw "$Message. Expected '$Expected', got '$Actual'."
    }
}

function Invoke-Netsuke {
    param(
        [Parameter(Mandatory)]
        [string[]]$Arguments
    )

    & $Netsuke @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Netsuke failed for '$($Arguments -join ' ')' with exit code $LASTEXITCODE."
    }
}

if ($PSVersionTable.PSEdition -ne 'Core') {
    throw 'This smoke test must be launched by PowerShell Core (pwsh), not Windows PowerShell.'
}

$workspace = Join-Path $env:RUNNER_TEMP "netsuke-windows-recipe-smoke-$PID"
New-Item -ItemType Directory -Path $workspace | Out-Null
Copy-Item -LiteralPath $Manifest -Destination (Join-Path $workspace 'Netsukefile')

Push-Location $workspace
try {
    $env:NETSUKE_SMOKE_VALUE = 'value with spaces'

    $discovery = & $Netsuke help targets 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Target discovery failed with exit code $LASTEXITCODE: $discovery"
    }
    if ($discovery -notmatch 'Confirm target discovery has no recipe side effects') {
        throw "Target discovery omitted the fixture target: $discovery"
    }
    if (Test-Path -LiteralPath 'discovery must not execute.txt') {
        throw 'Target discovery executed a recipe.'
    }

    Invoke-Netsuke -Arguments @('build', 'scalar')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'scalar interpreter with spaces.txt') `
        -Expected 'Desktop' -Message 'A scalar recipe did not run in Windows PowerShell'

    Invoke-Netsuke -Arguments @('build', 'ordered-list')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'ordered state.txt') -Expected 'first;second' `
        -Message 'The ordered command list did not preserve state and order'

    $failure = & $Netsuke build first-list-entry-fails 2>&1
    if ($LASTEXITCODE -eq 0) {
        throw 'A failed first command-list entry unexpectedly succeeded through Netsuke and Ninja.'
    }
    if (Test-Path -LiteralPath 'must not exist.txt') {
        throw "The second command-list entry ran after the first failed: $failure"
    }

    Invoke-Netsuke -Arguments @('build', 'script')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'script interpreter.txt') -Expected 'Desktop' `
        -Message 'A script recipe did not run in Windows PowerShell'

    Invoke-Netsuke -Arguments @('build', 'aggregate')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'dependency order.txt') -Expected 'first;second;aggregate' `
        -Message 'The aggregate action did not observe serial dependency order'

    Invoke-Netsuke -Arguments @('build', 'automatic path with spaces.txt')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'automatic path with spaces.txt') `
        -Expected 'automatic-path-quoting' -Message 'Automatic path quoting did not preserve spaces'

    $savedPath = $env:PATH
    $savedNinja = $env:NETSUKE_NINJA
    $savedShell = $env:NETSUKE_WINDOWS_SHELL
    try {
        $env:NETSUKE_NINJA = (Get-Command ninja -CommandType Application).Path
        $env:NETSUKE_WINDOWS_SHELL = 'bash'
        $env:PATH = $workspace
        $bashFailure = & $Netsuke build scalar 2>&1
        if ($LASTEXITCODE -eq 0) {
            throw 'Selecting Bash without bash.exe unexpectedly succeeded.'
        }
        if ($bashFailure -notmatch 'bash.exe.*not found on PATH') {
            throw "Missing-Bash diagnostics were not actionable: $bashFailure"
        }
    }
    finally {
        $env:PATH = $savedPath
        $env:NETSUKE_NINJA = $savedNinja
        $env:NETSUKE_WINDOWS_SHELL = $savedShell
    }
}
finally {
    Pop-Location
    Remove-Item -Recurse -Force -LiteralPath $workspace
}
