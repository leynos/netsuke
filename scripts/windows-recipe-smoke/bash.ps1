<#
.SYNOPSIS
Exercises explicit Windows Bash selection in the native recipe smoke suite.

.DESCRIPTION
Keeps Bash-specific executable identity and missing-runtime cases separate
from the PowerShell transport cases in the parent smoke script.
#>
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
        $bashPath = $null
        foreach ($pathEntry in ($env:PATH -split [regex]::Escape([string][IO.Path]::PathSeparator))) {
            if ([string]::IsNullOrWhiteSpace($pathEntry)) {
                continue
            }
            $candidate = Join-Path -Path $pathEntry -ChildPath 'bash.exe'
            if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                $bashPath = (Resolve-Path -LiteralPath $candidate).Path
                break
            }
        }
        if (-not $bashPath) {
            throw 'The Windows smoke environment must provide bash.exe on PATH for the explicit Bash route.'
        }
        Write-Host "Windows smoke Bash path from PATH: $bashPath"
        $bashVersion = & $bashPath --version 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "The selected Bash executable failed '--version': $bashPath ($LASTEXITCODE): $bashVersion"
        }
        Remove-Item Env:NETSUKE_BASH_ENV -ErrorAction SilentlyContinue
        $bashBaseline = & $bashPath -c 'pwd' 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "Could not determine the Bash working-directory baseline using ${bashPath}: $bashBaseline"
        }
        $env:NETSUKE_WINDOWS_SHELL = 'bash'
        $env:NETSUKE_BASH_BASELINE = $bashBaseline.Trim()
        Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'bash-isolated-state')
        Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'bash-isolated-state.txt') `
            -Expected 'isolated' -Message 'Windows Bash did not isolate command-list shell state'
    }
    finally {
        foreach ($state in $environmentState) {
            Restore-EnvironmentVariableState -State $state
        }
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
        $env:NETSUKE_NINJA = (Get-Command ninja -CommandType Application).Path
        $env:NETSUKE_WINDOWS_SHELL = 'bash'
        $env:PATH = $Workspace
        $targets = & $Netsuke help targets 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "Target discovery should not require Bash on PATH: $targets"
        }
        if (-not ($targets -match 'bash-isolated-state')) {
            throw "Target discovery omitted the Bash fixture target: $targets"
        }

        $generated = & $Netsuke generate 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "Generate should not require Bash on PATH: $generated"
        }
        if (-not ($generated -match 'bash\.exe')) {
            throw "Generate did not leave Bash executable binding to PATH: $generated"
        }

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
    finally {
        foreach ($state in $environmentState) {
            Restore-EnvironmentVariableState -State $state
        }
        # The expected child-process failure must not become the smoke script's exit status.
        $global:LASTEXITCODE = 0
    }
}
