param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
$OutputDirectory=[IO.Path]::GetFullPath($OutputDirectory)
$projectRoot=Split-Path $PSScriptRoot
$dotnet=Join-Path $projectRoot 'native-windows\tools\dotnet\dotnet.exe'
$native=Join-Path $projectRoot 'native-windows\Epsilon.Native\bin\Release\net10.0-windows\Epsilon Engineering.dll'
$gpuFile=Join-Path $OutputDirectory 'gpu-isolated.json'
$sampler=Start-Process powershell.exe -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+(Join-Path $PSScriptRoot 'gpu-sample.ps1')+'"'),'-Output',('"'+$gpuFile+'"'),'-Seconds','140') -WindowStyle Hidden -PassThru
try {
    $output=Join-Path $OutputDirectory 'native-gpu.json'
    Write-Output 'GPU-only pass: native'
    $app=Start-Process $dotnet -ArgumentList @(('"'+$native+'"'),'--windowed','--profile',('"'+$output+'"')) -WindowStyle Hidden -PassThru
    if(!$app.WaitForExit(65000)){$app.Kill();throw 'Native GPU workload timed out'}
    if($app.ExitCode){throw 'Native GPU workload failed'}
    $env:PROFILE_SCALE=(Get-Content $output -Raw | ConvertFrom-Json).deviceScaleFactor.ToString([Globalization.CultureInfo]::InvariantCulture)
    $env:PROFILE_NO_BROWSER_TRACE='1'
    if(!$env:CHROME_EXE){$env:CHROME_EXE=Get-Process chrome -ErrorAction SilentlyContinue | Where-Object Path | Select-Object -First 1 -ExpandProperty Path}
    if(!$env:PLAYWRIGHT_MODULE){$env:PLAYWRIGHT_MODULE='C:\Users\Cheyenne\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\node_modules\playwright'}
    Write-Output 'GPU-only pass: browser'
    & 'C:\Program Files\nodejs\node.exe' (Join-Path $PSScriptRoot 'profile-browser.cjs') (Join-Path $OutputDirectory 'browser-gpu.json')
    if($LASTEXITCODE){throw 'Browser GPU workload failed'}
} finally { Set-Content ($gpuFile+'.stop') 'stop';$sampler.WaitForExit(8000)|Out-Null }
@{reports=@('native-gpu.json','browser-gpu.json');sampleFile='gpu-isolated.json'} | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'gpu-sources.json') -Encoding utf8
