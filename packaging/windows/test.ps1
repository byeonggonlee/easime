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
# a unit test there pins the same text): one profile for every language (0xFFFF),
# and the per-language ones earlier builds registered, which must be gone.
$clsid = "{CFC09673-DDBA-4C84-B57F-CF0B0DDC262C}"
$profile = @{ Lang = "ffff"; Profile = "{05C63458-0689-4061-8D1B-D8C77D6A613C}" }
$earlier = @(
    @{ Lang = "0412"; Profile = "{42FBAB7A-5B91-4A6A-868A-3CC5C8153492}" },
    @{ Lang = "0411"; Profile = "{C059F0B3-8110-4CA5-AF4F-B2C79C3BE921}" }
)
$userProfiles = "HKCU:\Control Panel\International\User Profile"

# easime is switched on under the user's first language, so no language is added
# to their list: the entry the install must make, from the list as it is now.
function User-Languages { @((Get-ItemProperty $userProfiles -ErrorAction SilentlyContinue).Languages) }
$firstLanguage = (User-Languages)[0]
$firstLangId = "{0:X4}" -f ([System.Globalization.CultureInfo]::GetCultureInfo($firstLanguage).LCID -band 0xFFFF)
$tip = "$($firstLangId):$clsid$($profile.Profile)"

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
    Write-Host "profile for every language registered: $(Profile-Registered $profile); earlier profiles: $(Earlier-Registered)"
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
    Assert (Profile-Registered $profile) "$what`: the profile for every language is registered"
    Assert (-not (Earlier-Registered)) "$what`: no per-language profile of an earlier build is left"
}
function Assert-Unregistered([string]$what) {
    Assert ($null -eq (Class-Dll)) "$what`: Windows no longer knows the text service"
    Assert (-not (Profile-Registered $profile)) "$what`: the profile is gone from the list"
}

Assert (-not (Test-Path $dir)) "nothing is installed before the test"
Assert ($null -eq (Class-Dll)) "the text service is not registered before the test"
Assert ((User-Tips).Count -eq 0) "no easime input method is switched on for this user before the test"
$languagesAtStart = (User-Languages) -join ","
Write-Host "the user's languages: $languagesAtStart; easime goes under $firstLanguage ($tip)"

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
Assert ((User-Tips) -contains $tip) "easime is switched on under the user's first language ($tip)"
Assert ((User-Tips).Count -eq 1) "and nowhere else"
Assert (((User-Languages) -join ",") -eq $languagesAtStart) "no language was added to the user's list ($languagesAtStart)"

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

# 5. A newer version replaces an older one, and an older one does not replace a newer one.
Assert ((Invoke-Msi "install-old" @("/i", "`"$OldMsi`"")) -eq 0) "the older MSI installs"
$old = (Entries)[0].DisplayVersion
$languagesBefore = (Get-ItemProperty $userProfiles -ErrorAction SilentlyContinue).Languages -join ","
$disablesBefore = Setup-Lines "disable"
Assert ((Invoke-Msi "upgrade" @("/i", "`"$Msi`"")) -eq 0) "the newer MSI installs over it"
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
Assert ((Invoke-Msi "final-uninstall" @("/x", "`"$Msi`"")) -eq 0) "the MSI uninstalls"
Assert (-not (Test-Path $dir)) "the install folder is gone"
Assert-Unregistered "after the final uninstall"
Assert ((User-Tips).Count -eq 0) "easime is switched off again at the end"
Write-Host "all installer checks passed"
