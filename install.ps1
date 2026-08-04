# Installs coders on Windows. Uses the prebuilt dist\windows-x86_64\coders.exe
# when present; otherwise falls back to `cargo build` (requires Rust:
# https://rustup.rs).
$ErrorActionPreference = "Stop"

$RepoDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BinName = "coders.exe"
$InstallDir = if ($env:CODERS_INSTALL_DIR) { $env:CODERS_INSTALL_DIR } else { Join-Path $env:USERPROFILE ".local\bin" }
$Prebuilt = Join-Path $RepoDir "dist\windows-x86_64\$BinName"

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

if (Test-Path $Prebuilt) {
    Write-Host "Using prebuilt binary for windows-x86_64"
    Copy-Item $Prebuilt (Join-Path $InstallDir $BinName) -Force
} else {
    Write-Host "No prebuilt binary found, building from source..."
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Error "cargo/rustc not found. Install Rust first: https://rustup.rs"
        exit 1
    }
    cargo build --release --manifest-path (Join-Path $RepoDir "Cargo.toml") -p coders-cli
    Copy-Item (Join-Path $RepoDir "target\release\$BinName") (Join-Path $InstallDir $BinName) -Force
}

Write-Host "Installed to $InstallDir\$BinName"

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to your user PATH..."
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
    Write-Host "Restart your terminal for the PATH change to take effect."
}

& (Join-Path $InstallDir $BinName) init
