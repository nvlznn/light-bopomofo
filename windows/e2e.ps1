# End-to-end test of 輕注音 for Windows on a real machine (CI), through the
# real installer: install silently, check registration, type through the
# input method in 64- and 32-bit processes (examples/e2e.rs), update while
# the input method is loaded, uninstall. Needs admin and windows\build.ps1
# to have produced dist\LightBopomofo-Setup.exe.
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."
Set-Location $root
$setup = "$root\dist\LightBopomofo-Setup.exe"
$dir = "$env:ProgramFiles\LightBopomofo"
$clsid = "{0D5E0D6E-3056-4888-9151-8701B588AD0C}"
$profile = "{31638591-41D6-4DC9-B7D8-C2F29953F24B}"
$tip = "0404:$clsid$profile"
$history = "$env:APPDATA\LightBopomofo\history.tsv"
$script:failures = @()

function Check($name, $condition, $detail = "") {
    if ($condition) { Write-Host "PASS $name" }
    else { Write-Host "FAIL $name $detail"; $script:failures += $name }
}

function Wait-Exit($file, [string[]]$arguments) {
    (Start-Process $file -ArgumentList $arguments -Wait -PassThru).ExitCode
}

function In-UserList {
    $null -ne (Get-ChildItem "HKCU:\Control Panel\International\User Profile" -ErrorAction SilentlyContinue |
        Where-Object { (Get-ItemProperty $_.PSPath).PSObject.Properties.Name -contains $tip })
}

function Server-Path($view) {
    $key = [Microsoft.Win32.RegistryKey]::OpenBaseKey("ClassesRoot", $view).OpenSubKey("CLSID\$clsid\InprocServer32")
    if ($key) { $key.GetValue("") } else { $null }
}

function Run-Typing($triple, $label) {
    $exe = "target\$triple\release\examples\e2e.exe"
    $output = & $exe
    $code = $LASTEXITCODE
    $output | ForEach-Object { Write-Host "  [$label] $_" }
    Check "typing in a $label process" ($code -eq 0) "($code scenario(s) failed)"
}

# Key-level tracing in the input method's log (only when this is set).
$env:LIGHTBOPOMOFO_DEBUG = "1"

Write-Host "== build the test program (64- and 32-bit)"
foreach ($t in "x86_64-pc-windows-msvc", "i686-pc-windows-msvc") {
    cargo build --release -p lightbopomofo-windows --example e2e --target $t
    if ($LASTEXITCODE -ne 0) { throw "building e2e for $t failed" }
}

Write-Host "== install"
$code = Wait-Exit $setup @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=`"$env:TEMP\setup-install.log`"")
Check "installer exit code" ($code -eq 0) "($code)"
foreach ($f in "lexicon.bin", "LightBopomofo.exe", "lightbopomofo.ico", "x64\lightbopomofo.dll", "x86\lightbopomofo.dll", "licenses\tsi.csv", "licenses\LGPL-2.1.txt") {
    Check "installed $f" (Test-Path "$dir\$f")
}
Check "64-bit COM server" ((Server-Path "Registry64") -eq "$dir\x64\lightbopomofo.dll") "($(Server-Path 'Registry64'))"
Check "32-bit COM server" ((Server-Path "Registry32") -eq "$dir\x86\lightbopomofo.dll") "($(Server-Path 'Registry32'))"
Check "TSF language profile" (Test-Path "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid\LanguageProfile\0x00000404\$profile")
Check "in the user's input methods" (In-UserList)
Check "Start menu shortcut" (Test-Path "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\輕注音.lnk")
$code = Wait-Exit "$dir\LightBopomofo.exe" @("--status")
Check "app reports ready" ($code -eq 0) "(status $code)"

