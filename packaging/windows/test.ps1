# Installs the MSI, uses what it installed, upgrades it and removes it again, on
# this machine (the CI's Windows runner). It needs administrator rights and
# changes the machine and the current user's language settings; do not run it
# on a computer you use.
#
#   pwsh packaging\windows\test.ps1 -Msi target\easime-0.1.0-x64.msi -OldMsi target\easime-0.0.1-x64.msi
param(
    [Parameter(Mandatory)][string]$Msi,
    # An older build of the same installer, to try the upgrade.
    [Parameter(Mandatory)][string]$OldMsi,
    # The dictionary the MSI was built with, to compare with what it installs.
    [string]$Dictionary = "target\dictionary\mozc.bin"
)

$ErrorActionPreference = "Stop"
$Dictionary = (Resolve-Path $Dictionary).Path
$dir = Join-Path $env:ProgramFiles "easime"
$logs = Join-Path (Get-Location) "msi-logs"
New-Item -ItemType Directory -Force $logs | Out-Null

# What the text service is called in Windows (crates/frontend-tsf/src/profiles.rs;
# a unit test there pins the same text): one profile under Korean, and the
# per-language ones earlier builds registered, which must be gone.
$clsid = "{CFC09673-DDBA-4C84-B57F-CF0B0DDC262C}"
$profile = @{ Lang = "0412"; Profile = "{05C63458-0689-4061-8D1B-D8C77D6A613C}" }
$earlier = @(
    @{ Lang = "0412"; Profile = "{42FBAB7A-5B91-4A6A-868A-3CC5C8153492}" },
    @{ Lang = "0411"; Profile = "{C059F0B3-8110-4CA5-AF4F-B2C79C3BE921}" }
)
$userProfiles = "HKCU:\Control Panel\International\User Profile"

# easime is switched on under Korean (added to the user's languages if needed).
function User-Languages { @((Get-ItemProperty $userProfiles -ErrorAction SilentlyContinue).Languages) }
$tip = "0412:$clsid$($profile.Profile)"

