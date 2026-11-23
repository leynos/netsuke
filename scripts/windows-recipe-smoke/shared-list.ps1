<#
.SYNOPSIS
Exercises shared PowerShell command-list interpolation in its own fixtures.

.DESCRIPTION
The neutral and cross-entry scenarios each build their own temporary
workspace, so they live beside the main smoke script rather than inside it.
The caller dot-sources this file after defining Invoke-Netsuke and
Assert-Equal, which the scenarios use.
#>

function Test-NeutralSharedListInterpolation {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke,

        [Parameter(Mandatory)]
        [string]$Manifest,

        [Parameter(Mandatory)]
        [string]$FixtureName
    )

    # A here-string carries line feeds, which the POSIX route cannot place in
    # one Ninja command binding, so this PowerShell-only target keeps its own
    # fixture and workspace. The shared fixture stays lowerable under Bash.
    $listManifest = Join-Path (Split-Path -Parent $Manifest) $FixtureName
    $tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
    $workspace = Join-Path $tempRoot "netsuke-windows-neutral-list-smoke-$PID"

    try {
        New-Item -ItemType Directory -Path $workspace | Out-Null
        Copy-Item -LiteralPath $listManifest -Destination (Join-Path $workspace 'Netsukefile')
        Push-Location $workspace
        try {
            $payloadSource = Join-Path $workspace '; New-Item pwned; Write-Output x'
            # New-Item's -Path parameter treats this semicolon as literal data.
            New-Item -ItemType File -Path $payloadSource | Out-Null
            Set-Content -NoNewline -LiteralPath $payloadSource -Value 'path-derived-content'

            Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'shared list.txt')
            Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'shared list.txt') `
                -Expected 'path-derived-content' `
                -Message 'A marker after a neutral shared-list entry was not interpolated as literal path text'
            if (Test-Path -LiteralPath 'pwned') {
                throw 'A shared command list executed its path-derived payload.'
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

function Test-CrossEntryListPlaceholderRejection {
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

    $listManifest = Join-Path (Split-Path -Parent $Manifest) $FixtureName
    $tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
    $workspace = Join-Path $tempRoot "netsuke-windows-cross-entry-list-smoke-$PID"

    try {
        New-Item -ItemType Directory -Path $workspace | Out-Null
        Copy-Item -LiteralPath $listManifest -Destination (Join-Path $workspace 'Netsukefile')
        Push-Location $workspace
        try {
            $payloadSource = Join-Path $workspace '; New-Item pwned; Write-Output x'
            # New-Item's -Path parameter treats this semicolon as literal data.
            New-Item -ItemType File -Path $payloadSource | Out-Null
            $failure = & $Netsuke --json --locale en-GB build $TargetName 2>&1
            if ($LASTEXITCODE -eq 0) {
                throw 'A shared command list that leaves a quote open unexpectedly built successfully.'
            }
            $failureText = [string]::Join([Environment]::NewLine, [string[]]@($failure))
            try {
                $diagnostics = $failureText | ConvertFrom-Json -ErrorAction Stop
            }
            catch {
                throw "The cross-entry list failure did not emit JSON diagnostics: $failureText"
            }
            $entry = $diagnostics.diagnostics[0]
            $causes = @($entry.causes | ForEach-Object { [string]$_ })
            $detail = [string]::Join([Environment]::NewLine,
                [string[]](@([string]$entry.message) + $causes))
            if (-not ($detail -match 'PowerShell command-list entry')) {
                throw "The cross-entry list failure omitted its interpolation diagnostic: $detail"
            }
            # Fluent isolates message arguments, so match across the isolate
            # between the word "entry" and its number.
            if (-not ($detail -match 'entry.*1' -and $detail -match 'single_quoted_string')) {
                throw "The cross-entry list failure did not name the unfinished entry and region: $detail"
            }
            if (Test-Path -LiteralPath 'pwned') {
                throw 'The rejected cross-entry list executed its path-derived payload.'
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

<#
.SYNOPSIS
Runs each Windows recipe smoke scenario in the isolated workspace.

.DESCRIPTION
Keeps scenario ordering in one place while leaving setup and restoration to
the outer smoke-test orchestrator.
#>
