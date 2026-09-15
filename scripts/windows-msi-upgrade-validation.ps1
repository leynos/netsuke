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

function Assert-InstalledProduct {
    param(
        [Parameter(Mandatory)]
        [string]$ProductCode,
        [Parameter(Mandatory)]
        [int]$ExpectedRank,
        [Parameter(Mandatory)]
        [string]$ExpectedPayload
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
    $programFilesX86 = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFilesX86)
    $applicationPath = Join-Path $programFilesX86 'Netsuke MSI Upgrade Validation\netsuke.exe'
    $payload = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($applicationPath))
    if ($payload -ne $ExpectedPayload) {
        throw "Expected installed payload $ExpectedPayload, got $payload."
    }
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
    Assert-InstalledProduct -ProductCode $ProductCode -ExpectedRank $ExpectedInstallation.Rank -ExpectedPayload $ExpectedInstallation.Payload
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

$beta1ProductCode = Get-MsiProperty -MsiPath $Beta1Msi -PropertyName ProductCode
$beta2ProductCode = Get-MsiProperty -MsiPath $Beta2Msi -PropertyName ProductCode
$finalProductCode = Get-MsiProperty -MsiPath $FinalMsi -PropertyName ProductCode
$beta1Installation = @{ Rank = 1; Payload = 'MZbeta1'; LogName = 'beta1-install.log' }
$beta2Installation = @{ Rank = 2; Payload = 'MZbeta2'; LogName = 'beta2-install.log' }
$finalInstallation = @{ Rank = 65535; Payload = 'MZfinal'; LogName = 'final-install.log' }
if (
    ($beta1ProductCode -eq $beta2ProductCode) -or
    ($beta1ProductCode -eq $finalProductCode) -or
    ($beta2ProductCode -eq $finalProductCode)
) {
    throw 'Each MSI fixture must have a generated, distinct ProductCode.'
}

Install-AndAssert -MsiPath $Beta1Msi -ProductCode $beta1ProductCode -ExpectedInstallation $beta1Installation
Install-AndAssert -MsiPath $Beta2Msi -ProductCode $beta2ProductCode -ExpectedInstallation $beta2Installation
Assert-ProductAbsent -ProductCode $beta1ProductCode
Assert-DowngradeIsRejected -MsiPath $Beta1Msi -LogName beta1-downgrade.log
Assert-InstalledProduct -ProductCode $beta2ProductCode -ExpectedRank $beta2Installation.Rank -ExpectedPayload $beta2Installation.Payload
Install-AndAssert -MsiPath $FinalMsi -ProductCode $finalProductCode -ExpectedInstallation $finalInstallation
Assert-ProductAbsent -ProductCode $beta2ProductCode
Assert-DowngradeIsRejected -MsiPath $Beta2Msi -LogName beta2-downgrade.log
Assert-InstalledProduct -ProductCode $finalProductCode -ExpectedRank $finalInstallation.Rank -ExpectedPayload $finalInstallation.Payload
