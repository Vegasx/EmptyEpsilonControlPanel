param([switch]$CppOnly, [int]$Trials = 2)
$ErrorActionPreference = 'Stop'
if ($Trials -lt 1 -or $Trials -gt 10) { throw 'Trials must be 1..10.' }
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$output = Join-Path $PSScriptRoot "artifacts\benchmark-$stamp"
New-Item -ItemType Directory -Force $output | Out-Null
$cpp = Join-Path $PSScriptRoot 'dist\Epsilon-Engineering-Cpp-win-x64\Epsilon Engineering Cpp.exe'
$dotnet = Join-Path $PSScriptRoot '..\native-windows\tools\dotnet\dotnet.exe'
$wpf = Join-Path $PSScriptRoot '..\native-windows\Epsilon.Native\bin\Release\net10.0-windows\Epsilon Engineering.dll'
if (!(Test-Path -LiteralPath $cpp)) { throw 'Build C++ first.' }
if (!$CppOnly -and !(Test-Path -LiteralPath $wpf)) { throw 'Build the native-windows project with -SkipPublish first.' }
$reports = @()
for ($trial = 1; $trial -le $Trials; ++$trial) {
    $order = if ($CppOnly) { @('cpp') } elseif ($trial % 2) { @('cpp', 'wpf') } else { @('wpf', 'cpp') }
    foreach ($implementation in $order) {
        $report = Join-Path $output "$implementation-$trial.json"
        Write-Output "Measuring $implementation trial $trial (about 50 seconds)."
        if ($implementation -eq 'cpp') {
            $process = Start-Process -FilePath $cpp -ArgumentList @('--profile', ('"' + $report + '"')) -WindowStyle Hidden -PassThru
        } else {
            $process = Start-Process -FilePath $dotnet -ArgumentList @(('"' + $wpf + '"'), '--windowed', '--profile', ('"' + $report + '"')) -WindowStyle Hidden -PassThru
        }
        if (!$process.WaitForExit(90000)) { $process.Kill(); throw "$implementation timed out." }
        if ($process.ExitCode) { throw "$implementation failed: $(Get-Content -Raw -LiteralPath $report)" }
        $data = Get-Content -Raw -LiteralPath $report | ConvertFrom-Json
        if ($data.error) { throw $data.error }
        if ([math]::Abs($data.width - 1000) -gt 1 -or [math]::Abs($data.height - 700) -gt 1) { throw "Unmatched panel size: $implementation measured $($data.width)x$($data.height); expected 1000x700." }
        if ($reports.Count -gt 0 -and $data.deviceScaleFactor -ne $reports[0].report.deviceScaleFactor) { throw 'Display scaling changed between runs.' }
        if ($data.reducedMotion) { throw 'Enable Windows animations for a matched animated comparison.' }
        $reports += [pscustomobject]@{ implementation = $implementation; trial = $trial; report = $data }
    }
}
$summary = foreach ($entry in $reports) {
    foreach ($phase in $entry.report.phases) {
        [pscustomobject]@{
            implementation = $entry.implementation; trial = $entry.trial; phase = $phase.name
            cpuMs = [math]::Round($phase.cpuMs, 2)
            machineCpuPercent = [math]::Round(100 * $phase.cpuMs / $phase.elapsedMs / $entry.report.logicalProcessors, 3)
            privateMiB = [math]::Round($phase.privateMiB, 2)
            width = $entry.report.width; height = $entry.report.height; scale = $entry.report.deviceScaleFactor
        }
    }
}
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'comparison.json') -Encoding utf8
$summary | Format-Table -AutoSize
Write-Output "Evidence: $output"
