# Opens the settings app (a debug build) in several display languages and keeps
# a picture of each window: the CI's check that the app starts, shows its window
# and stays up, with the Windows App Runtime it carries next to it. A throwaway
# settings folder (APPDATA) is used, so the runner's settings are not touched,
# and opening the window must not write anything there.
#
#   pwsh settings\windows\tests\open-window.ps1 -Exe settings\windows\target\debug\easime-settings.exe
param(
    [Parameter(Mandatory)][string]$Exe,
    [string]$Out = "shots-windows"
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out RECT rect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
"@
[Win]::SetProcessDPIAware() | Out-Null
$Exe = (Resolve-Path $Exe).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$appdata = Join-Path ([IO.Path]::GetTempPath()) "easime-settings-window-test"
Remove-Item -Recurse -Force $appdata -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $appdata | Out-Null
$env:APPDATA = $appdata

# The window title in each language (from the shared table; English is checked).
$failed = $false
foreach ($language in "ko", "en", "ja", "zh-Hant", "zh-Hans") {
    $log = Join-Path $Out "settings-$language"
    $process = Start-Process $Exe -ArgumentList "--ui-language=$language" -PassThru `
        -RedirectStandardError "$log.err.log" -RedirectStandardOutput "$log.out.log"
    $deadline = (Get-Date).AddSeconds(90)
    do {
        Start-Sleep -Milliseconds 500
        $process.Refresh()
    } until ($process.HasExited -or $process.MainWindowHandle -ne 0 -or (Get-Date) -gt $deadline)
    if ($process.HasExited -or $process.MainWindowHandle -eq 0) {
        Write-Host "FAILED: $language`: no window (exited: $($process.HasExited), code: $(if ($process.HasExited) { $process.ExitCode }))"
        Get-Content "$log.err.log", "$log.out.log" -ErrorAction SilentlyContinue | ForEach-Object { Write-Host "  $_" }
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
        $failed = $true
        continue
    }
    # Time to draw, in front.
    $window = $process.MainWindowHandle
    [Win]::SetForegroundWindow($window) | Out-Null
    Start-Sleep -Seconds 4
    $rect = New-Object Win+RECT
    [Win]::GetWindowRect($window, [ref]$rect) | Out-Null
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    Write-Host "$language`: '$($process.MainWindowTitle)' at $($rect.Left),$($rect.Top), $width x $height"
    if ($language -eq "en" -and $process.MainWindowTitle -ne "easime Settings") {
        Write-Host "FAILED: the English window is titled '$($process.MainWindowTitle)'"
        $failed = $true
    }
    if ($width -gt 0 -and $height -gt 0) {
        $picture = New-Object System.Drawing.Bitmap $width, $height
        $graphics = [System.Drawing.Graphics]::FromImage($picture)
        $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $picture.Size)
        $picture.Save("$log.png")
        $graphics.Dispose()
        $picture.Dispose()
    }
    $process.Refresh()
    if ($process.HasExited) {
        Write-Host "FAILED: $language`: the app exited by itself (code $($process.ExitCode))"
        $failed = $true
        continue
    }
    $process.CloseMainWindow() | Out-Null
    if (-not $process.WaitForExit(15000)) {
        Write-Host "FAILED: $language`: the app did not close with its window"
        Stop-Process -Id $process.Id -Force
        $failed = $true
    }
}

if (Test-Path (Join-Path $appdata "easime\config.toml")) {
    Write-Host "FAILED: opening the window wrote a settings file"
    $failed = $true
}
if ($failed) { exit 1 }
Write-Host "the settings window opened and closed in every display language"
