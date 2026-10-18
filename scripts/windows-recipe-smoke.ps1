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

$smokeScriptDirectory = Join-Path $PSScriptRoot 'windows-recipe-smoke'
. (Join-Path $smokeScriptDirectory 'bash.ps1')

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
        [pscustomobject]$Failure
    )

    $buildFailure = & $Netsuke build $Failure.Target 2>&1
    if ($LASTEXITCODE -ne 1) {
        throw "$($Failure.ExitCodeMessage), got ${LASTEXITCODE}: $buildFailure"
    }
    if (Test-Path -LiteralPath $Failure.NotCreatedPath) {
        throw "$($Failure.UnexpectedExecutionMessage): $buildFailure"
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

    $expectedFailure = [pscustomobject]@{
        Target = 'first-list-entry-fails'
        NotCreatedPath = 'must not exist.txt'
        ExitCodeMessage = "A recipe exit code of 27 should become Netsuke's documented failure exit code 1"
        UnexpectedExecutionMessage = 'The second command-list entry ran after the first failed'
    }
    Test-ExpectedRecipeFailure -Netsuke $Netsuke -Failure $expectedFailure
}

function Test-IsolatedPowerShellLists {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'isolated-state')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'isolated state.txt') `
        -Expected 'baseline' -Message 'An isolated PowerShell entry inherited location or variables'

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'isolated-exit-zero')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'isolated exit zero.txt') `
        -Expected 'continued' -Message 'A successful isolated exit stopped later entries'

    $expectedFailure = [pscustomobject]@{
        Target = 'isolated-list-fails'
        NotCreatedPath = 'isolated must not exist.txt'
        ExitCodeMessage = "An isolated recipe exit code of 29 should become Netsuke's documented failure exit code 1"
        UnexpectedExecutionMessage = 'An isolated entry ran after the native command failed'
    }
    Test-ExpectedRecipeFailure -Netsuke $Netsuke -Failure $expectedFailure

    $expectedPowerShellFailure = [pscustomobject]@{
        Target = 'isolated-powershell-error'
        NotCreatedPath = 'isolated powershell error.txt'
        ExitCodeMessage = "A terminating PowerShell cmdlet error should become Netsuke's documented failure exit code 1"
        UnexpectedExecutionMessage = 'A command after the isolated PowerShell error ran'
    }
    Test-ExpectedRecipeFailure -Netsuke $Netsuke -Failure $expectedPowerShellFailure

    $nativeError = & $Netsuke build isolated-native-stderr 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "The isolated native-stderr target failed with ${LASTEXITCODE}: $nativeError"
    }
    if (-not ($nativeError -match 'isolated-native-stderr')) {
        throw "The isolated child did not pass native stderr through: $nativeError"
    }
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'isolated stderr.txt') `
        -Expected 'complete' -Message 'The isolated list did not continue after native stderr output'
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
    if (Get-ChildItem -LiteralPath . -Filter '*.netsuke-*.ps1' -ErrorAction SilentlyContinue) {
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

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'isolated-large-list')
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'isolated large list.txt') -Expected 'second' `
        -Message 'A large isolated recipe did not use the response-file transport'

    $expectedFailure = [pscustomobject]@{
        Target = 'large-list-fails'
        NotCreatedPath = 'large list must not exist.txt'
        ExitCodeMessage = "A large recipe exit code of 28 should become Netsuke's documented failure exit code 1"
        UnexpectedExecutionMessage = 'The large ordered recipe continued after its first failed entry'
    }
    Test-ExpectedRecipeFailure -Netsuke $Netsuke -Failure $expectedFailure
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

function Test-PathPlaceholderScript {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'path-copy.txt')
    if (-not (Test-Path -LiteralPath 'path-copy.txt')) {
        throw 'The semicolon-bearing source path did not produce its output file.'
    }
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'path-copy.txt') `
        -Expected 'path-derived-content' -Message 'A semicolon-bearing source path was not copied literally'

    Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'literal-in-script-variable.txt')
    if (-not (Test-Path -LiteralPath 'literal-in-script-variable.txt')) {
        throw 'The PowerShell script variable target did not produce its output file.'
    }
    Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'literal-in-script-variable.txt') `
        -Expected '$in' -Message 'A PowerShell variable inside a script string was not preserved'

    if (Test-Path -LiteralPath 'pwned') {
        throw 'A path-derived PowerShell command created the pwned sentinel.'
    }
}

