# The Windows App Runtime the settings app needs (WinUI 3), from Microsoft.
# easime does not redistribute it (docs/decisions.md, "설정 GUI"): the
# installer downloads Microsoft's own runtime installer when the runtime is
# missing. This script finds that installer, says what it is (where it really
# comes from, its size, hash and signature, which the installer bundle pins),
# and with -Install runs it and shows the runtime packages Windows then has.
#
#   pwsh packaging\windows\runtime.ps1 [-Install]
param([switch]$Install)

$ErrorActionPreference = "Stop"
# The runtime the settings app is built against (windows-reactor 0.100 asks
# for Microsoft.WindowsAppRuntime.2, version 2.4 or later).
$channel = "2.4"
$candidates = @(
    "https://aka.ms/windowsappsdk/$channel/latest/windowsappruntimeinstall-x64.exe"
)
$file = Join-Path ([IO.Path]::GetTempPath()) "windowsappruntimeinstall-x64.exe"

# What an installer can see of the runtime without asking Windows' package
# manager. The installer bundle can only search the registry and files, so this
# lists where the registry mentions the runtime's package family, to find a key
# the bundle can test (the package repository's family key turned out not to be
# there: CI, 2026-11-09).
$roots = @(
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Appx",
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModel",
    "HKLM:\SOFTWARE\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel",
    "Registry::HKEY_CURRENT_USER\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel"
)
function Show-Family([string]$when) {
    Write-Host "registry keys naming Microsoft.WindowsAppRuntime.2 $when`:"
    foreach ($root in $roots) {
        if (-not (Test-Path $root)) { Write-Host "  (no $root)"; continue }
        Get-ChildItem $root -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.PSChildName -like "Microsoft.WindowsAppRuntime.2*" } |
            Select-Object -First 30 |
            ForEach-Object { Write-Host "  $($_.Name)" }
    }
}
Show-Family "before"

$found = $null
foreach ($url in $candidates) {
    try {
        $response = Invoke-WebRequest -Uri $url -OutFile $file -PassThru -MaximumRedirection 10
        $found = $url
        Write-Host "downloaded $url"
        Write-Host "  from: $($response.BaseResponse.RequestMessage.RequestUri)"
        break
    } catch {
        Write-Host "not there: $url ($($_.Exception.Message))"
    }
}
if (-not $found) { throw "no Windows App Runtime installer found for $channel" }

$item = Get-Item $file
$signature = Get-AuthenticodeSignature $file
Write-Host "  size: $($item.Length) bytes"
Write-Host "  sha512: $((Get-FileHash -Algorithm SHA512 $file).Hash)"
Write-Host "  version: $($item.VersionInfo.ProductVersion) ($($item.VersionInfo.FileDescription))"
Write-Host "  signature: $($signature.Status), $($signature.SignerCertificate.Subject)"
if ($signature.Status -ne "Valid" -or $signature.SignerCertificate.Subject -notmatch "O=Microsoft Corporation") {
    throw "the runtime installer is not validly signed by Microsoft"
}

if ($Install) {
    $process = Start-Process $file -ArgumentList "--quiet" -Wait -PassThru
    Write-Host "runtime installer exit code: $($process.ExitCode)"
    if ($process.ExitCode -ne 0) { throw "the runtime installer failed ($($process.ExitCode))" }
    powershell.exe -NoProfile -Command "Get-AppxPackage -AllUsers -Name 'Microsoft.WindowsAppRuntime*' | Sort-Object Name | ForEach-Object { '  ' + `$_.Name + ' ' + `$_.Version + ' ' + `$_.Architecture }"
    Show-Family "after"
}
