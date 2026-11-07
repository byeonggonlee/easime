# Installs the MSI, uses what it installed, upgrades it and removes it again, on
# this machine (the CI's Windows runner). It needs administrator rights and
# changes the machine; do not run it on a computer you use.
#
#   pwsh packaging\windows\test.ps1 -Msi target\easime-0.1.0-x64.msi -OldMsi target\easime-0.0.1-x64.msi
param(
    [Parameter(Mandatory)][string]$Msi,
    # An older build of the same installer, to try the upgrade.
    [Parameter(Mandatory)][string]$OldMsi
)

$ErrorActionPreference = "Stop"
$dir = Join-Path $env:ProgramFiles "easime"
$logs = Join-Path (Get-Location) "msi-logs"
New-Item -ItemType Directory -Force $logs | Out-Null

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

function Machine-Path { [Environment]::GetEnvironmentVariable("Path", "Machine") -split ";" }

Assert (-not (Test-Path $dir)) "nothing is installed before the test"

# 1. A plain install.
Assert ((Invoke-Msi "install" @("/i", "`"$Msi`"")) -eq 0) "the MSI installs"
foreach ($file in "easime-cli.exe", "LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_LICENSES.md") {
    Assert (Test-Path (Join-Path $dir $file)) "$file is installed"
}
Assert ((Entries).Count -eq 1) "Apps & features lists easime once"
Assert (-not ((Machine-Path) -contains $dir)) "the PATH is left alone by default"

# What it installed works: the Hangul demo.
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$typed = "gksrmf" | & (Join-Path $dir "easime-cli.exe")
$hangul = [string]::new([char[]](0xD55C, 0xAE00))   # the two syllables of "Hangul", kept out of this file's text
Assert ($typed -eq $hangul) "easime-cli turns gksrmf into $hangul (got '$typed')"

# 2. Removing it leaves nothing behind.
Assert ((Invoke-Msi "uninstall" @("/x", "`"$Msi`"")) -eq 0) "the MSI uninstalls"
Assert (-not (Test-Path $dir)) "the install folder is gone"
Assert ((Entries).Count -eq 0) "Apps & features no longer lists easime"

# 3. The PATH entry is opt in, and goes away with the program.
Assert ((Invoke-Msi "install-with-path" @("/i", "`"$Msi`"", "ADDLOCAL=ALL")) -eq 0) "the MSI installs with ADDLOCAL=ALL"
Assert ((Machine-Path) -contains $dir) "the folder is on the system PATH"
Assert ((Invoke-Msi "uninstall-with-path" @("/x", "`"$Msi`"")) -eq 0) "the MSI uninstalls again"
Assert (-not ((Machine-Path) -contains $dir)) "the PATH entry is removed"
Assert (-not (Test-Path $dir)) "the install folder is gone"

# 4. A newer version replaces an older one, and an older one does not replace a newer one.
Assert ((Invoke-Msi "install-old" @("/i", "`"$OldMsi`"")) -eq 0) "the older MSI installs"
$old = (Entries)[0].DisplayVersion
Assert ((Invoke-Msi "upgrade" @("/i", "`"$Msi`"")) -eq 0) "the newer MSI installs over it"
$entries = Entries
Assert ($entries.Count -eq 1) "still one entry after the upgrade"
Assert ($entries[0].DisplayVersion -ne $old) "the version changed from $old to $($entries[0].DisplayVersion)"
Assert ((Test-Path (Join-Path $dir "easime-cli.exe"))) "the program is still there"
Assert ((Invoke-Msi "downgrade" @("/i", "`"$OldMsi`"")) -ne 0) "the older MSI is refused"
Assert ((Entries)[0].DisplayVersion -eq $entries[0].DisplayVersion) "the newer version is still installed"
Assert ((Invoke-Msi "final-uninstall" @("/x", "`"$Msi`"")) -eq 0) "the MSI uninstalls"
Assert (-not (Test-Path $dir)) "the install folder is gone"
Write-Host "all installer checks passed"
