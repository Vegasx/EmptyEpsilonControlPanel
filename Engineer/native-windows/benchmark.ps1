$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'dist\Epsilon-Engineering-win-x64\Epsilon Engineering.exe'
$reportPath = Join-Path $PSScriptRoot 'artifacts\native-performance.json'
New-Item -ItemType Directory (Split-Path $reportPath) -Force | Out-Null
$testProcess = Start-Process -FilePath $exe -ArgumentList ('--benchmark "' + $reportPath + '"') -PassThru
$testProcess.WaitForExit()
Get-Content -LiteralPath $reportPath
if ($testProcess.ExitCode -ne 0) { throw 'Native benchmark failed.' }
