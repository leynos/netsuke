<#
.SYNOPSIS
Checks PowerShell path-placeholder safety in the Windows recipe smoke suite.

.DESCRIPTION
Runs safe unquoted path placeholders and confirms quoted script markers fail
closed before a semicolon-bearing source path can become PowerShell syntax.
#>
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
            # New-Item's -Path parameter treats this semicolon as literal data.
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
