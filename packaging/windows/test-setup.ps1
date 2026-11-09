# Runs setup.exe (packaging/windows/bundle.wxs) on this machine, the CI's Windows
# runner, which has no Windows App Runtime from Microsoft's installer: setup.exe
# must download it from Microsoft, install easime, and the settings app must open
# on it; removing easime must leave Microsoft's runtime; and a second setup must
# find the runtime and not download it again. It needs administrator rights and
# changes the machine; do not run it on a computer you use.
#
#   pwsh packaging\windows\test-setup.ps1 -Setup target\easime-setup-0.1.0-x64.exe
param([Parameter(Mandatory)][string]$Setup)

$ErrorActionPreference = "Stop"
$Setup = (Resolve-Path $Setup).Path
$logs = Join-Path (Get-Location) "setup-logs"
New-Item -ItemType Directory -Force $logs | Out-Null

# What Microsoft's 2.4 runtime installer provisions (bundle.wxs searches for it).
$runtime = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Appx\AppxAllUserStore\Applications\MicrosoftCorporationII.WinAppRuntime.Main.2_2.4.0.0_x64__8wekyb3d8bbwe"
$dir = Join-Path $env:ProgramFiles "easime"
$settingsExe = Join-Path $dir "settings\easime-settings.exe"
$clsid = "{CFC09673-DDBA-4C84-B57F-CF0B0DDC262C}"

function Assert([bool]$condition, [string]$message) {
    if (-not $condition) { throw "FAILED: $message" }
    Write-Host "ok: $message"
}

function Invoke-Setup([string]$what, [string[]]$arguments) {
    $log = Join-Path $logs "$what.log"
    $process = Start-Process $Setup -Wait -PassThru -ArgumentList (@("/quiet", "/norestart", "/log", "`"$log`"") + $arguments)
    Write-Host "setup.exe $what -> exit code $($process.ExitCode)"
    return $process.ExitCode
}

# What the bundle's log says about the runtime package.
function Runtime-Lines([string]$what) {
    @(Get-Content (Join-Path $logs "$what.log") | Where-Object { $_ -match "WindowsAppRuntime" })
}

# Apps & features entries named easime that a user sees (the MSI inside the
# bundle is hidden from the list).
function Entries {
    @(Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*", "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -eq "easime" -and $_.SystemComponent -ne 1 })
}

function Registered { Test-Path "HKLM:\SOFTWARE\Classes\CLSID\$clsid\InprocServer32" }

# Opens the installed settings app and returns its window title (the runtime's
# own message box is titled after the program instead).
function Settings-Title {
    $app = Start-Process $settingsExe -ArgumentList "--ui-language=en" -PassThru
    try {
        $deadline = (Get-Date).AddSeconds(90)
        do { Start-Sleep -Milliseconds 500; $app.Refresh() } until ($app.HasExited -or ($app.MainWindowHandle -ne 0 -and $app.MainWindowTitle) -or (Get-Date) -gt $deadline)
        Start-Sleep -Seconds 2
        $app.Refresh()
        return $app.MainWindowTitle
    } finally {
        if (-not $app.HasExited) { Stop-Process -Id $app.Id -Force }
    }
}

Assert (-not (Test-Path $runtime)) "this PC has no Windows App Runtime from Microsoft's installer"
Assert (-not (Test-Path $dir)) "nothing of easime is installed"

# 1. A PC without the runtime: setup.exe downloads Microsoft's installer first.
$code = Invoke-Setup "install" @()
Runtime-Lines "install" | Select-Object -First 15 | ForEach-Object { Write-Host "  $_" }
Assert ($code -in 0, 3010) "setup.exe installs easime (exit code $code)"
Assert (@(Runtime-Lines "install" | Where-Object { $_ -match "Detected package: WindowsAppRuntime, state: Absent" }).Count -gt 0) "it found no runtime"
Assert (Test-Path $runtime) "Microsoft's runtime is installed"
Assert (Test-Path $settingsExe) "the settings app is installed"
Assert (Registered) "the text service is registered"
Assert ((Entries).Count -eq 1) "Apps & features lists easime once"
$title = Settings-Title
Assert ($title -eq "easime Settings") "the settings app opens on that runtime ('$title')"

# 2. Removing easime leaves Microsoft's runtime, which other programs may use.
$code = Invoke-Setup "uninstall" @("/uninstall")
Assert ($code -in 0, 3010) "setup.exe removes easime (exit code $code)"
Assert (-not (Test-Path $dir)) "the install folder is gone"
Assert (-not (Registered)) "the text service is no longer registered"
Assert ((Entries).Count -eq 0) "Apps & features no longer lists easime"
Assert (Test-Path $runtime) "Microsoft's runtime stays"

# 3. With the runtime there, setup.exe does not download it again.
$code = Invoke-Setup "again" @()
Runtime-Lines "again" | Select-Object -First 15 | ForEach-Object { Write-Host "  $_" }
Assert ($code -in 0, 3010) "setup.exe installs easime again (exit code $code)"
Assert (@(Runtime-Lines "again" | Where-Object { $_ -match "Detected package: WindowsAppRuntime, state: Present" }).Count -gt 0) "it finds the runtime and leaves it alone"
$title = Settings-Title
Assert ($title -eq "easime Settings") "the settings app opens ('$title')"
$code = Invoke-Setup "final-uninstall" @("/uninstall")
Assert ($code -in 0, 3010) "setup.exe removes easime again (exit code $code)"
Assert (-not (Test-Path $dir)) "the install folder is gone"
Write-Host "all setup.exe checks passed"
