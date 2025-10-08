[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$toolsDir = Join-Path $env:USERPROFILE '.dotnet\tools'
if (-not ($env:PATH -split ';' | Where-Object { $_ -eq $toolsDir })) {
    $env:PATH = "$toolsDir;$env:PATH"
}

$toolVersion = $env:WIX_TOOL_VERSION
$installArgs = @('--global', 'wix')
if (-not [string]::IsNullOrWhiteSpace($toolVersion)) {
    $installArgs += @('--version', $toolVersion)
}

dotnet tool update @installArgs 2>&1 | Tee-Object -Variable updateOutput | Out-Null
$updateExitCode = $LASTEXITCODE
if ($updateExitCode -ne 0) {
    if ($updateOutput) {
        Write-Warning "dotnet tool update failed with exit code ${updateExitCode}:`n$updateOutput"
    }
    else {
        Write-Warning "dotnet tool update failed with exit code $updateExitCode."
    }

    dotnet tool install @installArgs 2>&1 | Tee-Object -Variable installOutput | Out-Null
    if ($LASTEXITCODE -ne 0) {
        if ($installOutput) {
            Write-Error "Failed to install WiX CLI via dotnet tool (exit code ${LASTEXITCODE}):`n$installOutput"
        }
        else {
            Write-Error "Failed to install WiX CLI via dotnet tool (exit code $LASTEXITCODE)."
        }
        exit $LASTEXITCODE
    }
}

if (-not [string]::IsNullOrWhiteSpace($env:WIX_EXTENSION)) {
    $extensionCoordinate = $env:WIX_EXTENSION
    if (-not [string]::IsNullOrWhiteSpace($env:WIX_EXTENSION_VERSION)) {
        $extensionCoordinate = "$extensionCoordinate/$($env:WIX_EXTENSION_VERSION)"
    }
    wix extension add -g $extensionCoordinate
}
