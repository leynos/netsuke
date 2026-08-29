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
    Remove-Item Env:NETSUKE_WINDOWS_SHELL -ErrorAction SilentlyContinue
    Remove-Item Env:NETSUKE_SMOKE_OPTION -ErrorAction SilentlyContinue
    $env:NETSUKE_SMOKE_VALUE = 'value with spaces'

    $discovery = & $Netsuke help targets 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Target discovery failed with exit code ${LASTEXITCODE}: $discovery"
    }
    if (-not ($discovery -match 'Confirm target discovery has no recipe side effects')) {
        throw "Target discovery omitted the fixture target: $discovery"
    }
    if (Test-Path -LiteralPath 'discovery-must-not-execute.txt') {
        throw 'Target discovery executed a recipe.'
    }

    Invoke-Netsuke -Arguments @('build', 'scalar')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'scalar interpreter with spaces.txt') `
        -Expected 'Desktop' -Message 'A scalar recipe did not run in Windows PowerShell'

    Invoke-Netsuke -Arguments @('build', 'dollar-syntax')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'dollar value.txt') -Expected 'default value' `
        -Message 'Ninja did not preserve ordinary PowerShell dollar syntax'

    Invoke-Netsuke -Arguments @('build', 'ordered-list')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'ordered state.txt') -Expected 'first;second' `
        -Message 'The ordered command list did not preserve state and order'

    $failure = & $Netsuke build first-list-entry-fails 2>&1
    if ($LASTEXITCODE -ne 1) {
        throw "A recipe exit code of 27 should become Netsuke's documented failure exit code 1, got ${LASTEXITCODE}: $failure"
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

    Invoke-Netsuke -Arguments @('build', 'automatic-path.txt')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'automatic-path.txt') `
        -Expected 'automatic-path-quoting' -Message 'The discovered target did not build'

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
        if (-not ($bashFailure -match 'bash\.exe.*(not found on PATH|exited with)')) {
            throw "Bash runtime diagnostics were not actionable: $bashFailure"
        }
        # The expected child-process failure must not become the smoke script's exit status.
        $global:LASTEXITCODE = 0
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