Write-Host "== updates"
$task = "LightBopomofo\UpdateCheck"
Check "update check scheduled" ((Wait-Exit "schtasks.exe" @("/Query", "/TN", $task)) -eq 0)
Check "update link registered" (Test-Path "Registry::HKEY_CLASSES_ROOT\lightbopomofo\shell\open\command")
# 0 up to date, 10 newer release, 1 could not ask GitHub.
$code = Wait-Exit "$dir\LightBopomofo.exe" @("--check-update", "--now")
Check "update check reaches GitHub" (($code -eq 0) -or ($code -eq 10)) "(exit $code)"
$code = Wait-Exit "$dir\LightBopomofo.exe" @("--notify-test", "2.0")
Check "update notification shown" ($code -eq 0) "(exit $code)"
# The app offering an update (as if 2.0 were out).
$env:LIGHTBOPOMOFO_PRETEND_LATEST = "2.0"
$shown = Start-Process "$dir\LightBopomofo.exe" -PassThru
Start-Sleep -Seconds 6
$shown.Refresh()
Check "app stays open" (-not $shown.HasExited) "(exit $(if ($shown.HasExited) { $shown.ExitCode }))"
$appLog = "$env:LOCALAPPDATA\LightBopomofo\logs\app.log"
if (Test-Path $appLog) { Get-Content $appLog -Tail 10 | ForEach-Object { Write-Host "  app.log: $_" } }
Stop-Process -Id $shown.Id -Force -ErrorAction SilentlyContinue
Remove-Item Env:\LIGHTBOPOMOFO_PRETEND_LATEST
# The scheduled task runs the check as the signed-in user.
Remove-ItemProperty "HKCU:\Software\LightBopomofo" -Name LastUpdateCheck -ErrorAction SilentlyContinue
$null = Wait-Exit "schtasks.exe" @("/Run", "/TN", $task)
Start-Sleep -Seconds 20
Check "scheduled update check ran" ($null -ne (Get-ItemProperty "HKCU:\Software\LightBopomofo" -ErrorAction SilentlyContinue).LastUpdateCheck)

Write-Host "== type"
Run-Typing "x86_64-pc-windows-msvc" "64-bit"
Run-Typing "i686-pc-windows-msvc" "32-bit"
Check "history written" ((Test-Path $history) -and (Get-Item $history).Length -gt 0)

Write-Host "== a real TSF-aware application (Microsoft Edge)"
$edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") |
    Where-Object { Test-Path $_ } | Select-Object -First 1
