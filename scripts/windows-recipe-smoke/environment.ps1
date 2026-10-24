<#
.SYNOPSIS
Captures and restores process environment variables for the Windows recipe smoke.

.DESCRIPTION
Keeps environment cleanup state in one helper module so the smoke orchestrator
can focus on arranging the isolated workspace and running its scenarios.
#>
function Get-WindowsRecipeEnvironmentState {
    $names = @(
        'NETSUKE_WINDOWS_SHELL'
        'NETSUKE_SMOKE_OPTION'
        'NETSUKE_SMOKE_VALUE'
        'NETSUKE_ISOLATION_ROOT'
        'NETSUKE_ISOLATED_ENV'
        'NETSUKE_ISOLATED_LARGE'
        'NETSUKE_BASH_BASELINE'
        'NETSUKE_BASH_ENV'
        'NETSUKE_BASH_VARIABLE'
        'NETSUKE_NINJA'
        'PATH'
    )

    foreach ($name in $names) {
        Get-EnvironmentVariableState -Name $name
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
