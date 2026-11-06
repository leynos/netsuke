<#
.SYNOPSIS
Checks PowerShell path-placeholder safety in the Windows recipe smoke suite.

.DESCRIPTION
Runs safe unquoted path placeholders and confirms quoted script markers fail
closed before a semicolon-bearing source path can become PowerShell syntax.
Also places a marker-writing decoy named `powershell.exe` in the working
directory and at the front of `PATH`, then requires short and oversized recipes
to still run under the trusted system-directory executable.
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

function New-PowerShellSearchDecoy {
    param(
        [Parameter(Mandatory)]
        [string]$Directory
    )

    # Add-Type refuses -OutputType ConsoleApplication as of PowerShell 7.1, and
    # this suite runs under PowerShell Core, so the decoy is compiled by the
    # .NET Framework C# compiler that ships with Windows instead.
    $compilerCandidates = @(
        (Join-Path $env:SystemRoot 'Microsoft.NET\Framework64\v4.0.30319\csc.exe')
        (Join-Path $env:SystemRoot 'Microsoft.NET\Framework\v4.0.30319\csc.exe')
    )
    $compiler = $compilerCandidates |
        Where-Object { Test-Path -LiteralPath $_ } |
        Select-Object -First 1
    if (-not $compiler) {
        throw "The .NET Framework C# compiler is unavailable; looked for $($compilerCandidates -join ', ')."
    }

    $source = @'
public static class Program {
    public static int Main(string[] args) {
        System.IO.File.WriteAllText(
            "decoy-powershell-marker.txt",
            "powershell decoy executed");
        return 41;
    }
}
'@
    $sourceFile = Join-Path $Directory 'powershell-decoy.cs'
    $output = Join-Path $Directory 'powershell.exe'
    try {
        [System.IO.File]::WriteAllText($sourceFile, $source)
        $compilerOutput = & $compiler /nologo /target:exe "/out:$output" $sourceFile 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "The .NET Framework C# compiler failed with exit code ${LASTEXITCODE}: $compilerOutput"
        }
    }
    finally {
        Remove-Item -Force -LiteralPath $sourceFile -ErrorAction SilentlyContinue
    }
}

function Test-PowerShellSearchPathBinding {
    param(
        [Parameter(Mandatory)]
        [string]$Netsuke
    )

    $workspace = (Get-Location).Path
    $decoyPath = Join-Path $workspace 'decoy-path'
    New-Item -ItemType Directory -Path $decoyPath | Out-Null
    New-PowerShellSearchDecoy -Directory $workspace
    New-PowerShellSearchDecoy -Directory $decoyPath
    $environmentState = @(
        Get-EnvironmentVariableState -Name 'PATH'
        Get-EnvironmentVariableState -Name 'NETSUKE_NINJA'
    )
    try {
        # Get-Command may report several ninja applications; NETSUKE_NINJA names
        # exactly one program, so an array would be joined into an invalid path.
        $ninja = Get-Command ninja -CommandType Application | Select-Object -First 1
        if (-not $ninja) {
            throw 'The decoy scenario requires a ninja application on PATH.'
        }
        $env:NETSUKE_NINJA = $ninja.Path
        $env:PATH = $decoyPath + [IO.Path]::PathSeparator + $env:PATH
        # A previous phase writes this output, so a stale copy would satisfy the
        # assertion below even if this build never ran the recipe.
        Remove-Item -Force -LiteralPath 'scalar interpreter with spaces.txt' `
            -ErrorAction SilentlyContinue
        Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'scalar')
        Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'scalar interpreter with spaces.txt') `
            -Expected 'Desktop' -Message 'A working-directory powershell.exe decoy captured a short recipe'
        # Ninja treats an existing output as up to date, so a file left by an
        # earlier invocation would satisfy the assertion without this build.
        Remove-Item -Force -LiteralPath 'large scalar.txt' -ErrorAction SilentlyContinue
        Invoke-Netsuke -Netsuke $Netsuke -Arguments @('build', 'large-scalar')
        Assert-Equal -Actual (Get-Content -Raw -LiteralPath 'large scalar.txt') -Expected 'scalar' `
            -Message 'A PATH powershell.exe decoy captured a response-file recipe'
        if (Test-Path -LiteralPath 'decoy-powershell-marker.txt') {
            throw 'Windows resolved an unqualified powershell.exe through executable search.'
        }
    }
    finally {
        foreach ($state in $environmentState) {
            Restore-EnvironmentVariableState -State $state
        }
        Remove-Item -Recurse -Force -LiteralPath $decoyPath -ErrorAction SilentlyContinue
        # A later phase builds this target again, so a stale output would let
        # its assertion pass without running the response-file recipe.
        Remove-Item -Force -LiteralPath 'large scalar.txt' -ErrorAction SilentlyContinue
        Remove-Item -Force -LiteralPath (Join-Path $workspace 'powershell.exe') -ErrorAction SilentlyContinue
        Remove-Item -Force -LiteralPath (Join-Path $workspace 'decoy-powershell-marker.txt') `
            -ErrorAction SilentlyContinue
    }
}
