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
        [string]$Netsuke,

        [Parameter(Mandatory)]
        [string[]]$Arguments
    )

    & $Netsuke @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Netsuke failed for '$($Arguments -join ' ')' with exit code $LASTEXITCODE."
    }
}

function Get-EnvironmentVariableState {
    param(
        [Parameter(Mandatory)]
        [string]$Name
    )

    $value = [System.Environment]::GetEnvironmentVariable($Name, 'Process')
    [pscustomobject]@{
        Name = $Name
        Exists = $null -ne $value
        Value = $value
    }
}

function Restore-EnvironmentVariableState {
    param(
        [Parameter(Mandatory)]
        [pscustomobject]$State
    )

    if ($State.Exists) {
        Set-Item -LiteralPath "Env:$($State.Name)" -Value $State.Value
        return
    }

    Remove-Item -LiteralPath "Env:$($State.Name)" -ErrorAction SilentlyContinue
}

function Test-PowerShellCoreHost {
    if ($PSVersionTable.PSEdition -ne 'Core') {
        throw 'This smoke test must be launched by PowerShell Core (pwsh), not Windows PowerShell.'
    }
}

function Test-TargetDiscovery {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

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
}

function Test-ExpectedRecipeFailure {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke,

        [Parameter(Mandatory)]
        [string]$Target,

        [Parameter(Mandatory)]
        [string]$NotCreatedPath,

        [Parameter(Mandatory)]
        [string]$ExitCodeMessage,

        [Parameter(Mandatory)]
        [string]$UnexpectedExecutionMessage
    )

    $failure = & $Netsuke build $Target 2>&1
    if ($LASTEXITCODE -ne 1) {
        throw "$ExitCodeMessage, got ${LASTEXITCODE}: $failure"
    }
    if (Test-Path -LiteralPath $NotCreatedPath) {
        throw "$UnexpectedExecutionMessage: $failure"
    }
}

function Test-InterpreterSelection {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'scalar')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'scalar interpreter with spaces.txt') `
        -Expected 'Desktop' -Message 'A scalar recipe did not run in Windows PowerShell'

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'script')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'script interpreter.txt') -Expected 'Desktop' `
        -Message 'A script recipe did not run in Windows PowerShell'
}

function Test-DollarSyntax {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'dollar-syntax')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'dollar value.txt') -Expected 'default value' `
        -Message 'Ninja did not preserve ordinary PowerShell dollar syntax'
}

function Test-OrderedCommandList {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'ordered-list')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'ordered state.txt') -Expected 'first;second' `
        -Message 'The ordered command list did not preserve state and order'

    Test-ExpectedRecipeFailure -Netsuke $Netsuke -Target 'first-list-entry-fails' `
        -NotCreatedPath 'must not exist.txt' `
        -ExitCodeMessage "A recipe exit code of 27 should become Netsuke's documented failure exit code 1" `
        -UnexpectedExecutionMessage 'The second command-list entry ran after the first failed'
}

function Test-DependencyOrdering {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'aggregate')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'dependency order.txt') -Expected 'first;second;aggregate' `
        -Message 'The aggregate action did not observe serial dependency order'
}

function Test-ResponseFileCleanup {
    if (Get-ChildItem -LiteralPath . -Filter '*.netsuke-*.rsp' -ErrorAction SilentlyContinue) {
        throw 'Ninja did not clean the PowerShell response files after executing the recipes.'
    }
}

function Test-LargeRecipeTransport {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'large-scalar')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'large scalar.txt') -Expected 'scalar' `
        -Message 'A large scalar recipe did not use the alternate PowerShell transport'

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'large-script')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'large script.txt') -Expected 'script' `
        -Message 'A large script recipe did not use the alternate PowerShell transport'

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'large-list')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'large list.txt') -Expected 'first' `
        -Message 'A large ordered recipe did not preserve PowerShell state'

    Test-ExpectedRecipeFailure -Netsuke $Netsuke -Target 'large-list-fails' `
        -NotCreatedPath 'large list must not exist.txt' `
        -ExitCodeMessage "A large recipe exit code of 28 should become Netsuke's documented failure exit code 1" `
        -UnexpectedExecutionMessage 'The large ordered recipe continued after its first failed entry'
    Test-ResponseFileCleanup
}

function Test-AutomaticPathQuoting {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'automatic path.txt')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'automatic path.txt') `
        -Expected 'automatic-path-quoting' -Message 'The discovered target did not build'
}

function Test-MissingBashRuntimeDiagnostic {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke,

        [Parameter(Mandatory)]
        [string]$Workspace
    )

    $env:NETSUKE_NINJA = (Get-Command ninja -CommandType Application).Path
    $env:NETSUKE_WINDOWS_SHELL = 'bash'
    $env:PATH = $Workspace
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

function Invoke-WindowsRecipeSmoke {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke,

        [Parameter(Mandatory)]
        [string]$Manifest
    )

    $ErrorActionPreference = 'Stop'
    $Netsuke = (Resolve-Path -LiteralPath $Netsuke).Path
    $Manifest = (Resolve-Path -LiteralPath $Manifest).Path
    Test-PowerShellCoreHost

    $workspace = Join-Path $env:RUNNER_TEMP "netsuke-windows-recipe-smoke-$PID"
    $environmentState = @(
        Get-EnvironmentVariableState -Name 'NETSUKE_WINDOWS_SHELL'
        Get-EnvironmentVariableState -Name 'NETSUKE_SMOKE_OPTION'
        Get-EnvironmentVariableState -Name 'NETSUKE_SMOKE_VALUE'
        Get-EnvironmentVariableState -Name 'NETSUKE_NINJA'
        Get-EnvironmentVariableState -Name 'PATH'
    )

    try {
        New-Item -ItemType Directory -Path $workspace | Out-Null
        Copy-Item -LiteralPath $Manifest -Destination (Join-Path $workspace 'Netsukefile')

        Push-Location $workspace
        try {
            Remove-Item Env:NETSUKE_WINDOWS_SHELL -ErrorAction SilentlyContinue
            Remove-Item Env:NETSUKE_SMOKE_OPTION -ErrorAction SilentlyContinue
            $env:NETSUKE_SMOKE_VALUE = 'value with spaces'

            Test-TargetDiscovery -Netsuke $Netsuke
            Test-InterpreterSelection -Netsuke $Netsuke
            Test-DollarSyntax -Netsuke $Netsuke
            Test-OrderedCommandList -Netsuke $Netsuke
            Test-DependencyOrdering -Netsuke $Netsuke
            Test-LargeRecipeTransport -Netsuke $Netsuke
            Test-AutomaticPathQuoting -Netsuke $Netsuke
            Test-MissingBashRuntimeDiagnostic -Netsuke $Netsuke -Workspace $workspace
        }
        finally {
            Pop-Location
        }
    }
    finally {
        foreach ($state in $environmentState) {
            Restore-EnvironmentVariableState -State $state
        }
        Remove-Item -Recurse -Force -LiteralPath $workspace -ErrorAction SilentlyContinue
    }
}

Invoke-WindowsRecipeSmoke -Netsuke $Netsuke -Manifest $Manifest
