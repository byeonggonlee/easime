# Builds the Windows installer: <OutDir>\easime-<version>-x64.msi (default target\)
#
# Needs the Rust toolchain and WiX 5 (`dotnet tool install --global wix --version 5.0.2`).
# Run from anywhere on Windows: pwsh packaging\windows\build.ps1 [-Version 0.1.0]
#
# This installer only contains the terminal demo and the licenses; see
# packaging\windows\easime.wxs.
param(
    # The MSI version. Defaults to the version of the easime-cli crate. It must
    # be numeric (major.minor.build), so a pre-release tag needs a number here.
    [string]$Version,
    # Where the .msi goes (relative paths are relative to the repository).
    [string]$OutDir = "target"
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $root

if (-not $Version) {
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $Version = ($metadata.packages | Where-Object { $_.name -eq "easime-cli" }).version
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "MSI versions are numeric (major.minor.build); got '$Version'"
}

cargo build --release --locked -p easime-cli
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$wix = Join-Path $env:USERPROFILE ".dotnet\tools\wix.exe"
if (-not (Test-Path $wix)) { $wix = "wix" }

if (-not [System.IO.Path]::IsPathRooted($OutDir)) { $OutDir = Join-Path $root $OutDir }
New-Item -ItemType Directory -Force $OutDir | Out-Null
$out = Join-Path $OutDir "easime-$Version-x64.msi"
$binaries = Join-Path $root "target\release"
$source = Join-Path $root "packaging\windows\easime.wxs"
& $wix build -arch x64 -d "Version=$Version" -d "Binaries=$binaries" -d "Root=$root" -o $out $source
if ($LASTEXITCODE -ne 0) { throw "wix build failed" }

Write-Host "built $out"
Write-Output $out