function Invoke-Msi([string]$what, [string[]]$arguments) {
    $log = Join-Path $logs ("$what.log" -replace '[^\w.-]', '_')
    $process = Start-Process msiexec.exe -Wait -PassThru -ArgumentList (@("/qn", "/norestart", "/l*v", "`"$log`"") + $arguments)
    Write-Host "msiexec $what -> exit code $($process.ExitCode)"
    return $process.ExitCode
}

function Assert([bool]$condition, [string]$message) {
    if (-not $condition) { throw "FAILED: $message" }
    Write-Host "ok: $message"
}

# A program the user is typing in: it has the text service loaded (COM creates it
# from the installed DLL) and keeps running. Returns the process once the DLL is in it.
function Start-Holder {
    $code = "`$t = [type]::GetTypeFromCLSID([guid]'$clsid'); `$o = [Activator]::CreateInstance(`$t); if (`$null -eq `$o) { exit 3 }; Start-Sleep -Seconds 900"
    $holder = Start-Process (Get-Process -Id $PID).Path -PassThru -WindowStyle Hidden -ArgumentList @("-NoProfile", "-Command", "`"$code`"")
    $deadline = (Get-Date).AddSeconds(60)
    do {
        Start-Sleep -Milliseconds 500
        $loaded = -not $holder.HasExited -and @((Get-Process -Id $holder.Id -ErrorAction SilentlyContinue).Modules | Where-Object { $_.ModuleName -eq "easime_tsf.dll" }).Count -gt 0
    } until ($loaded -or $holder.HasExited -or (Get-Date) -gt $deadline)
    Assert $loaded "a program is using easime (process $($holder.Id) has the text service loaded)"
    return $holder
}

# Whether Windows Installer kept away from Restart Manager in the operation that
# wrote this log (it would close the programs using easime).
function Restart-Manager-Off([string]$what) {
    $log = Join-Path $logs ("$what.log" -replace '[^\w.-]', '_')
    @(Get-Content $log | Where-Object { $_ -match "RESTART MANAGER: Disabled" }).Count -gt 0
}

# What the installer log of one operation says about files in use, and what
# waits for the next restart: printed so the docs can say what happens.
function Show-Files-In-Use([string]$what) {
    Write-Host "---- $what`: files in use"
    Get-Content (Join-Path $logs ("$what.log" -replace '[^\w.-]', '_')) |
        Where-Object { $_ -match 'in use|InUse|Scheduling reboot|Config\.Msi' } |
        Select-Object -First 20 | ForEach-Object { Write-Host "  $_" }
    Write-Host "  waiting for the next restart: $(@(Pending-At-Restart | Where-Object { $_ -match 'easime|Config\.Msi' }) -join ' | ')"
    Write-Host "----"
}

# The settings app: installed in its own folder, with a Start menu entry.
$settingsExe = Join-Path $dir "settings\easime-settings.exe"
$startMenu = Join-Path $env:ProgramData "Microsoft\Windows\Start Menu\Programs\easime.lnk"
function Assert-Settings-App([string]$what) {
    Assert (Test-Path $settingsExe) "$what`: the settings app is installed"
    Assert (Test-Path $startMenu) "$what`: the Start menu has easime"
    $target = (New-Object -ComObject WScript.Shell).CreateShortcut($startMenu).TargetPath
    Assert ($target -eq $settingsExe) "$what`: the Start menu entry opens the settings app ($target)"
}

# The installed settings app opens (on the Windows App Runtime installed from
# Microsoft) and finds the installed Japanese dictionary, one folder up next to
# the text service: read through UI Automation, as a screen reader would.
function Assert-Settings-App-Works {
    Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
    $app = Start-Process $settingsExe -ArgumentList "--ui-language=en" -PassThru
    try {
        $deadline = (Get-Date).AddSeconds(90)
        do { Start-Sleep -Milliseconds 500; $app.Refresh() } until ($app.HasExited -or $app.MainWindowHandle -ne 0 -or (Get-Date) -gt $deadline)
        Assert ($app.MainWindowHandle -ne 0) "the installed settings app opens its window (exited: $($app.HasExited))"
        Assert ($app.MainWindowTitle -eq "easime Settings") "its window is 'easime Settings' ('$($app.MainWindowTitle)')"
        Start-Sleep -Seconds 3
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
        $names = @($root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
            ForEach-Object { $_.Current.Name } | Where-Object { $_ })
        $dictionary = @($names | Where-Object { $_ -like "Found (*" -or $_ -like "Not found*" -or $_ -like "*dictionary file*" })
        Write-Host "the settings app says about the dictionary: $($dictionary -join ' | ')"
        Assert (@($names | Where-Object { $_ -like "Found (*" }).Count -gt 0) "the settings app finds the installed Japanese dictionary"
    } finally {
        if (-not $app.HasExited) { $app.CloseMainWindow() | Out-Null; if (-not $app.WaitForExit(15000)) { Stop-Process -Id $app.Id -Force } }
    }
}

# Files Windows will rename or delete at the next restart.
function Pending-At-Restart {
    $key = Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager" -Name PendingFileRenameOperations -ErrorAction SilentlyContinue
    if ($key) { @($key.PendingFileRenameOperations) } else { @() }
}

function Entries {
    @(Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*", "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -eq "easime" })
}

# The folders on the system PATH, without the trailing backslash the installer writes.
function Machine-Path { [Environment]::GetEnvironmentVariable("Path", "Machine") -split ";" | ForEach-Object { $_.TrimEnd("\") } }

# Windows' registration of the text service.
function Class-Dll {
    $key = Get-Item "HKLM:\SOFTWARE\Classes\CLSID\$clsid\InprocServer32" -ErrorAction SilentlyContinue
    if ($key) { $key.GetValue("") } else { $null }
}
# The 32-bit view of the registry (WOW6432Node), where 32-bit programs look for the class.
function Class-Dll32 {
    $key = Get-Item "HKLM:\SOFTWARE\Classes\WOW6432Node\CLSID\$clsid\InprocServer32" -ErrorAction SilentlyContinue
    if ($key) { $key.GetValue("") } else { $null }
}
function Profile-Registered($language) {
    Test-Path "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid\LanguageProfile\0x0000$($language.Lang)\$($language.Profile)"
}
function Earlier-Registered { @($earlier | Where-Object { Profile-Registered $_ }).Count -gt 0 }

# The input methods of easime that the current user has switched on, as the language
# settings keep them (`0409:{class}{profile}` below the language's tag).
function User-Tips {
    @(Get-ChildItem $userProfiles -ErrorAction SilentlyContinue | ForEach-Object {
        $_.GetValueNames() | Where-Object { $_ -like "*$clsid*" }
    })
}

# The languages in this user's language list that the installer also lists easime
# under (decision of 2026-11-09): the ones that name one language of one region
# (en-US); Korean has it anyway. (A bare "en" is listed under en-US too: Windows
# reads it that way. Only what must be there is checked.) As language ids in four hex digits.
function User-Language-Ids {
    @(Get-ChildItem $userProfiles -ErrorAction SilentlyContinue | ForEach-Object {
        try { $culture = [System.Globalization.CultureInfo]::new($_.PSChildName) } catch { return }
        $id = $culture.LCID -band 0xFFFF
        if ($culture.IsNeutralCulture -or $id -eq 0 -or $id -eq 0x1000 -or $id -eq 0x0412) { return }
        "{0:X4}" -f $id
    })
}
# The languages TSF lists easime's profile under (machine-wide registration).
function Listed-Languages {
    $languages = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid\LanguageProfile"
    @(Get-ChildItem $languages -ErrorAction SilentlyContinue |
        Where-Object { Test-Path (Join-Path $_.PSPath $profile.Profile) } |
        ForEach-Object { $_.PSChildName })
}
# How many times the text service was started in a program (easime.log's "start:"):
# TSF does that for a program only when easime is the input method it uses.
function Service-Starts {
    $log = Join-Path $env:LOCALAPPDATA "easime\easime.log"
    if (Test-Path $log) { @(Get-Content $log | Where-Object { $_ -like "*start:*" }).Count } else { 0 }
}

# How many times the installer's helper logged a step ("enable"/"disable").
function Setup-Lines([string]$step) {
    $log = Join-Path $env:LOCALAPPDATA "easime\easime.log"
    if (Test-Path $log) { @(Get-Content $log | Where-Object { $_ -like "*setup: $step *" }).Count } else { 0 }
}

# What to read when a check fails: the user's language list and easime's own log.
function Show-State([string]$when) {
    Write-Host "---- state $when"
    Write-Host "class DLL: $(Class-Dll); 32-bit class DLL: $(Class-Dll32)"
    Write-Host "profile registered: $(Profile-Registered $profile); earlier profiles: $(Earlier-Registered)"
    Write-Host "easime tips of this user: $((User-Tips) -join ', ')"
    Get-ChildItem $userProfiles -ErrorAction SilentlyContinue | ForEach-Object {
        Write-Host "  language $($_.PSChildName): $($_.GetValueNames() -join ', ')"
    }
    foreach ($log in (Join-Path $env:LOCALAPPDATA "easime\easime.log"), (Join-Path $env:TEMP "easime-setup.log")) {
        if (Test-Path $log) { Write-Host "  $log`:"; Get-Content $log | ForEach-Object { Write-Host "    $_" } }
    }
    Write-Host "----"
}

# Everything an install leaves in Windows, and nothing of it afterwards.
function Assert-Registered([string]$what) {
    Assert ((Class-Dll) -eq (Join-Path $dir "easime_tsf.dll")) "$what`: Windows knows the text service, pointing at the installed DLL"
    Assert ((Class-Dll32) -eq (Join-Path $dir "easime_tsf32.dll")) "$what`: 32-bit programs know it too, pointing at the installed 32-bit DLL"
    Assert (Profile-Registered $profile) "$what`: the profile is registered under Korean"
    Assert (-not (Earlier-Registered)) "$what`: no per-language profile of an earlier build is left"
    # Also listed under the user's own languages (so "Add a keyboard" of their language has it).
    $ids = @(User-Language-Ids)
    Assert ($ids.Count -ge 1) "$what`: the user has a language other than Korean to list easime under ($($ids -join ', '))"
    foreach ($id in $ids) {
        Assert ((Listed-Languages) -contains "0x0000$id".ToLower()) "$what`: easime is also listed under the user's language $id"
    }
}
function Assert-Unregistered([string]$what) {
    Assert ($null -eq (Class-Dll)) "$what`: Windows no longer knows the text service"
    Assert ($null -eq (Class-Dll32)) "$what`: nor do 32-bit programs"
    Assert (-not (Profile-Registered $profile)) "$what`: the profile is gone from the list"
    Assert ((Listed-Languages).Count -eq 0) "$what`: easime is listed under no language any more ($((Listed-Languages) -join ', '))"
}

Assert (-not (Test-Path $dir)) "nothing is installed before the test"
Assert ($null -eq (Class-Dll)) "the text service is not registered before the test"
Assert ($null -eq (Class-Dll32)) "nor for 32-bit programs"
Assert ((User-Tips).Count -eq 0) "no easime input method is switched on for this user before the test"
Write-Host "the user's languages: $((User-Languages) -join ","); easime goes under Korean ($tip)"

# The setup window's texts are the ones of packaging/windows/strings.wxl: the "Files in
# Use" window says that Ignore keeps the programs open (the WiX default says to close them).
function Msi-Text([string]$msi, [string]$query) {
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $installer.GetType().InvokeMember("OpenDatabase", "InvokeMethod", $null, $installer, @($msi, 0))
    $view = $database.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $database, @($query))
    $view.GetType().InvokeMember("Execute", "InvokeMethod", $null, $view, $null) | Out-Null
    $record = $view.GetType().InvokeMember("Fetch", "InvokeMethod", $null, $view, $null)
    if ($record) { $record.GetType().InvokeMember("StringData", "GetProperty", $null, $record, @(1)) } else { $null }
}
$filesInUse = Msi-Text $Msi "SELECT Text FROM Control WHERE Dialog_='FilesInUse' AND Control='Text'"
Write-Host "the Files in Use window says: $filesInUse"
Assert ($filesInUse -like "*Ignore*keep them open*") "the Files in Use window tells the user that Ignore keeps the programs open"
Assert ($filesInUse -notlike "*Close these applications*") "and no longer tells them to close the programs"

# 1. A plain install: the program, the text service, and easime switched on for this user.
Assert ((Invoke-Msi "install" @("/i", "`"$Msi`"")) -eq 0) "the MSI installs"
foreach ($file in "easime-cli.exe", "easime_tsf.dll", "easime_tsf32.dll", "LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_LICENSES.md") {
    Assert (Test-Path (Join-Path $dir $file)) "$file is installed"
}
# The Japanese dictionary is next to the DLL (where the text service looks for
# it), the very file that was built, with Mozc's own license files.
$installedDictionary = Join-Path $dir "dictionary\mozc.bin"
Assert (Test-Path $installedDictionary) "the Japanese dictionary is installed next to the text service"
Assert ((Get-FileHash $installedDictionary).Hash -eq (Get-FileHash $Dictionary).Hash) "it is the dictionary that was built ($((Get-Item $installedDictionary).Length) bytes)"
foreach ($file in "mozc-LICENSE.txt", "mozc-dictionary_oss-README.txt") {
    Assert (Test-Path (Join-Path $dir "dictionary\$file")) "the dictionary's $file is installed"
}
Assert ((Entries).Count -eq 1) "Apps & features lists easime once"
Assert (-not ((Machine-Path) -contains $dir)) "the PATH is left alone by default"
Show-State "after the install"
Assert-Registered "after the install"
Assert ((User-Tips) -contains $tip) "easime is switched on under Korean ($tip)"
Assert ((User-Tips).Count -eq 1) "and nowhere else"
# Listed under the user's language is not switched on there, and no program has started
# the text service on its own (the problem of listing it under every language, enabled by
# default). A little time for programs that would.
Start-Sleep -Seconds 8
Assert ((Service-Starts) -eq 0) "no program started the text service because easime is listed under more languages ($(Service-Starts) starts)"
Assert-Settings-App "after the install"
Assert-Settings-App-Works

# What it installed works: the Hangul demo.
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$typed = "gksrmf" | & (Join-Path $dir "easime-cli.exe")
$hangul = [string]::new([char[]](0xD55C, 0xAE00))   # the two syllables of "Hangul", kept out of this file's text
Assert ($typed -eq $hangul) "easime-cli turns gksrmf into $hangul (got '$typed')"

# The installed DLL loads from where it is installed: COM creates the text service
# from the registry entry, without any of the build tree around it. In a process of
# its own, so that the DLL is not held open when the uninstall below wants it gone.
$powershell = (Get-Process -Id $PID).Path
& $powershell -NoProfile -Command "`$t = [type]::GetTypeFromCLSID([guid]'$clsid'); `$o = [Activator]::CreateInstance(`$t); if (`$null -eq `$o) { exit 3 }; exit 0"
Assert ($LASTEXITCODE -eq 0) "COM creates the text service from the installed DLL"

# The same for a 32-bit program: 32-bit PowerShell asks COM for the class, which
# must come from the 32-bit DLL (a 64-bit DLL cannot be loaded into it).
$powershell32 = Join-Path $env:SystemRoot "SysWOW64\WindowsPowerShell\v1.0\powershell.exe"
Assert (Test-Path $powershell32) "32-bit PowerShell is there to stand for a 32-bit program"
& $powershell32 -NoProfile -Command "`$t = [type]::GetTypeFromCLSID([guid]'$clsid'); `$o = [Activator]::CreateInstance(`$t); if (`$null -eq `$o) { exit 3 }; if ([IntPtr]::Size -ne 4) { exit 5 }; if (@([Diagnostics.Process]::GetCurrentProcess().Modules | Where-Object { `$_.ModuleName -eq 'easime_tsf32.dll' }).Count -eq 0) { exit 4 }; exit 0"
Assert ($LASTEXITCODE -eq 0) "a 32-bit program gets the text service from easime_tsf32.dll (exit code $LASTEXITCODE; 3: no object, 4: not the 32-bit DLL, 5: not a 32-bit process)"

# 2. Removing it leaves nothing behind, in the folder, in Windows or in this user's settings.
Assert ((Invoke-Msi "uninstall" @("/x", "`"$Msi`"")) -eq 0) "the MSI uninstalls"
Show-State "after the uninstall"
Assert (-not (Test-Path $dir)) "the install folder is gone"
Assert ((Entries).Count -eq 0) "Apps & features no longer lists easime"
Assert-Unregistered "after the uninstall"
Assert ((User-Tips).Count -eq 0) "easime is switched off again in this user's language settings"
Assert (-not (Test-Path "HKLM:\SOFTWARE\easime\Keyboard")) "the record of the chosen feature is gone"
Assert (-not (Test-Path $startMenu)) "the Start menu entry is gone"

# 3. Without the keyboard feature the text service is still registered (other
# users can add it), but nothing is switched on for this user.
Assert ((Invoke-Msi "install-no-input" @("/i", "`"$Msi`"", "ADDLOCAL=Main")) -eq 0) "the MSI installs without the keyboard feature"
Assert-Registered "without the keyboard feature"
Assert ((User-Tips).Count -eq 0) "nothing is switched on for this user"
Assert (-not (Test-Path (Join-Path $dir "dictionary"))) "without its feature the dictionary is not installed"
Assert ((Invoke-Msi "uninstall-no-input" @("/x", "`"$Msi`"")) -eq 0) "the MSI uninstalls"
Assert-Unregistered "after removing the install without the keyboard feature"

# 4. The PATH entry is opt in, and goes away with the program.
Assert ((Invoke-Msi "install-with-path" @("/i", "`"$Msi`"", "ADDLOCAL=ALL")) -eq 0) "the MSI installs with ADDLOCAL=ALL"
Assert ((Machine-Path) -contains $dir) "the folder is on the system PATH"
Assert ((User-Tips) -contains $tip) "easime is switched on with ADDLOCAL=ALL too"
Assert ((Invoke-Msi "uninstall-with-path" @("/x", "`"$Msi`"")) -eq 0) "the MSI uninstalls again"
Assert (-not ((Machine-Path) -contains $dir)) "the PATH entry is removed"
Assert (-not (Test-Path $dir)) "the install folder is gone"
Assert-Unregistered "after the uninstall with the PATH entry"
Assert ((User-Tips).Count -eq 0) "easime is switched off again"

# 5. A newer version replaces an older one, and an older one does not replace a newer
# one, while a program is using easime: it is never closed (silent installs too);
# the file it uses is replaced, and finally removed, at the next restart.
Assert ((Invoke-Msi "install-old" @("/i", "`"$OldMsi`"")) -eq 0) "the older MSI installs"
$old = (Entries)[0].DisplayVersion
$languagesBefore = (Get-ItemProperty $userProfiles -ErrorAction SilentlyContinue).Languages -join ","
$disablesBefore = Setup-Lines "disable"
$holder = Start-Holder
$second = $null
try {
    $pendingBefore = @(Pending-At-Restart | Where-Object { $_ -match 'Config\.Msi' }).Count
    $upgraded = Invoke-Msi "upgrade" @("/i", "`"$Msi`"")
    Assert ($upgraded -in 0, 3010) "the newer MSI installs over it (exit code $upgraded; 3010 asks for a restart)"
    Assert (Restart-Manager-Off "upgrade") "the upgrade did not use Restart Manager"
    Assert (-not $holder.HasExited) "the program using easime is still running after the silent upgrade"
    # Observed in the CI: the installer moves the DLL the program uses aside
    # (to C:\Config.Msi), installs the new one at once and deletes the old
    # copy at the next restart; the program keeps the copy it loaded.
    Assert (@(Pending-At-Restart | Where-Object { $_ -match 'Config\.Msi' }).Count -gt $pendingBefore) "the copy the program uses is deleted at the next restart"
    Show-Files-In-Use "upgrade"
    Assert ((Setup-Lines "disable") -eq $disablesBefore) "the upgrade does not take the user's input method away in between"
    Assert (((Get-ItemProperty $userProfiles -ErrorAction SilentlyContinue).Languages -join ",") -eq $languagesBefore) "the user's language list keeps its order ($languagesBefore)"
    $entries = Entries
    Assert ($entries.Count -eq 1) "still one entry after the upgrade"
    Assert ($entries[0].DisplayVersion -ne $old) "the version changed from $old to $($entries[0].DisplayVersion)"
    Assert ((Test-Path (Join-Path $dir "easime-cli.exe"))) "the program is still there"
    Show-State "after the upgrade"
    Assert-Registered "after the upgrade"
    Assert ((User-Tips) -contains $tip) "easime is still switched on after the upgrade"
    Assert ((Invoke-Msi "downgrade" @("/i", "`"$OldMsi`"")) -ne 0) "the older MSI is refused"
    Assert ((Entries)[0].DisplayVersion -eq $entries[0].DisplayVersion) "the newer version is still installed"
    Assert-Registered "after the refused downgrade"

    # A program opened after the upgrade uses the new DLL, which the uninstall
    # then has to take away while it is in use.
    $second = Start-Holder
    $pendingBefore = @(Pending-At-Restart | Where-Object { $_ -match 'Config\.Msi|easime' }).Count
    $removed = Invoke-Msi "final-uninstall" @("/x", "`"$Msi`"")
    Assert ($removed -in 0, 3010) "the MSI uninstalls while programs use easime (exit code $removed)"
    Assert (Restart-Manager-Off "final-uninstall") "the uninstall did not use Restart Manager"
    Assert (-not $holder.HasExited -and -not $second.HasExited) "both programs using easime are still running after the silent uninstall"
    Assert-Unregistered "after the final uninstall"
    Assert ((User-Tips).Count -eq 0) "easime is switched off again at the end"
    Show-Files-In-Use "final-uninstall"
    Write-Host "install folder after the uninstall: $(Test-Path $dir)$(if (Test-Path $dir) { '; left in it: ' + ((Get-ChildItem $dir -Recurse | ForEach-Object Name) -join ', ') })"
    Write-Host "the program opened after the upgrade has loaded: $(@((Get-Process -Id $second.Id).Modules | Where-Object { $_.FileName -match 'easime|Config\.Msi' } | ForEach-Object FileName) -join ', ')"
    $dll = Join-Path $dir "easime_tsf.dll"
    Assert ((-not (Test-Path $dll)) -or @(Pending-At-Restart | Where-Object { $_ -like "*easime*" }).Count -gt 0) "the DLL in use is not left behind for good (gone, or deleted at the next restart)"
    Assert (@(Pending-At-Restart | Where-Object { $_ -match 'Config\.Msi|easime' }).Count -gt $pendingBefore) "what the program still uses waits for the next restart"
} finally {
    foreach ($process in $holder, $second) {
        if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    }
}
# What waited for a restart can go now that nothing uses it.
if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
Assert (-not (Test-Path $dir)) "the install folder is gone"
Write-Host "all installer checks passed"
