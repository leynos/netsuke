[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Beta1Msi,
    [Parameter(Mandatory)]
    [string]$Beta2Msi,
    [Parameter(Mandatory)]
    [string]$FinalMsi,
    [Parameter(Mandatory)]
    [string]$LogDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-MsiProperty {
    param(
        [Parameter(Mandatory)]
        [string]$MsiPath,
        [Parameter(Mandatory)]
        [string]$PropertyName
    )

    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $installer.OpenDatabase($MsiPath, 0)
    $view = $database.OpenView("SELECT `Value` FROM `Property` WHERE `Property`='$PropertyName'")
    [void]$view.Execute()
    $record = $view.Fetch()
    if ($null -eq $record) {
        throw "MSI $MsiPath does not define ProductCode."
    }
    return [string]$record.StringData(1)
}

function Invoke-MsiInstall {
    param(
        [Parameter(Mandatory)]
        [string]$MsiPath,
        [Parameter(Mandatory)]
        [string]$LogPath
    )

    $arguments = "/i `"$MsiPath`" /qn /norestart /l*v `"$LogPath`""
    $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $arguments -Wait -PassThru
    return $process.ExitCode
}

function Assert-PayloadEquals {
    param(
        [Parameter(Mandatory)]
        [string]$Path,
        [Parameter(Mandatory)]
        [string]$ExpectedPayload
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Expected installed payload at $Path was not found."
    }
    $payload = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($Path))
    if ($payload -ne $ExpectedPayload) {
        throw "Expected installed payload $ExpectedPayload at $Path, got $payload."
    }
}

function Assert-PathAbsent {
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )

    if (Test-Path -LiteralPath $Path) {
        throw "Expected $Path to be absent after the upgrade."
    }
}

function Assert-InstalledProduct {
    param(
        [Parameter(Mandatory)]
        [string]$ProductCode,
        [Parameter(Mandatory)]
        [int]$ExpectedRank,
        [Parameter(Mandatory)]
        [string]$ExpectedPayload,
        [Parameter(Mandatory)]
        [string]$PayloadPath
    )

    $uninstallPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$ProductCode"
    if (-not (Test-Path -LiteralPath $uninstallPath)) {
        throw "Expected installed product $ProductCode was not registered."
    }
    $displayName = Get-ItemPropertyValue -LiteralPath $uninstallPath -Name DisplayName
    if ($displayName -ne 'Netsuke MSI Upgrade Validation 1.2.3') {
        throw "Expected installed product identity, got $displayName."
    }
    $installedRank = Get-ItemPropertyValue -LiteralPath 'HKLM:\SOFTWARE\Leynos\Netsuke\1.2.3' -Name ReleaseRank
    if ($installedRank -ne $ExpectedRank) {
        throw "Expected ReleaseRank $ExpectedRank, got $installedRank."
    }
    Assert-PayloadEquals -Path $PayloadPath -ExpectedPayload $ExpectedPayload
}

function Assert-ProductAbsent {
    param(
        [Parameter(Mandatory)]
        [string]$ProductCode
    )

    $uninstallPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$ProductCode"
    if (Test-Path -LiteralPath $uninstallPath) {
        throw "Superseded product $ProductCode is still registered."
    }
}

function Install-AndAssert {
    param(
        [Parameter(Mandatory)]
        [string]$MsiPath,
        [Parameter(Mandatory)]
        [string]$ProductCode,
        [Parameter(Mandatory)]
        [hashtable]$ExpectedInstallation
    )

    $exitCode = Invoke-MsiInstall -MsiPath $MsiPath -LogPath (Join-Path $LogDirectory $ExpectedInstallation.LogName)
    if ($exitCode -ne 0) {
        throw "Installing $MsiPath failed with msiexec exit code $exitCode."
    }
    Assert-InstalledProduct `
        -ProductCode $ProductCode `
        -ExpectedRank $ExpectedInstallation.Rank `
        -ExpectedPayload $ExpectedInstallation.Payload `
        -PayloadPath $ExpectedInstallation.PayloadPath
}

