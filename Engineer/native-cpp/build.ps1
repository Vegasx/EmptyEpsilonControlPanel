param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$compilerBin = Join-Path $PSScriptRoot 'tools\llvm-mingw-20260616-ucrt-x86_64\bin'
if (!(Test-Path -LiteralPath (Join-Path $compilerBin 'clang++.exe'))) { & (Join-Path $PSScriptRoot 'bootstrap.ps1') }
$output = Join-Path $PSScriptRoot 'dist\Epsilon-Engineering-Cpp-win-x64'
$build = Join-Path $PSScriptRoot 'build'
New-Item -ItemType Directory -Force $output, $build | Out-Null
Push-Location $PSScriptRoot
try {
    & "$compilerBin\x86_64-w64-mingw32-windres.exe" app.rc -O coff -o "$build\app.res"
    if ($LASTEXITCODE) { throw 'Resource compilation failed.' }
    $flags = @('-std=c++20', '-O2', '-DNDEBUG', '-Wall', '-Wextra', '-Wpedantic', '-Werror', '-static', '-D_WIN32_WINNT=0x0A00', '-DWINVER=0x0A00')
    & "$compilerBin\clang++.exe" @flags -municode -mwindows src/main.cpp "$build\app.res" -o "$output\Epsilon Engineering Cpp.exe" -ld2d1 -ldwrite -lwindowscodecs -lwinhttp -lole32 -lshell32 -lpsapi -luuid
    if ($LASTEXITCODE) { throw 'C++ application build failed.' }
    Copy-Item -LiteralPath '..\config.json' -Destination "$output\config.json" -Force
    # Preserve an existing user's connection settings across rebuilds.
    if (!(Test-Path -LiteralPath "$output\transport.json")) { Copy-Item -LiteralPath 'transport.json' -Destination "$output\transport.json" }
    Copy-Item -LiteralPath '..\FONT-LICENSE.txt', 'README.md' -Destination $output -Force
    Copy-Item -LiteralPath (Join-Path $compilerBin '..\LICENSE.TXT') -Destination (Join-Path $output 'LLVM-LICENSE.txt') -Force
    $notices = Join-Path $output 'licenses'
    New-Item -ItemType Directory -Force $notices | Out-Null
    Copy-Item -Path (Join-Path $compilerBin '..\x86_64-w64-mingw32\share\mingw32\COPYING*') -Destination $notices -Force
    if (Test-Path -LiteralPath 'PERFORMANCE.md') { Copy-Item -LiteralPath 'PERFORMANCE.md' -Destination $output -Force }
    if (Test-Path -LiteralPath 'GPU-PERFORMANCE.md') { Copy-Item -LiteralPath 'GPU-PERFORMANCE.md' -Destination $output -Force }
    if (Test-Path -LiteralPath 'artifacts') { Copy-Item -LiteralPath 'artifacts' -Destination $output -Recurse -Force }
    if (!$SkipTests) {
        & "$compilerBin\clang++.exe" @flags src/tests.cpp -o "$build\core-tests.exe"
        if ($LASTEXITCODE) { throw 'Core test build failed.' }
        & "$build\core-tests.exe" '..\config.json'
        if ($LASTEXITCODE) { throw 'Core tests failed.' }
    }
    Compress-Archive -LiteralPath $output -DestinationPath (Join-Path $PSScriptRoot 'dist\Epsilon-Engineering-Cpp-win-x64.zip') -Force
    Write-Output "Built: $output\Epsilon Engineering Cpp.exe"
} finally { Pop-Location }
