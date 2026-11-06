param([int]$Trials = 2)
$ErrorActionPreference = 'Stop'
if ($Trials -lt 2 -or $Trials -gt 10) { throw 'Trials must be 2..10 so both implementation orders are measured.' }
$compiler = Join-Path $PSScriptRoot 'tools\llvm-mingw-20260616-ucrt-x86_64\bin\clang++.exe'
$samplerExe = Join-Path $PSScriptRoot 'build\gpu-sampler.exe'
& $compiler -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror -static -municode (Join-Path $PSScriptRoot 'src\gpu-sampler.cpp') -o $samplerExe -lpdh
if ($LASTEXITCODE) { throw 'GPU sampler build failed.' }
& $compiler -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror -static (Join-Path $PSScriptRoot 'src\gpu-info.cpp') -o (Join-Path $PSScriptRoot 'build\gpu-info.exe') -ldxgi -luuid
if ($LASTEXITCODE) { throw 'GPU metadata build failed.' }
$output = Join-Path $PSScriptRoot ('artifacts\gpu-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force $output | Out-Null
& (Join-Path $PSScriptRoot 'build\gpu-info.exe') | Set-Content -LiteralPath (Join-Path $output 'hardware.json') -Encoding utf8
if ($LASTEXITCODE) { throw 'GPU metadata collection failed.' }
$cpp = Join-Path $PSScriptRoot 'dist\Epsilon-Engineering-Cpp-win-x64\Epsilon Engineering Cpp.exe'
$dotnet = Join-Path $PSScriptRoot '..\native-windows\tools\dotnet\dotnet.exe'
$wpf = Join-Path $PSScriptRoot '..\native-windows\Epsilon.Native\bin\Release\net10.0-windows\Epsilon Engineering.dll'
$runs = @()
for ($trial = 1; $trial -le $Trials; ++$trial) {
    $order = if ($trial % 2) { @('cpp', 'wpf') } else { @('wpf', 'cpp') }
    foreach ($implementation in $order) {
        Write-Output "GPU profiling: $implementation trial $trial (about 50 seconds)."
        $report = Join-Path $output "$implementation-$trial.json"
        $samples = Join-Path $output "$implementation-$trial-gpu.json"
        $app = $null; $sampler = $null
        try {
            if ($implementation -eq 'cpp') { $app = Start-Process $cpp -ArgumentList @('--profile', ('"'+$report+'"')) -WindowStyle Hidden -PassThru }
            else { $app = Start-Process $dotnet -ArgumentList @(('"'+$wpf+'"'), '--windowed', '--profile', ('"'+$report+'"')) -WindowStyle Hidden -PassThru }
            $sampler = Start-Process $samplerExe -ArgumentList @($app.Id, ('"'+$samples+'"')) -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $output "$implementation-$trial-sampler-errors.txt")
            if (!$app.WaitForExit(70000)) { throw 'GPU workload timed out.' }
            if ($app.ExitCode) { throw "Workload failed: $report" }
            $data = Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
            if ($data.width -ne 1000 -or $data.height -ne 700 -or $data.reducedMotion) { throw 'Unmatched workload dimensions/animations.' }
            if ($runs.Count -and $data.deviceScaleFactor -ne $runs[0].scale) { throw 'Display scaling changed during comparison.' }
            $runs += @{ implementation = $implementation; trial = $trial; report = "$implementation-$trial.json"; samples = "$implementation-$trial-gpu.json"; pid = $app.Id; scale = $data.deviceScaleFactor }
        } finally {
            if ($app -and !$app.HasExited) { $app.Kill(); $app.WaitForExit() }
            if ($sampler) {
                Set-Content -LiteralPath ($samples+'.stop') -Value stop
                if (!$sampler.WaitForExit(10000)) { $sampler.Kill(); throw 'GPU sampler failed to stop.' }
                if ($sampler.ExitCode) { throw "GPU sampler failed: $(Get-Content -Raw (Join-Path $output "$implementation-$trial-sampler-errors.txt"))" }
            }
        }
    }
}
@{runs=$runs} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $output 'runs.json') -Encoding utf8
& node (Join-Path $PSScriptRoot 'summarize-gpu.cjs') $output
if ($LASTEXITCODE) { throw 'GPU summary validation failed.' }
Write-Output "GPU evidence: $output"
