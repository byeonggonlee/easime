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

$config = Join-Path $appdata "easime\config.toml"
if (Test-Path $config) {
    Write-Host "FAILED: opening the window wrote a settings file"
    $failed = $true
}
if ($failed) { exit 1 }
Write-Host "the settings window opened and closed in every display language"

# Using it, the way a screen reader or a keyboard user reaches it (UI
# Automation): the pages are listed, the Languages page opens, and switching
# Japanese on writes it to the settings file.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$Automation = [System.Windows.Automation.AutomationElement]
$Descendants = [System.Windows.Automation.TreeScope]::Descendants
function Names($root) {
    @($root.FindAll($Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
        ForEach-Object { $_.Current.Name } | Where-Object { $_ })
}
function Find($root, [string]$name) {
    $condition = New-Object System.Windows.Automation.PropertyCondition($Automation::NameProperty, $name)
    $root.FindFirst($Descendants, $condition)
}
function Check([bool]$condition, [string]$message) {
    if (-not $condition) { throw "FAILED: $message" }
    Write-Host "ok: $message"
}

$process = Start-Process $Exe -ArgumentList "--ui-language=en" -PassThru `
    -RedirectStandardError (Join-Path $Out "use.err.log") -RedirectStandardOutput (Join-Path $Out "use.out.log")
try {
    $deadline = (Get-Date).AddSeconds(90)
    do { Start-Sleep -Milliseconds 500; $process.Refresh() } until ($process.HasExited -or $process.MainWindowHandle -ne 0 -or (Get-Date) -gt $deadline)
    Check ($process.MainWindowHandle -ne 0) "the window opens"
    Start-Sleep -Seconds 3
    $root = $Automation::FromHandle($process.MainWindowHandle)
    $names = Names $root
    Write-Host "what UI Automation sees on the first page: $($names -join ' | ')"
    foreach ($page in "General", "Languages", "Shortcuts", "Appearance") {
        Check ($names -contains $page) "the '$page' page is listed"
    }
    Check (@($names | Where-Object { $_ -like "Settings file*" }).Count -gt 0) "the General page shows where the settings file is"

    $item = Find $root "Languages"
    $selection = $null
    if ($item.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$selection)) {
        $selection.Select()
    } else {
        $item.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    }
    Start-Sleep -Seconds 2
    $names = Names $root
    Write-Host "what UI Automation sees on the Languages page: $($names -join ' | ')"
    Check (@($names | Where-Object { $_ -like "Switching order:*" }).Count -gt 0) "the Languages page shows the switching order"

    $japanese = Find $root ([string]::new([char[]](0x65E5, 0x672C, 0x8A9E)))   # the Japanese name of Japanese
    Check ($null -ne $japanese) "the Japanese switch is there"
    $toggle = $japanese.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
    $before = $toggle.Current.ToggleState
    $toggle.Toggle()
    Start-Sleep -Seconds 2
    Check (Test-Path $config) "switching it writes the settings file"
    $text = Get-Content $config -Raw
    Write-Host "the settings file now:`n$text"
    if ($before -eq [System.Windows.Automation.ToggleState]::Off) {
        Check ($text -match 'enabled = \[[^\]]*"japanese"') "Japanese is switched on in the file"
    } else {
        Check ($text -match 'enabled = \[' -and $text -notmatch '"japanese"') "Japanese is switched off in the file"
    }
    $names = Names $root
    Check (@($names | Where-Object { $_ -like "Saved*" }).Count -gt 0) "the page says it was saved"
    $picture = Join-Path $Out "settings-en-languages.png"
    $rect = New-Object Win+RECT
    [Win]::GetWindowRect($process.MainWindowHandle, [ref]$rect) | Out-Null
    $bitmap = New-Object System.Drawing.Bitmap ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
    $bitmap.Save($picture)
    $graphics.Dispose()
    $bitmap.Dispose()
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
}
Write-Host "the settings window can be used: pages, and a change saved"
