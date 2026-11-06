$ErrorActionPreference = 'Stop'
$env:CARGO_HOME = Join-Path $PSScriptRoot 'tools/cargo'
$env:RUSTUP_HOME = Join-Path $PSScriptRoot 'tools/rustup'
$env:RUSTUP_INIT_SKIP_PATH_CHECK = 'yes'
New-Item -ItemType Directory -Force (Join-Path $PSScriptRoot 'tools') | Out-Null
$installer = Join-Path $PSScriptRoot 'tools/rustup-init.exe'
if (!(Test-Path $installer)) { Invoke-WebRequest 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe' -OutFile $installer }
& $installer -y --no-modify-path --profile minimal --default-host x86_64-pc-windows-gnullvm --default-toolchain 1.91.0
if ($LASTEXITCODE) { throw 'Local Rust installation failed' }
