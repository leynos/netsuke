<#
.SYNOPSIS
Exercises explicit Windows Bash selection in the native recipe smoke suite.

.DESCRIPTION
Keeps Bash-specific executable identity and missing-runtime cases separate
from the PowerShell transport cases in the parent smoke script.
#>
function Find-BashExecutableOnPath {
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )

    foreach ($pathEntry in ($Path -split [regex]::Escape([string][IO.Path]::PathSeparator))) {
        if ([string]::IsNullOrWhiteSpace($pathEntry)) {
            continue
        }
        $candidate = Join-Path -Path $pathEntry -ChildPath 'bash.exe'
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }
    return $null
}

function Assert-BashExecutableVersion {
    param(
        [Parameter(Mandatory)]
        [string]$BashPath
    )

    $bashVersion = & $BashPath --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "The selected Bash executable failed '--version': $BashPath ($LASTEXITCODE): $bashVersion"
    }
}

function Get-BashWorkingDirectoryBaseline {
    param(
        [Parameter(Mandatory)]
        [string]$BashPath
    )

    $bashBaseline = & $BashPath -c 'pwd' 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Could not determine the Bash working-directory baseline using ${BashPath}: $bashBaseline"
    }
    $bashBaseline.Trim()
}

function Assert-BashIsolationState {
    $bashIsolationState = @(Get-Content -LiteralPath 'bash-isolated-state.txt')
    if ($bashIsolationState.Count -ne 1) {
        throw "Windows Bash isolation state should contain one line, got $($bashIsolationState.Count)."
    }
    Assert-Equal -Actual $bashIsolationState[0] `
        -Expected 'isolated' -Message 'Windows Bash did not isolate command-list shell state'
}

function Test-ExplicitBashIsolation {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    $environmentState = @(
        Get-EnvironmentVariableState -Name 'NETSUKE_WINDOWS_SHELL'
        Get-EnvironmentVariableState -Name 'NETSUKE_BASH_BASELINE'
        Get-EnvironmentVariableState -Name 'NETSUKE_BASH_ENV'
    )
    try {
        $bashPath = Find-BashExecutableOnPath -Path $env:PATH
        if (-not $bashPath) {
            throw 'The Windows smoke environment must provide bash.exe on PATH for the explicit Bash route.'
        }
        Write-Host "Windows smoke Bash path from PATH: $bashPath"
        Assert-BashExecutableVersion -BashPath $bashPath
        Remove-Item Env:NETSUKE_BASH_ENV -ErrorAction SilentlyContinue
        $env:NETSUKE_BASH_BASELINE = Get-BashWorkingDirectoryBaseline -BashPath $bashPath
        $env:NETSUKE_WINDOWS_SHELL = 'bash'
        Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'bash-isolated-state')
        Assert-BashIsolationState
    }
    finally {
        foreach ($state in $environmentState) {
            Restore-EnvironmentVariableState -State $state
        }
    }
}

function Assert-BashTargetDiscoveryWithoutRuntime {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    $targets = & $Netsuke help targets 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Target discovery should not require Bash on PATH: $targets"
    }
    if (-not ($targets -match 'bash-isolated-state')) {
        throw "Target discovery omitted the Bash fixture target: $targets"
    }
}

function Assert-BashGenerationRequiresRuntime {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke,

        [Parameter(Mandatory)]
        [string]$PlanPath
    )

    Remove-Item -LiteralPath $PlanPath -ErrorAction SilentlyContinue
    $generated = & $Netsuke generate --output $PlanPath 2>&1
    if ($LASTEXITCODE -eq 0) {
        throw 'Generate without bash.exe should fail rather than emit an unbound plan.'
    }
    $generatedText = [string]::Join([Environment]::NewLine, [string[]]@($generated))
    if (-not ($generatedText -match 'PATH contains no bash\.exe')) {
        throw "Bash generate diagnostics did not distinguish a missing runtime: $generatedText"
    }
    if (Test-Path -LiteralPath $PlanPath) {
        throw 'Generate without bash.exe emitted an unbound Ninja plan.'
    }
}

function Assert-MissingBashRuntimeDiagnostic {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    $bashFailure = & $Netsuke build scalar 2>&1
    if ($LASTEXITCODE -eq 0) {
        throw 'Selecting Bash without bash.exe unexpectedly succeeded.'
    }
    if (-not ($bashFailure -match 'PATH contains no bash\.exe')) {
        throw "Bash resolution diagnostics did not distinguish a missing runtime: $bashFailure"
    }
    if ($bashFailure -match 'exited with') {
        throw "A missing-runtime diagnostic must not report a failed executable probe: $bashFailure"
    }
}

function Test-MissingBashRuntimeDiagnostic {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke,

        [Parameter(Mandatory)]
        [string]$Workspace
    )

    $environmentState = @(
        Get-EnvironmentVariableState -Name 'NETSUKE_NINJA'
        Get-EnvironmentVariableState -Name 'NETSUKE_WINDOWS_SHELL'
        Get-EnvironmentVariableState -Name 'PATH'
    )
    try {
        # Get-Command may report several ninja applications; NETSUKE_NINJA names
        # exactly one program, so an array would be joined into an invalid path.
        $ninja = Get-Command ninja -CommandType Application | Select-Object -First 1
        if (-not $ninja) {
            throw 'The missing-runtime scenario requires a ninja application on PATH.'
        }
        $env:NETSUKE_NINJA = $ninja.Path
        $env:NETSUKE_WINDOWS_SHELL = 'bash'
        $env:PATH = $Workspace
        Assert-BashTargetDiscoveryWithoutRuntime -Netsuke $Netsuke
        $planPath = Join-Path $Workspace 'bash-generation-must-not-exist.ninja'
        Assert-BashGenerationRequiresRuntime -Netsuke $Netsuke -PlanPath $planPath
        Assert-MissingBashRuntimeDiagnostic -Netsuke $Netsuke
    }
    finally {
        foreach ($state in $environmentState) {
            Restore-EnvironmentVariableState -State $state
        }
        # The expected child-process failure must not become the smoke script's exit status.
        $global:LASTEXITCODE = 0
    }
}