if ($edge) {
    & "target\x86_64-pc-windows-msvc\release\examples\e2e.exe" --activate-session | ForEach-Object { Write-Host "  $_" }
    Add-Type -AssemblyName System.Windows.Forms, UIAutomationClient, UIAutomationTypes
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Fg {
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
"@
    $page = "$env:TEMP\lb-textarea.html"
    Set-Content -Encoding UTF8 $page '<!doctype html><meta charset="utf-8"><title>lb</title><textarea id="t" autofocus style="width:600px;height:200px"></textarea>'
    $cases = @(
        @{ name = "ㄋㄏㄇ → 你好嗎"; keys = @("s", "c", "a", " "); want = "你好嗎" },
        @{ name = "ㄍㄗㄈㄈ → 工作方法"; keys = @("e", "y", "z", "z", " "); want = "工作方法" },
        @{ name = "choose 工作 then 方法"; keys = @("e", "y", "z", "z", "{RIGHT}", "{RIGHT}", "{RIGHT}", "{RIGHT}", " ", " "); want = "工作方法" }
    )
    foreach ($case in $cases) {
        $dataDir = "$env:TEMP\lb-edge-$([guid]::NewGuid())"
        $null = Start-Process $edge -ArgumentList "--new-window", "--no-first-run", "--no-default-browser-check", "--disable-features=msEdgeSidebar", "--user-data-dir=`"$dataDir`"", "file:///$($page -replace '\\','/')" -PassThru
        Start-Sleep -Seconds 8
        $win = Get-Process msedge -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like "lb*" } | Select-Object -First 1
        if (-not $win) { Check "Edge: $($case.name)" $false "(Edge window not found)"; continue }
        [Fg]::SetForegroundWindow($win.MainWindowHandle) | Out-Null
        Start-Sleep -Milliseconds 800
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($win.MainWindowHandle)
        $isEdit = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)
        # Put the keyboard focus in the page's textarea (not the address bar).
        $textarea = $null
        foreach ($e in $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $isEdit)) {
            try { if ($e.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -notlike "*lb-textarea*") { $textarea = $e } } catch {}
        }
        if ($textarea) { try { $textarea.SetFocus() } catch { Write-Host "NOTE could not focus the textarea: $_" } } else { Write-Host "NOTE textarea not found" }
        Start-Sleep -Milliseconds 500
        Write-Host "NOTE Edge in the foreground: $([Fg]::GetForegroundWindow() -eq $win.MainWindowHandle)"
        foreach ($k in $case.keys) { [System.Windows.Forms.SendKeys]::SendWait($k); Start-Sleep -Milliseconds 200 }
        Start-Sleep -Seconds 1
        $values = @()
        foreach ($e in $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $isEdit)) {
            try { $values += $e.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value } catch {}
        }
        $got = $values | Where-Object { $_ -notlike "file:*" -and $_ -notlike "*lb-textarea*" }
        Check "Edge: $($case.name)" ($got -contains $case.want) "(edit values: '$($values -join "' | '")')"
        Get-Process msedge -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2
    }
    $log = "$env:LOCALAPPDATA\LightBopomofo\logs\ime.log"
    Check "Edge loaded 輕注音" ((Test-Path $log) -and (Select-String -Path $log -Pattern "msedge.exe\] activated" -Quiet))
} else {
    Write-Host "NOTE Microsoft Edge is not installed on this machine"
}

Write-Host "== update while in use"
$holder = Start-Process "target\x86_64-pc-windows-msvc\release\examples\e2e.exe" -ArgumentList "--hold", "180" -PassThru
# The app is open too, as when it updates itself.
$app = Start-Process "$dir\LightBopomofo.exe" -PassThru
Start-Sleep -Seconds 8
$app.Refresh()
Write-Host "NOTE app running before the update: $(-not $app.HasExited)"
$code = Wait-Exit $setup @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=`"$env:TEMP\setup-update.log`"")
Check "update exit code" ($code -eq 0) "($code)"
Check "loaded DLL moved aside, not left for a reboot" ((Get-ChildItem "$dir\x64" -Filter *.old).Count -ge 1)
Check "new DLL in place" (Test-Path "$dir\x64\lightbopomofo.dll")
Check "running app moved aside" ((Get-ChildItem $dir -Filter "LightBopomofo.*.old").Count -ge 1)
Check "new app in place" (Test-Path "$dir\LightBopomofo.exe")
Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
Run-Typing "x86_64-pc-windows-msvc" "64-bit, after update"
Stop-Process -Id $holder.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

Write-Host "== uninstall"
$code = Wait-Exit "$dir\unins000.exe" @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=`"$env:TEMP\setup-uninstall.log`"")
Start-Sleep -Seconds 5
Check "uninstaller exit code" ($code -eq 0) "($code)"
Check "COM class removed" ($null -eq (Server-Path "Registry64"))
Check "32-bit COM class removed" ($null -eq (Server-Path "Registry32"))
Check "removed from the user's input methods" (-not (In-UserList))
Check "DLLs removed" ((-not (Test-Path "$dir\x64\lightbopomofo.dll")) -and (-not (Test-Path "$dir\x86\lightbopomofo.dll")))
Check "history kept" (Test-Path $history)
Check "update check unscheduled" ((Wait-Exit "schtasks.exe" @("/Query", "/TN", $task)) -ne 0)
Check "update link removed" (-not (Test-Path "Registry::HKEY_CLASSES_ROOT\lightbopomofo"))
# Files still in use (an application kept the old DLL loaded) are queued for
# deletion at restart rather than left behind for good.
$left = Get-ChildItem $dir -Recurse -File -ErrorAction SilentlyContinue
if ($left) {
    $pending = (Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager" -ErrorAction SilentlyContinue).PendingFileRenameOperations -join "`n"
    foreach ($f in $left) {
        Check "leftover $($f.Name) deleted at restart" ($pending -like "*$($f.Name)*")
    }
} else {
    Check "install folder emptied" $true
}

Write-Host "== logs"
$log = "$env:LOCALAPPDATA\LightBopomofo\logs\ime.log"
if (Test-Path $log) {
    Write-Host "---- ime.log (non-debug)"; Get-Content $log | Where-Object { $_ -notmatch " DEBUG " } | Select-Object -Last 40
    Write-Host "---- ime.log debug lines by process (from just before the last control's start, 40 each)"
    Get-Content $log | Where-Object { $_ -match " (DEBUG|MARK) " } | Group-Object { ($_ -split " ")[2] } |
        ForEach-Object {
            $lines = @($_.Group)
            $last = ($lines | Select-String " MARK .* start " | Select-Object -Last 1)
            $from = if ($last) { [Math]::Max(0, $last.LineNumber - 7) } else { [Math]::Max(0, $lines.Count - 40) }
            Write-Host "-- $($_.Name)"
            $lines[$from..([Math]::Min($lines.Count - 1, $from + 39))] | ForEach-Object { Write-Host $_ }
        }
} else { Write-Host "no ime.log" }
if ($script:failures.Count -gt 0) {
    foreach ($l in "install", "update", "uninstall") {
        $f = "$env:TEMP\setup-$l.log"
        if (Test-Path $f) { Write-Host "---- setup-$l.log (tail)"; Get-Content $f -Tail 25 }
    }
    throw "e2e failed: $($script:failures -join ', ')"
}
Write-Host "e2e: all passed"
