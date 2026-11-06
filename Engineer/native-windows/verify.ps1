$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'dist\Epsilon-Engineering-win-x64\Epsilon Engineering.exe'
$reportPath = Join-Path $PSScriptRoot 'artifacts\interaction-verification.json'
New-Item -ItemType Directory (Split-Path $reportPath) -Force | Out-Null
$testProcess = Start-Process -FilePath $exe -ArgumentList ('--verify "' + $reportPath + '"') -PassThru
$testProcess.WaitForExit()
Get-Content -LiteralPath $reportPath
if ($testProcess.ExitCode -ne 0) { throw 'Native interaction verification failed.' }