function Assert-DowngradeIsRejected {
    param(
        [Parameter(Mandatory)]
        [string]$MsiPath,
        [Parameter(Mandatory)]
        [string]$LogName
    )

    $exitCode = Invoke-MsiInstall -MsiPath $MsiPath -LogPath (Join-Path $LogDirectory $LogName)
    if ($exitCode -eq 0) {
        throw "Downgrade MSI $MsiPath installed successfully."
    }
}

$legacyProgramFiles = ${env:ProgramFiles(x86)}
if ([string]::IsNullOrWhiteSpace($legacyProgramFiles)) {
    $legacyProgramFiles = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFilesX86)
}
$nativeProgramFiles = $env:ProgramW6432
if ([string]::IsNullOrWhiteSpace($nativeProgramFiles)) {
    $nativeProgramFiles = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)
}
$legacyDirectory = Join-Path $legacyProgramFiles 'Netsuke MSI Upgrade Validation'
$nativeDirectory = Join-Path $nativeProgramFiles 'Netsuke MSI Upgrade Validation'
$legacyApplication = Join-Path $legacyDirectory 'netsuke.exe'
$nativeApplication = Join-Path $nativeDirectory 'netsuke.exe'

$beta1ProductCode = Get-MsiProperty -MsiPath $Beta1Msi -PropertyName ProductCode
$beta2ProductCode = Get-MsiProperty -MsiPath $Beta2Msi -PropertyName ProductCode
$finalProductCode = Get-MsiProperty -MsiPath $FinalMsi -PropertyName ProductCode
$beta1Installation = @{
    Rank = 1
    Payload = 'MZbeta1'
    PayloadPath = $legacyApplication
    LogName = 'beta1-install.log'
}
$beta2Installation = @{
    Rank = 2
    Payload = 'MZbeta2'
    PayloadPath = $nativeApplication
    LogName = 'beta2-install.log'
}
$finalInstallation = @{
    Rank = 65535
    Payload = 'MZfinal'
    PayloadPath = $nativeApplication
    LogName = 'final-install.log'
}
if (
    ($beta1ProductCode -eq $beta2ProductCode) -or
    ($beta1ProductCode -eq $finalProductCode) -or
    ($beta2ProductCode -eq $finalProductCode)
) {
    throw 'Each MSI fixture must have a generated, distinct ProductCode.'
}

# beta1 is the frozen beta4 layout, so it proves the starting point really is
# the 32-bit directory and that nothing already sits in the native one.
Install-AndAssert -MsiPath $Beta1Msi -ProductCode $beta1ProductCode -ExpectedInstallation $beta1Installation
Assert-PathAbsent -Path $nativeApplication

# beta2 moves the payload: the old product, executable, and directory must all
# be gone, and the new payload must be exactly where a native package installs.
Install-AndAssert -MsiPath $Beta2Msi -ProductCode $beta2ProductCode -ExpectedInstallation $beta2Installation
Assert-ProductAbsent -ProductCode $beta1ProductCode
Assert-PathAbsent -Path $legacyApplication
Assert-PathAbsent -Path $legacyDirectory

# A refused downgrade must leave beta2 registered, ranked, and installed where
# it was, without resurrecting the legacy layout.
Assert-DowngradeIsRejected -MsiPath $Beta1Msi -LogName beta1-downgrade.log
Assert-InstalledProduct `
    -ProductCode $beta2ProductCode `
    -ExpectedRank $beta2Installation.Rank `
    -ExpectedPayload $beta2Installation.Payload `
    -PayloadPath $beta2Installation.PayloadPath
Assert-PathAbsent -Path $legacyApplication
Assert-PathAbsent -Path $legacyDirectory

Install-AndAssert -MsiPath $FinalMsi -ProductCode $finalProductCode -ExpectedInstallation $finalInstallation
Assert-ProductAbsent -ProductCode $beta2ProductCode
Assert-DowngradeIsRejected -MsiPath $Beta2Msi -LogName beta2-downgrade.log
Assert-InstalledProduct `
    -ProductCode $finalProductCode `
    -ExpectedRank $finalInstallation.Rank `
    -ExpectedPayload $finalInstallation.Payload `
    -PayloadPath $finalInstallation.PayloadPath
