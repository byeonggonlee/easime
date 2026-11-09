# Builds the Windows installer: <OutDir>\easime-<version>-x64.msi (default target\),
# and setup.exe around it: <OutDir>\easime-setup-<version>-x64.exe (bundle.wxs),
# which also fetches Microsoft's Windows App Runtime when a PC lacks it.
#
# Needs the Rust toolchain and WiX 5 (`dotnet tool install --global wix --version 5.0.2`);
# the WiX extensions it uses (UI, BootstrapperApplications, Util) are downloaded
# by this script.
# Run from anywhere on Windows: pwsh packaging\windows\build.ps1 [-Version 0.1.0]
#
# The installer holds the text service (the 64-bit DLL), registers it with Windows,
# switches easime on for the installing user, and has the settings app, the
# Japanese dictionary, the terminal demo and the licenses; see
# packaging\windows\easime.wxs.
param(
    # The MSI version. Defaults to the version of the easime-cli crate. It must
    # be numeric (major.minor.build), so a pre-release tag needs a number here.
    [string]$Version,
    # Where the .msi goes (relative paths are relative to the repository).
    [string]$OutDir = "target",
    # The Japanese dictionary to install. Build it first, in Git's bash:
    #   sh scripts/build-dictionary.sh mozc
    [string]$Dictionary = "target\dictionary\mozc.bin"
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

# The terminal demo, and the text service. The text service is a DLL inside other
# programs, so it is built with panics that unwind (and are caught at its entry
# points) instead of killing the program: the `dll` profile, see Cargo.toml.
cargo build --release --locked -p easime-cli
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
cargo build --profile dll --locked -p frontend-tsf
if ($LASTEXITCODE -ne 0) { throw "cargo build of the text service failed" }
$tsfDll = Join-Path $root "target\dll\frontend_tsf.dll"
if (-not (Test-Path $tsfDll)) { throw "frontend_tsf.dll was not built" }

# The settings app (WinUI 3), its own cargo project. It runs on the Windows App
# Runtime installed from Microsoft, which the installer does not carry.
cargo build --release --locked --manifest-path settings\windows\Cargo.toml
if ($LASTEXITCODE -ne 0) { throw "cargo build of the settings app failed" }
$settingsExe = Join-Path $root "settings\windows\target\release\easime-settings.exe"
if (-not (Test-Path $settingsExe)) { throw "easime-settings.exe was not built" }

if (-not [System.IO.Path]::IsPathRooted($Dictionary)) { $Dictionary = Join-Path $root $Dictionary }
if (-not (Test-Path $Dictionary)) {
    throw "the Japanese dictionary $Dictionary is missing; build it with: sh scripts/build-dictionary.sh mozc"
}

$wix = Join-Path $env:USERPROFILE ".dotnet\tools\wix.exe"
if (-not (Test-Path $wix)) { $wix = "wix" }

# The setup window (WixUI), and the bundle's window and searches
# (BootstrapperApplications, Util), are
# extensions of WiX. They are taken from NuGet at the version of WiX itself and
# passed by path, rather than through `wix extension add`, whose global cache
# once claimed Bal was there when it was not and once hung the CI.
$wixVersion = ((& $wix --version) -split '\+')[0].Trim()
$extensions = Join-Path $root "target\wix-extensions"
# (The bootstrapper extension is called BootstrapperApplications since WiX 5;
# its old name, Bal, ships a DLL by the new name, which -ext Bal does not find.)
function Get-WixExtension([string]$name) {
    $folder = Join-Path $extensions "$name.$wixVersion"
    $dll = Join-Path $folder "wixext5\$name.dll"
    if (-not (Test-Path $dll)) {
        New-Item -ItemType Directory -Force $folder | Out-Null
        $package = Join-Path $extensions "$name.$wixVersion.nupkg"
        Write-Host "$(Get-Date -Format HH:mm:ss) downloading the WiX extension $name $wixVersion"
        Invoke-WebRequest -Uri "https://www.nuget.org/api/v2/package/$name/$wixVersion" -OutFile $package -TimeoutSec 300
        Expand-Archive -Path $package -DestinationPath $folder -Force
        if (-not (Test-Path $dll)) {
            Get-ChildItem $folder -Recurse -Filter *.dll | ForEach-Object { Write-Host "  $($_.FullName)" }
            throw "the WiX extension $name $wixVersion has no $dll"
        }
    }
    return $dll
}
$uiExtension = Get-WixExtension "WixToolset.UI.wixext"
$balExtension = Get-WixExtension "WixToolset.BootstrapperApplications.wixext"
$utilExtension = Get-WixExtension "WixToolset.Util.wixext"

if (-not [System.IO.Path]::IsPathRooted($OutDir)) { $OutDir = Join-Path $root $OutDir }
New-Item -ItemType Directory -Force $OutDir | Out-Null
$out = Join-Path $OutDir "easime-$Version-x64.msi"
$binaries = Join-Path $root "target\release"
$source = Join-Path $root "packaging\windows\easime.wxs"
Write-Host "$(Get-Date -Format HH:mm:ss) building $out"
$strings = Join-Path $root "packaging\windows\strings.wxl"
& $wix build -arch x64 -ext $uiExtension -culture en-US -loc $strings -d "Version=$Version" -d "Binaries=$binaries" -d "TsfDll=$tsfDll" -d "Dictionary=$Dictionary" -d "SettingsExe=$settingsExe" -d "Root=$root" -o $out $source
if ($LASTEXITCODE -ne 0) { throw "wix build failed" }
Write-Host "built $out"

$setup = Join-Path $OutDir "easime-setup-$Version-x64.exe"
$bundle = Join-Path $root "packaging\windows\bundle.wxs"
Write-Host "$(Get-Date -Format HH:mm:ss) building $setup"
& $wix build -arch x64 -ext $balExtension -ext $utilExtension -d "Version=$Version" -d "Msi=$out" -d "Root=$root" -o $setup $bundle
if ($LASTEXITCODE -ne 0) { throw "wix build of setup.exe failed" }
Write-Host "built $setup"
Write-Output $out
