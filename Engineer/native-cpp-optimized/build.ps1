param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$compilerBin = Join-Path $PSScriptRoot '..\native-cpp\tools\llvm-mingw-20260616-ucrt-x86_64\bin'
if (!(Test-Path -LiteralPath (Join-Path $compilerBin 'clang++.exe'))) { & (Join-Path $PSScriptRoot '..\native-cpp\bootstrap.ps1') }
$output = Join-Path $PSScriptRoot 'dist\Epsilon-Engineering-Cpp-Optimized-win-x64'
$build = Join-Path $PSScriptRoot 'build'
New-Item -ItemType Directory -Force $output, $build | Out-Null
Push-Location $PSScriptRoot
try {
    & "$compilerBin\x86_64-w64-mingw32-windres.exe" app.rc -O coff -o "$build\app.res"
    if ($LASTEXITCODE) { throw 'Resource compilation failed.' }
    $flags = @('-std=c++20','-O2','-DNDEBUG','-Wall','-Wextra','-Wpedantic','-Werror','-static','-D_WIN32_WINNT=0x0A00','-DWINVER=0x0A00')
    & "$compilerBin\clang++.exe" @flags -municode -mwindows src/main.cpp "$build\app.res" -o "$output\Epsilon Engineering Cpp Optimized.exe" -ld2d1 -ldwrite -ldxgi -lwindowscodecs -lwinhttp -lole32 -lshell32 -lpsapi -luuid
    if ($LASTEXITCODE) { throw 'Optimized C++ application build failed.' }
    foreach ($file in @('config.json','transport.json')) { if (!(Test-Path -LiteralPath "$output\$file")) { Copy-Item -LiteralPath $file -Destination "$output\$file" } }
    Copy-Item -LiteralPath '..\FONT-LICENSE.txt','README.md','baseline-hashes.json' -Destination $output -Force
    if (Test-Path -LiteralPath 'VERIFICATION.md') { Copy-Item -LiteralPath 'VERIFICATION.md' -Destination $output -Force }
    if (Test-Path -LiteralPath 'artifacts\verification.json') { Copy-Item -LiteralPath 'artifacts' -Destination $output -Recurse -Force }
    Copy-Item -LiteralPath (Join-Path $compilerBin '..\LICENSE.TXT') -Destination "$output\LLVM-LICENSE.txt" -Force
    New-Item -ItemType Directory -Force "$output\licenses" | Out-Null
    Copy-Item -Path (Join-Path $compilerBin '..\x86_64-w64-mingw32\share\mingw32\COPYING*') -Destination "$output\licenses" -Force
    if (!$SkipTests) {
        & "$compilerBin\clang++.exe" @flags src/tests.cpp -o "$build\core-tests.exe"
        if ($LASTEXITCODE) { throw 'Core test build failed.' }
        & "$build\core-tests.exe" config.json
        if ($LASTEXITCODE) { throw 'Core tests failed.' }
    }
    Compress-Archive -LiteralPath $output -DestinationPath (Join-Path $PSScriptRoot 'dist\Epsilon-Engineering-Cpp-Optimized-win-x64.zip') -Force
    Write-Output "Built: $output\Epsilon Engineering Cpp Optimized.exe"
} finally { Pop-Location }
