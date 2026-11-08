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

# How many times the installer's helper logged a step ("enable"/"disable").
function Setup-Lines([string]$step) {
    $log = Join-Path $env:LOCALAPPDATA "easime\easime.log"
    if (Test-Path $log) { @(Get-Content $log | Where-Object { $_ -like "*setup: $step *" }).Count } else { 0 }
}

# What to read when a check fails: the user's language list and easime's own log.
function Show-State([string]$when) {
    Write-Host "---- state $when"
    Write-Host "class DLL: $(Class-Dll)"
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
    Assert (Profile-Registered $profile) "$what`: the profile is registered under Korean"
    Assert (-not (Earlier-Registered)) "$what`: no per-language profile of an earlier build is left"
}
function Assert-Unregistered([string]$what) {
    Assert ($null -eq (Class-Dll)) "$what`: Windows no longer knows the text service"
    Assert (-not (Profile-Registered $profile)) "$what`: the profile is gone from the list"
}

Assert (-not (Test-Path $dir)) "nothing is installed before the test"
Assert ($null -eq (Class-Dll)) "the text service is not registered before the test"
Assert ((User-Tips).Count -eq 0) "no easime input method is switched on for this user before the test"
Write-Host "the user's languages: $((User-Languages) -join ","); easime goes under Korean ($tip)"

# 1. A plain install: the program, the text service, and easime switched on for this user.
Assert ((Invoke-Msi "install" @("/i", "`"$Msi`"")) -eq 0) "the MSI installs"
foreach ($file in "easime-cli.exe", "easime_tsf.dll", "LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_LICENSES.md") {
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

# 2. Removing it leaves nothing behind, in the folder, in Windows or in this user's settings.
Assert ((Invoke-Msi "uninstall" @("/x", "`"$Msi`"")) -eq 0) "the MSI uninstalls"
Show-State "after the uninstall"
Assert (-not (Test-Path $dir)) "the install folder is gone"
Assert ((Entries).Count -eq 0) "Apps & features no longer lists easime"
Assert-Unregistered "after the uninstall"
Assert ((User-Tips).Count -eq 0) "easime is switched off again in this user's language settings"
Assert (-not (Test-Path "HKLM:\SOFTWARE\easime\Keyboard")) "the record of the chosen feature is gone"

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
try {
    $upgraded = Invoke-Msi "upgrade" @("/i", "`"$Msi`"")
    Assert ($upgraded -in 0, 3010) "the newer MSI installs over it (exit code $upgraded; 3010 asks for a restart)"
    Assert (Restart-Manager-Off "upgrade") "the upgrade did not use Restart Manager"
    Assert (-not $holder.HasExited) "the program using easime is still running after the silent upgrade"
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
    $removed = Invoke-Msi "final-uninstall" @("/x", "`"$Msi`"")
    Assert ($removed -in 0, 3010) "the MSI uninstalls while the program uses easime (exit code $removed)"
    Assert (Restart-Manager-Off "final-uninstall") "the uninstall did not use Restart Manager"
    Assert (-not $holder.HasExited) "the program using easime is still running after the silent uninstall"
    Assert-Unregistered "after the final uninstall"
    Assert ((User-Tips).Count -eq 0) "easime is switched off again at the end"
    # What Windows Installer did with the DLL the program still has loaded:
    # printed, so the docs say what happens rather than what was expected.
    Write-Host "---- after the uninstall, with the program still running"
    Write-Host "install folder: $(Test-Path $dir); left in it: $((Get-ChildItem $dir -Recurse -ErrorAction SilentlyContinue | ForEach-Object Name) -join ', ')"
    Write-Host "to do at the next restart: $(@(Pending-At-Restart | Where-Object { $_ -match 'easime|Config\.Msi' }) -join ' | ')"
    $held = @((Get-Process -Id $holder.Id).Modules | Where-Object { $_.FileName -match 'easime|Config\.Msi' } | ForEach-Object FileName)
    Write-Host "the program has loaded: $($held -join ', ')"
    foreach ($what in "upgrade", "final-uninstall") {
        Write-Host "$what log, files in use:"
        Get-Content (Join-Path $logs "$what.log") | Where-Object { $_ -match 'in use|InUse|Config\.Msi|REBOOT|\.rbf' } |
            Select-Object -First 25 | ForEach-Object { Write-Host "  $_" }
    }
    Write-Host "----"
    $dll = Join-Path $dir "easime_tsf.dll"
    if (Test-Path $dll) {
        Assert (@(Pending-At-Restart | Where-Object { $_ -like "*easime*" }).Count -gt 0) "the DLL still in use is removed at the next restart"
    }
} finally {
    if (-not $holder.HasExited) { Stop-Process -Id $holder.Id -Force }
}
# What waited for a restart can go now that nothing uses it.
if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
Assert (-not (Test-Path $dir)) "the install folder is gone"
Write-Host "all installer checks passed"
