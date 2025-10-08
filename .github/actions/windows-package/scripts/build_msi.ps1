[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

. (Join-Path -Path $PSScriptRoot -ChildPath 'common.ps1')

if (-not (Test-Path -LiteralPath $env:WXS_PATH)) {
    Write-Error "WiX source file not found: $env:WXS_PATH"
    exit 1
}

if (-not [string]::IsNullOrWhiteSpace($env:LICENSE_TEXT_PATH)) {
    $pythonCommand = Get-Command -Name python -ErrorAction SilentlyContinue
    $pythonArgs = @()
    if (-not $pythonCommand) {
        $pythonCommand = Get-Command -Name py -ErrorAction SilentlyContinue
        if ($pythonCommand) {
            $pythonArgs = @('-3')
        }
    }
    if (-not $pythonCommand) {
        Write-Error 'Python is required to convert the license text file but neither python nor py -3 was found on PATH.'
        exit 1
    }

    $sourcePath = Resolve-Path -LiteralPath $env:LICENSE_TEXT_PATH -ErrorAction SilentlyContinue
    if (-not $sourcePath) {
        Write-Error "License text file not found: $env:LICENSE_TEXT_PATH"
        exit 1
    }

    $scriptPath = Join-Path -Path $env:GITHUB_ACTION_PATH -ChildPath 'scripts\plaintext_to_rtf.py'
    if (-not (Test-Path -LiteralPath $scriptPath)) {
        Write-Error "Converter script not found: $scriptPath"
        exit 1
    }

    $arguments = @($scriptPath, '--input', $sourcePath.Path)
    if (-not [string]::IsNullOrWhiteSpace($env:LICENSE_RTF_PATH)) {
        $rtfTarget = [System.IO.Path]::GetFullPath($env:LICENSE_RTF_PATH)
        $rtfDirectory = Split-Path -Path $rtfTarget -Parent
        if (-not [string]::IsNullOrWhiteSpace($rtfDirectory) -and -not (Test-Path -LiteralPath $rtfDirectory)) {
            New-Item -ItemType Directory -Path $rtfDirectory -Force | Out-Null
        }
        $arguments += @('--output', $rtfTarget)
    }

    $allArguments = @()
    if ($pythonArgs.Count -gt 0) {
        $allArguments += $pythonArgs
    }
    $allArguments += $arguments

    $process = & $pythonCommand.Source @allArguments
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $generatedPath = $process.Trim()
    if (-not (Test-Path -LiteralPath $generatedPath)) {
        Write-Error "Expected license RTF file was not created: $generatedPath"
        exit 1
    }

    $resolvedRtfPath = (Resolve-Path -LiteralPath $generatedPath).Path
    $env:LICENSE_RTF_PATH = $resolvedRtfPath
    Write-Host "Converted license text to RTF: $resolvedRtfPath"
}

$outDir = $env:OUTPUT_DIRECTORY
if (Test-Path -LiteralPath $outDir) {
    $outDirItem = Get-Item -LiteralPath $outDir
}
else {
    $outDirItem = New-Item -ItemType Directory -Path $outDir
}

$safeBaseName = Get-SafeName -Value $env:OUTPUT_BASENAME -Fallback 'package'
$archInput = if ([string]::IsNullOrWhiteSpace($env:ARCHITECTURE)) { 'x64' } else { $env:ARCHITECTURE }
try {
    $arch = Resolve-Architecture -Value $archInput
}
catch [System.ArgumentException] {
    Write-Error $_.Exception.Message
    exit 1
}

$outputPath = Join-Path -Path $outDirItem.FullName -ChildPath ("{0}-{1}-{2}.msi" -f $safeBaseName, $env:VERSION, $arch)
$arguments = @('build', $env:WXS_PATH)
if (-not [string]::IsNullOrWhiteSpace($env:WIX_EXTENSION)) {
    $extensions = $env:WIX_EXTENSION -split '[,\s]+'
    foreach ($extension in $extensions) {
        $trimmed = $extension.Trim()
        if (-not [string]::IsNullOrWhiteSpace($trimmed)) {
            $arguments += @('-ext', $trimmed)
        }
    }
}

$arguments += @('-arch', $arch, "-dVersion=$($env:VERSION)", '-o', $outputPath)
wix @arguments

if (-not (Test-Path -LiteralPath $outputPath)) {
    Write-Error "MSI build failed: output file '$outputPath' was not created."
    exit 1
}

$resolved = (Resolve-Path -LiteralPath $outputPath).Path
"msi-path=$resolved" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
