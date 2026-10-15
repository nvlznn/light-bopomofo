# End-to-end check of the Windows input method on a real Windows machine
# (CI): install the freshly built DLLs like the installer does, add 輕注音
# to the user's input methods, then type through it in a test window
# (examples/e2e.rs) and read back what arrived.
# Expects windows\build.ps1 to have run (target\windows-stage). Needs admin.
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."
Set-Location $root
$stage = "target\windows-stage"
$dir = "$env:ProgramFiles\LightBopomofo"
$tip = "0404:{0D5E0D6E-3056-4888-9151-8701B588AD0C}{31638591-41D6-4DC9-B7D8-C2F29953F24B}"
$failures = @()

function Check($name, $condition, $detail) {
    if ($condition) { Write-Host "PASS $name" }
    else { Write-Host "FAIL $name $detail"; $script:failures += $name }
}

# 1. Install like the installer does.
New-Item -ItemType Directory -Force "$dir\x64", "$dir\x86" | Out-Null
Copy-Item "$stage\lexicon.bin", "$stage\lightbopomofo.ico" $dir
Copy-Item "$stage\x64\lightbopomofo.dll", "$stage\x64\LightBopomofo.exe" "$dir\x64"
Copy-Item "$stage\x64\LightBopomofo.exe" $dir
Copy-Item "$stage\x86\lightbopomofo.dll" "$dir\x86"
$r64 = Start-Process "$env:windir\System32\regsvr32.exe" -ArgumentList "/s", "`"$dir\x64\lightbopomofo.dll`"" -Wait -PassThru
$r32 = Start-Process "$env:windir\SysWOW64\regsvr32.exe" -ArgumentList "/s", "`"$dir\x86\lightbopomofo.dll`"" -Wait -PassThru
Check "register x64" ($r64.ExitCode -eq 0) "exit $($r64.ExitCode)"
Check "register x86" ($r32.ExitCode -eq 0) "exit $($r32.ExitCode)"
Check "COM class registered" (Test-Path "Registry::HKEY_CLASSES_ROOT\CLSID\{0D5E0D6E-3056-4888-9151-8701B588AD0C}\InprocServer32") ""
Check "TSF profile registered" (Test-Path "HKLM:\SOFTWARE\Microsoft\CTF\TIP\{0D5E0D6E-3056-4888-9151-8701B588AD0C}") ""

# 2. Add it to the user's input methods (the app's --enable), make it default.
$enable = Start-Process "$dir\LightBopomofo.exe" -ArgumentList "--enable" -Wait -PassThru
Check "app --enable" ($enable.ExitCode -eq 0) "exit $($enable.ExitCode)"
$listed = Get-ChildItem "HKCU:\Control Panel\International\User Profile" -ErrorAction SilentlyContinue |
    Where-Object { (Get-ItemProperty $_.PSPath).PSObject.Properties.Name -contains $tip }
Check "in user's input methods" ($null -ne $listed) ""

# 3. Activate 輕注音 in a test window, type with real key presses, read back
#    (crates/lightbopomofo-windows/examples/e2e.rs; built by this script).
cargo build --release -p lightbopomofo-windows --example e2e --target x86_64-pc-windows-msvc
$e2e = "target\x86_64-pc-windows-msvc\release\examples\e2e.exe"
$output = & $e2e
$output | ForEach-Object { Write-Host $_ }
if ($LASTEXITCODE -ne 0) { $failures += "typing ($LASTEXITCODE failed)" }

# 4. The log, whatever happened.
$log = "$env:LOCALAPPDATA\LightBopomofo\logs\ime.log"
if (Test-Path $log) { Write-Host "---- ime.log"; Get-Content $log -Tail 40 } else { Write-Host "no ime.log" }

if ($failures.Count -gt 0) { throw "e2e failed: $($failures -join ', ')" }
Write-Host "e2e: all passed"
