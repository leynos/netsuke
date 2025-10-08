function Get-SafeName {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string] $Value,
        [Parameter(Mandatory = $true)][string] $Fallback
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $Fallback
    }

    $sanitised = $Value -replace '[^A-Za-z0-9._-]', '-'
    $sanitised = $sanitised.Trim('-_.')
    if ([string]::IsNullOrWhiteSpace($sanitised)) {
        return $Fallback
    }

    return $sanitised
}


function Resolve-Architecture {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string] $Value
    )

    $token = $Value.Trim()
    switch -Regex ($token.ToLowerInvariant()) {
        '^(x64|amd64)$' { return 'x64' }
        '^(x86|ia32|win32)$' { return 'x86' }
        '^(arm64|aarch64)$' { return 'arm64' }
        default {
            $message = "Unsupported architecture '$Value'. Use x86, x64, or arm64."
            throw (New-Object System.ArgumentException($message))
        }
    }
}