function Test-QuotedScriptPlaceholderRejection {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke,

        [Parameter(Mandatory)]
        [string]$Manifest,

        [Parameter(Mandatory)]
        [string]$FixtureName,

        [Parameter(Mandatory)]
        [string]$TargetName
    )

    $quotedManifest = Join-Path (Split-Path -Parent $Manifest) $FixtureName
    $tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
    $workspace = Join-Path $tempRoot "netsuke-windows-quoted-script-smoke-$PID"

    try {
        New-Item -ItemType Directory -Path $workspace | Out-Null
        Copy-Item -LiteralPath $quotedManifest -Destination (Join-Path $workspace 'Netsukefile')
        Push-Location $workspace
        try {
            $payloadSource = Join-Path $workspace '; New-Item pwned; Write-Output x'
            New-Item -ItemType File -Path $payloadSource | Out-Null
            $failure = & $Netsuke --json --locale en-GB build $TargetName 2>&1
            if ($LASTEXITCODE -eq 0) {
                throw 'A quoted PowerShell script marker unexpectedly built successfully.'
            }
            $failureText = [string]::Join([Environment]::NewLine, [string[]]@($failure))
            try {
                $diagnostics = $failureText | ConvertFrom-Json -ErrorAction Stop
            }
            catch {
                throw "The quoted script failure did not emit JSON diagnostics: $failureText"
            }
            $causes = @($diagnostics.diagnostics[0].causes | ForEach-Object { [string]$_ })
            $causesText = [string]::Join([Environment]::NewLine, [string[]]$causes)
            if (-not ($causesText -match 'Invalid command interpolation')) {
                throw "The quoted script failure omitted its interpolation diagnostic: $causesText"
            }
            if (-not ($causesText -match 'Write-Output' -and $causesText -match '__NETSUKE_INS_PLACEHOLDER__')) {
                throw "The quoted script failure omitted its original template: $causesText"
            }
            if (Test-Path -LiteralPath 'pwned') {
                throw 'The rejected quoted script executed its path-derived payload.'
            }
        }
        finally {
            Pop-Location
        }
    }
    finally {
        Remove-Item -Recurse -Force -LiteralPath $workspace -ErrorAction SilentlyContinue
    }
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

    $tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
    $workspace = Join-Path $tempRoot "netsuke-windows-recipe-smoke-$PID"
    $environmentState = @(
        Get-EnvironmentVariableState -Name 'NETSUKE_WINDOWS_SHELL'
        Get-EnvironmentVariableState -Name 'NETSUKE_SMOKE_OPTION'
        Get-EnvironmentVariableState -Name 'NETSUKE_SMOKE_VALUE'
        Get-EnvironmentVariableState -Name 'NETSUKE_ISOLATION_ROOT'
        Get-EnvironmentVariableState -Name 'NETSUKE_ISOLATED_ENV'
        Get-EnvironmentVariableState -Name 'NETSUKE_ISOLATED_LARGE'
        Get-EnvironmentVariableState -Name 'NETSUKE_BASH_BASELINE'
        Get-EnvironmentVariableState -Name 'NETSUKE_BASH_ENV'
        Get-EnvironmentVariableState -Name 'NETSUKE_BASH_VARIABLE'
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
            Remove-Item Env:NETSUKE_ISOLATED_ENV -ErrorAction SilentlyContinue
            Remove-Item Env:NETSUKE_ISOLATED_LARGE -ErrorAction SilentlyContinue
            Remove-Item Env:NETSUKE_BASH_VARIABLE -ErrorAction SilentlyContinue
            $env:NETSUKE_SMOKE_VALUE = 'value with spaces'
            $env:NETSUKE_ISOLATION_ROOT = $workspace

            $payloadName = '; New-Item pwned; Write-Output x'
            $payloadSource = Join-Path $workspace $payloadName
            New-Item -ItemType File -Path $payloadSource | Out-Null
            Set-Content -NoNewline -LiteralPath $payloadSource -Value 'path-derived-content'

            Test-TargetDiscovery -Netsuke $Netsuke
            Test-InterpreterSelection -Netsuke $Netsuke
            Test-DollarSyntax -Netsuke $Netsuke
            Test-OrderedCommandList -Netsuke $Netsuke
            Test-IsolatedPowerShellLists -Netsuke $Netsuke
            Test-DependencyOrdering -Netsuke $Netsuke
            Test-LargeRecipeTransport -Netsuke $Netsuke
            Test-AutomaticPathQuoting -Netsuke $Netsuke
            Test-ExplicitBashIsolation -Netsuke $Netsuke
            Test-PathPlaceholderScript -Netsuke $Netsuke
            Test-QuotedScriptPlaceholderRejection -Netsuke $Netsuke -Manifest $Manifest `
                -FixtureName 'windows-recipe-smoke-quoted-script.yml' `
                -TargetName 'quoted-script-single.txt'
            Test-QuotedScriptPlaceholderRejection -Netsuke $Netsuke -Manifest $Manifest `
                -FixtureName 'windows-recipe-smoke-quoted-script-double.yml' `
                -TargetName 'quoted-script-double.txt'
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
