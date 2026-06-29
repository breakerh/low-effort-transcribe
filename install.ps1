$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ScriptDir

# 1. Rust toolchain
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust toolchain niet gevonden. Installeren via rustup..."
    $RustupExe = Join-Path $env:TEMP "rustup-init.exe"
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $RustupExe
    & $RustupExe -y --default-toolchain stable
    if ($LASTEXITCODE -ne 0) { throw "rustup install failed" }
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
}

# 2. Build dependencies
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    Write-Host "cmake niet gevonden."
    Write-Host "Installeer via 'winget install Kitware.CMake' of https://cmake.org/download/"
    Write-Host "Daarna ook Visual Studio Build Tools (C++ workload) nodig: https://aka.ms/vs/17/release/vs_BuildTools.exe"
    exit 1
}

# 3. Build (CLI + GUI)
Write-Host ""
Write-Host "Building (eerste keer 5-15 min, daarna seconden)..."
cargo build --release --features gui
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

# 4. Install both binaries
$InstallDir = if ($env:INSTALL_DIR) { $env:INSTALL_DIR } else { "$env:USERPROFILE\.local\bin" }
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

$SrcCli = Join-Path $ScriptDir "target\release\transcribe.exe"
$DstCli = Join-Path $InstallDir "transcribe.exe"
Copy-Item -Path $SrcCli -Destination $DstCli -Force

$SrcGui = Join-Path $ScriptDir "target\release\transcribe-gui.exe"
$DstGui = Join-Path $InstallDir "transcribe-gui.exe"
Copy-Item -Path $SrcGui -Destination $DstGui -Force

Write-Host ""
Write-Host "Geinstalleerd:"
Write-Host "  $DstCli   (CLI)"
Write-Host "  $DstGui   (GUI venster)"
Write-Host ""

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$OnPath = $UserPath -split ";" | Where-Object { $_.TrimEnd("\") -eq $InstallDir.TrimEnd("\") }

if ($OnPath) {
    Write-Host "Run:"
    Write-Host "  transcribe [path]"
} else {
    Write-Host "$InstallDir staat niet in je User PATH. Opties:"
    Write-Host ""
    Write-Host "  1) Voeg permanent toe (huidige PowerShell, herstart shell daarna):"
    Write-Host "     [Environment]::SetEnvironmentVariable('Path', `"`$([Environment]::GetEnvironmentVariable('Path','User'));$InstallDir`", 'User')"
    Write-Host ""
    Write-Host "  2) Of run direct met volledig pad:"
    Write-Host "     `"$DstExe`" [path]"
}
