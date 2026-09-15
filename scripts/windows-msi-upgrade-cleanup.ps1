[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string[]]$MsiPaths,
    [Parameter(Mandatory)]
    [string]$LogDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-MsiProductCode {
    param(
        [Parameter(Mandatory)]
        [string]$MsiPath
    )

    if ([string]::IsNullOrWhiteSpace($MsiPath) -or -not (Test-Path -LiteralPath $MsiPath)) {
        return $null
    }
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $installer.OpenDatabase($MsiPath, 0)
    $view = $database.OpenView("SELECT `Value` FROM `Property` WHERE `Property`='ProductCode'")
    $view.Execute()
    $record = $view.Fetch()
    if ($null -eq $record) {
        return $null
    }
    $record.StringData(1)
}

foreach ($msiPath in $MsiPaths) {
    $productCode = Get-MsiProductCode -MsiPath $msiPath
    if ($null -eq $productCode) {
        continue
    }
    $uninstallPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$productCode"
    if (-not (Test-Path -LiteralPath $uninstallPath)) {
        continue
    }
    $logName = "cleanup-$($productCode.Trim('{}')).log"
    $logPath = Join-Path $LogDirectory $logName
    & msiexec.exe /x $productCode /qn /norestart /l*v $logPath
    if ($LASTEXITCODE -ne 0) {
        throw "Removing MSI validation product $productCode failed with exit code $LASTEXITCODE."
    }
}
