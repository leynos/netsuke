[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

function Get-MsiVersion([string] $candidate) {
    if ([string]::IsNullOrWhiteSpace($candidate)) {
        return $null
    }
    $trimmed = $candidate.Trim()
    $pattern = '^[vV]?\d+(\.\d+){0,2}$'
    if ($trimmed -notmatch $pattern) {
        return $null
    }
    $numeric = $trimmed.TrimStart('v', 'V')
    $parts = $numeric.Split('.', [System.StringSplitOptions]::RemoveEmptyEntries)
    while ($parts.Count -lt 3) {
        $parts += '0'
    }
    if ($parts.Count -gt 3) {
        $parts = $parts[0..2]
    }
    $validated = @()
    for ($i = 0; $i -lt $parts.Count; $i++) {
        $part = $parts[$i]
        $value = 0
        if (-not [int]::TryParse($part, [ref]$value)) {
            return $null
        }
        $max = if ($i -eq 2) { 65535 } else { 255 }
        if ($value -lt 0 -or $value -gt $max) {
            return $null
        }
        $validated += $value
    }
    return ($validated -join '.')
}

$versionSource = 'default'
$explicitVersion = $env:INPUT_VERSION
$resolved = $null
if (-not [string]::IsNullOrWhiteSpace($explicitVersion)) {
    $resolved = Get-MsiVersion $explicitVersion
    if ($null -eq $resolved) {
        Write-Error "Invalid MSI version '$explicitVersion'. Provide numeric major.minor.build where major/minor are 0–255 and build is 0–65535."
        exit 1
    }
    $versionSource = 'input'
}

if ($null -eq $resolved) {
    $refType = $env:GITHUB_REF_TYPE
    $refName = $env:GITHUB_REF_NAME
    if ($refType -eq 'tag' -and -not [string]::IsNullOrWhiteSpace($refName)) {
        $resolved = Get-MsiVersion $refName
        if ($null -eq $resolved) {
            Write-Warning "Tag '$refName' does not match v#.#.# semantics required for MSI ProductVersion. Falling back to 0.0.0."
        }
        else {
            $versionSource = "tag '$refName'"
        }
    }
    elseif (-not [string]::IsNullOrWhiteSpace($refName)) {
        Write-Host "Ignoring ref '$refName' of type '$refType' when resolving MSI version."
    }
}

if ($null -eq $resolved) {
    $resolved = '0.0.0'
}

Write-Host "Resolved version ($versionSource): $resolved"
"version=$resolved" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
