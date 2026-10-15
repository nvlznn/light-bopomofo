# Build dist\LightBopomofo-Setup.exe: the input method DLL for x64, x86 and
# Arm64, the app for x64 and Arm64, the compiled lexicon, and the installer.
# Used by CI and on a Windows PC with Rust (MSVC) installed:
#   powershell -ExecutionPolicy Bypass -File windows\build.ps1
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\.."
Set-Location $root

function Run($exe, [string[]]$arguments) {
    & $exe @arguments
    if ($LASTEXITCODE -ne 0) { throw "$exe $arguments failed ($LASTEXITCODE)" }
}

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "輕注音 $version"

# Lexicon (data\tsi.csv from scripts/fetch-lexicon.sh).
if (-not (Test-Path data\tsi.csv)) {
    New-Item -ItemType Directory -Force data | Out-Null
    Invoke-WebRequest -UseBasicParsing -OutFile data\tsi.csv `
        "https://raw.githubusercontent.com/chewing/libchewing-data/ea74f76dd2548b1d65d0ff70c3ae66057a6ad97d/dict/chewing/tsi.csv"
}
Run cargo @("build", "--release", "-p", "build-lexicon")

$stage = "target\windows-stage"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage\licenses | Out-Null
Run target\release\build-lexicon.exe @("data\tsi.csv", "$stage\lexicon.bin")

$targets = @{ "x64" = "x86_64-pc-windows-msvc"; "x86" = "i686-pc-windows-msvc"; "arm64" = "aarch64-pc-windows-msvc" }
foreach ($arch in $targets.Keys) {
    $triple = $targets[$arch]
    Run rustup @("target", "add", $triple)
    Run cargo @("build", "--release", "-p", "lightbopomofo-windows", "--target", $triple)
    New-Item -ItemType Directory -Force "$stage\$arch" | Out-Null
    Copy-Item "target\$triple\release\lightbopomofo.dll" "$stage\$arch\"
    if ($arch -ne "x86") {
        Run cargo @("build", "--release", "-p", "lightbopomofo-settings", "--target", $triple)
        Copy-Item "target\$triple\release\LightBopomofo.exe" "$stage\$arch\"
    }
}

Copy-Item windows\lightbopomofo.ico $stage\
Copy-Item LICENSE "$stage\licenses\LICENSE.txt"
Copy-Item DATA_LICENSES.md, licenses\LGPL-2.1.txt, data\tsi.csv "$stage\licenses\"

# Traditional Chinese installer messages (Inno Setup's translation); English if unavailable.
$isl = "windows\ChineseTraditional.isl"
if (-not (Test-Path $isl)) {
    try {
        Invoke-WebRequest -UseBasicParsing -OutFile $isl `
            "https://raw.githubusercontent.com/jrsoftware/issrc/main/Files/Languages/Unofficial/ChineseTraditional.isl"
    } catch {
        Write-Warning "Traditional Chinese installer messages unavailable; using English."
    }
}

$iscc = @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe") |
    Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) {
    Run choco @("install", "innosetup", "-y", "--no-progress")
    $iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
}
Run $iscc @("/Qp", "/DAppVersion=$version", "/DStage=$root\$stage", "/DOutput=$root\dist", "windows\installer.iss")
Get-Item dist\LightBopomofo-Setup.exe | Format-List Name, Length
