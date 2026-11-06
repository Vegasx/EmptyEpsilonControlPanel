param([switch]$SkipTests, [switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
$env:CARGO_HOME = Join-Path $PSScriptRoot 'tools/cargo'
$env:RUSTUP_HOME = Join-Path $PSScriptRoot 'tools/rustup'
$env:RUSTUP_TOOLCHAIN = '1.91.0-x86_64-pc-windows-gnullvm'
$compilerBin = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../native-cpp/tools/llvm-mingw-20260616-ucrt-x86_64/bin'))
$env:EPSILON_WINDRES = Join-Path $compilerBin 'x86_64-w64-mingw32-windres.exe'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER = Join-Path $compilerBin 'x86_64-w64-mingw32-clang.exe'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_RUSTFLAGS = '-C target-feature=+crt-static -D warnings'
$env:PATH = "$compilerBin;$(Join-Path $env:CARGO_HOME 'bin');$env:PATH"
$cargo = Join-Path $env:CARGO_HOME 'bin/cargo.exe'
if (!(Test-Path $cargo)) { throw 'Run bootstrap.ps1 first to download the local Rust compiler.' }
if (!(Test-Path $env:EPSILON_WINDRES)) { & (Join-Path $PSScriptRoot '../native-cpp/bootstrap.ps1') }
Push-Location $PSScriptRoot
try {
    if ($CheckOnly) { & $cargo check --locked --target x86_64-pc-windows-gnullvm; if ($LASTEXITCODE) { throw 'Rust check failed' }; return }
    & $cargo build --locked --release --target x86_64-pc-windows-gnullvm
    if ($LASTEXITCODE) { throw 'Rust build failed' }
    if (!$SkipTests) { & $cargo test --locked --release --target x86_64-pc-windows-gnullvm; if ($LASTEXITCODE) { throw 'Rust tests failed' } }
    $output = Join-Path $PSScriptRoot 'dist/Epsilon-Engineering-Rust-win-x64'
    New-Item -ItemType Directory -Force $output | Out-Null
    Copy-Item 'target/x86_64-pc-windows-gnullvm/release/epsilon-engineering-rust.exe' "$output/Epsilon Engineering Rust.exe" -Force
    foreach ($file in @('config.json','transport.json')) { if (!(Test-Path "$output/$file")) { Copy-Item $file $output } }
    Copy-Item '../FONT-LICENSE.txt','README.md','Cargo.lock' $output -Force
    $metadata = (& $cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-gnullvm | ConvertFrom-Json)
    if ($LASTEXITCODE) { throw 'Cannot read dependency licenses' }
    foreach ($package in $metadata.packages) {
        if ($package.name -eq 'epsilon-engineering-rust') { continue }
        $licenseFolder = Join-Path $output "licenses/$($package.name)-$($package.version)"
        New-Item -ItemType Directory -Force $licenseFolder | Out-Null
        Get-ChildItem -LiteralPath (Split-Path $package.manifest_path) -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|COPYRIGHT)' } | Copy-Item -Destination $licenseFolder -Force
        "$($package.name) $($package.version)`nLicense: $($package.license)`n$($package.repository)" | Set-Content (Join-Path $licenseFolder 'NOTICE.txt')
    }
    $rustDocs = Join-Path $env:RUSTUP_HOME "toolchains/$env:RUSTUP_TOOLCHAIN/share/doc/rust"
    $runtimeLicenses = Join-Path $output 'licenses/rust-runtime'
    New-Item -ItemType Directory -Force $runtimeLicenses | Out-Null
    Copy-Item "$rustDocs/COPYRIGHT-library.html" $runtimeLicenses -Force
    Copy-Item "$rustDocs/licenses" $runtimeLicenses -Recurse -Force
    if (Test-Path 'VERIFICATION.md') { Copy-Item 'VERIFICATION.md' $output -Force }
    if (Test-Path 'artifacts/verification.json') {
        $evidence = Join-Path $output 'artifacts'
        New-Item -ItemType Directory -Force $evidence | Out-Null
        foreach ($name in @('verification.json','ui-verification.json','cpp-visual-comparison.json','portable-smoke.json','rust-panel.png','rust-panel-minimum.png','rust-panel-portrait.png','rust-panel-coolant.png')) {
            if (Test-Path "artifacts/$name") { Copy-Item "artifacts/$name" $evidence -Force }
        }
    }
    Compress-Archive -LiteralPath $output -DestinationPath 'dist/Epsilon-Engineering-Rust-win-x64.zip' -Force
    Write-Output "Built: $output/Epsilon Engineering Rust.exe"
} finally { Pop-Location }
