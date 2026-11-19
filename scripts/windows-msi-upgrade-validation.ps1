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

# The package refuses a downgrade through a launch condition, and Windows
# Installer reports a failed launch condition as ERROR_INSTALL_FAILURE. Treating
# every non-zero code as a refusal would also accept 1618 (another installation
# in progress), 1619 or 1620 (unreadable package) and 1602 (cancelled), none of
# which says anything about downgrade handling.
$downgradeRefusalExitCode = 1603

function Get-MsiColumnValue {
    param(
        [Parameter(Mandatory)]
        [string]$MsiPath,
        [Parameter(Mandatory)]
        [string]$Query,
        [Parameter(Mandatory)]
        [string]$MissingMessage
    )

    # The database and view stay inside this function: the caller gets a plain
    # string, so nothing depends on a COM view outliving the database that owns
    # it.
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $installer.OpenDatabase($MsiPath, 0)
    $view = $database.OpenView($Query)
    [void]$view.Execute()
    $record = $view.Fetch()
    if ($null -eq $record) {
        throw "MSI $MsiPath $MissingMessage"
    }
    return [string]$record.StringData(1)
}

function Get-MsiProperty {
    param(
        [Parameter(Mandatory)]
        [string]$MsiPath,
        [Parameter(Mandatory)]
        [string]$PropertyName
    )

    return Get-MsiColumnValue `
        -MsiPath $MsiPath `
        -Query "SELECT `Value` FROM `Property` WHERE `Property`='$PropertyName'" `
        -MissingMessage "does not define $PropertyName."
}

function Get-DowngradeRefusalMessage {
    param(
        [Parameter(Mandatory)]
        [string]$MsiPath
    )

    # The package authors the refusal text itself, so read it from the built
    # MSI rather than copying it here: a copy would drift from
    # installer/Package.wxs, and the log must carry exactly this sentence for
    # the attempt to count as a refusal. The rank comparison is what refuses
    # the downgrade, so its launch condition is the row that carries the
    # sentence; the MajorUpgrade downgrade message belongs to a different
    # check that never runs between equal numeric versions.
    #
    # Windows Installer's SQL has no LIKE operator, so the row is chosen in
    # PowerShell: the query itself fails to parse rather than returning no
    # rows, which is why the whole table is read here.
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $installer.OpenDatabase($MsiPath, 0)
    $view = $database.OpenView('SELECT `Condition`, `Description` FROM `LaunchCondition`')
    [void]$view.Execute()
    $template = $null
    $record = $view.Fetch()
    while ($null -ne $record) {
        $condition = [string]$record.StringData(1)
        if (($null -eq $template) -and ($condition -like '*NETSUKE_RELEASE_RANK*')) {
            $template = [string]$record.StringData(2)
        }
        $record = $view.Fetch()
    }
    if ($null -eq $template) {
        throw "MSI $MsiPath declares no release-rank launch condition."
    }
    $productName = Get-MsiProperty -MsiPath $MsiPath -PropertyName ProductName
    return $template.Replace('[ProductName]', $productName)
}

function Invoke-MsiExec {
    param(
        [Parameter(Mandatory)]
        [ValidateSet('Install', 'Uninstall')]
        [string]$Operation,
        [Parameter(Mandatory)]
        [string]$Target,
        [Parameter(Mandatory)]
        [string]$LogName
    )

    # `/i` takes a package path, which has to be quoted; `/x` takes a product
    # code, which must not be.
    $verb = if ($Operation -eq 'Install') { "/i `"$Target`"" } else { "/x $Target" }
    $arguments = "$verb /qn /norestart /l*v `"$(Join-Path $LogDirectory $LogName)`""
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
        throw "Expected $Path to be absent."
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

function Assert-NoRelatedProductInstalled {
    param(
        [Parameter(Mandatory)]
        [string]$UpgradeCode
    )

    # A leftover installation from an earlier run of this fixture family has a
    # different ProductCode but the same UpgradeCode, so the final package
    # would upgrade it and the checks labelled clean install would still pass.
    # RelatedProducts returns a COM StringList, which PowerShell cannot
    # enumerate with foreach: it would hand back the list itself as one opaque
    # item and report every runner as dirty. Walk the list through its own
    # Count and Item members instead.
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $relatedProducts = $installer.RelatedProducts($UpgradeCode)
    if ($null -eq $relatedProducts) {
        return
    }
    for ($index = 1; $index -le $relatedProducts.Count; $index++) {
        $productCode = $relatedProducts.Item($index)
        throw "Product $productCode related to upgrade code $UpgradeCode is already installed."
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

    $exitCode = Invoke-MsiExec `
        -Operation Install `
        -Target $MsiPath `
        -LogName $ExpectedInstallation.LogName
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
        [string]$LogName,
        [Parameter(Mandatory)]
        [string]$ExpectedRefusalMessage
    )

    $logPath = Join-Path $LogDirectory $LogName
    $exitCode = Invoke-MsiExec -Operation Install -Target $MsiPath -LogName $LogName
    if ($exitCode -ne $downgradeRefusalExitCode) {
        throw "Downgrade MSI $MsiPath returned msiexec exit code $exitCode rather than the refusal code $downgradeRefusalExitCode."
    }
    # The code alone only says the install failed. The package's own refusal
    # sentence in this attempt's log is what distinguishes a downgrade the
    # package rejected from an unrelated failure that happened to abort it.
    if (-not (Select-String -LiteralPath $logPath -SimpleMatch -Pattern $ExpectedRefusalMessage -Quiet)) {
        throw "Downgrade attempt with $MsiPath did not record '$ExpectedRefusalMessage' in $logPath."
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
$beta1UpgradeCode = Get-MsiProperty -MsiPath $Beta1Msi -PropertyName UpgradeCode
$beta2UpgradeCode = Get-MsiProperty -MsiPath $Beta2Msi -PropertyName UpgradeCode
$finalUpgradeCode = Get-MsiProperty -MsiPath $FinalMsi -PropertyName UpgradeCode
$beta1RefusalMessage = Get-DowngradeRefusalMessage -MsiPath $Beta1Msi
$beta2RefusalMessage = Get-DowngradeRefusalMessage -MsiPath $Beta2Msi
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
$cleanInstallation = $finalInstallation.Clone()
$cleanInstallation.LogName = 'final-clean-install.log'
if (
    ($beta1ProductCode -eq $beta2ProductCode) -or
    ($beta1ProductCode -eq $finalProductCode) -or
    ($beta2ProductCode -eq $finalProductCode)
) {
    throw 'Each MSI fixture must have a generated, distinct ProductCode.'
}
if (
    ($beta1UpgradeCode -ne $beta2UpgradeCode) -or
    ($beta1UpgradeCode -ne $finalUpgradeCode)
) {
    throw 'Each MSI fixture must share the same UpgradeCode for the upgrade sequence.'
}

# A clean install is the case issue #1143 reported: with no earlier
# installation, the package must populate only the native directory. The
# upgrade sequence cannot show that on its own, because its first fixture
# deliberately installs the legacy layout. Refusing a dirty runner keeps a
# leftover installation, including one from an earlier run of this fixture
# family, from being misreported as a clean install.
Assert-ProductAbsent -ProductCode $beta1ProductCode
Assert-ProductAbsent -ProductCode $beta2ProductCode
Assert-ProductAbsent -ProductCode $finalProductCode
Assert-NoRelatedProductInstalled -UpgradeCode $finalUpgradeCode
Assert-PathAbsent -Path $legacyApplication
Assert-PathAbsent -Path $legacyDirectory
Assert-PathAbsent -Path $nativeApplication
Assert-PathAbsent -Path $nativeDirectory
Install-AndAssert `
    -MsiPath $FinalMsi `
    -ProductCode $finalProductCode `
    -ExpectedInstallation $cleanInstallation
Assert-PathAbsent -Path $legacyApplication
$cleanUninstallCode = Invoke-MsiExec `
    -Operation Uninstall `
    -Target $finalProductCode `
    -LogName 'final-clean-uninstall.log'
if ($cleanUninstallCode -ne 0) {
    throw "Uninstalling $finalProductCode failed with msiexec exit code $cleanUninstallCode."
}
Assert-ProductAbsent -ProductCode $finalProductCode
Assert-PathAbsent -Path $nativeApplication

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
Assert-DowngradeIsRejected `
    -MsiPath $Beta1Msi `
    -LogName beta1-downgrade.log `
    -ExpectedRefusalMessage $beta1RefusalMessage
Assert-InstalledProduct `
    -ProductCode $beta2ProductCode `
    -ExpectedRank $beta2Installation.Rank `
    -ExpectedPayload $beta2Installation.Payload `
    -PayloadPath $beta2Installation.PayloadPath
Assert-PathAbsent -Path $legacyApplication
Assert-PathAbsent -Path $legacyDirectory

Install-AndAssert -MsiPath $FinalMsi -ProductCode $finalProductCode -ExpectedInstallation $finalInstallation
Assert-ProductAbsent -ProductCode $beta2ProductCode
Assert-DowngradeIsRejected `
    -MsiPath $Beta2Msi `
    -LogName beta2-downgrade.log `
    -ExpectedRefusalMessage $beta2RefusalMessage
Assert-InstalledProduct `
    -ProductCode $finalProductCode `
    -ExpectedRank $finalInstallation.Rank `
    -ExpectedPayload $finalInstallation.Payload `
    -PayloadPath $finalInstallation.PayloadPath
