param([string]$OutputDirectory,[switch]$NoBrowserTrace)
$ErrorActionPreference='Stop'
$profileRoot=$PSScriptRoot
$projectRoot=Split-Path $profileRoot
$dotnet=Join-Path $projectRoot 'native-windows\tools\dotnet\dotnet.exe'
$reader=Join-Path $profileRoot 'TraceReader\bin\Release\net10.0-windows\TraceReader.dll'
$native=Join-Path $projectRoot 'native-windows\Epsilon.Native\bin\Release\net10.0-windows\Epsilon Engineering.dll'
if(!$OutputDirectory){$OutputDirectory=Join-Path $profileRoot ('artifacts\run-'+(Get-Date -Format 'yyyyMMdd-HHmmss'))}
$OutputDirectory=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory $OutputDirectory -Force | Out-Null
$trace=Join-Path $OutputDirectory 'cpu.etl'
$gpu=Join-Path $OutputDirectory 'gpu.json'
$collector=$null;$gpuProcess=$null
try {
  $principal=[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
  $launch=@{FilePath=$dotnet;ArgumentList=@(('"'+$reader+'"'),'capture',('"'+$trace+'"'));WindowStyle='Hidden';PassThru=$true}
  if(!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){$launch.Verb='RunAs'}
  $collector=Start-Process @launch
  $deadline=(Get-Date).AddMinutes(2)
  while(!(Test-Path ($trace+'.ready'))){
    if(Test-Path ($trace+'.error')){throw (Get-Content ($trace+'.error') -Raw)}
    if((Get-Date) -gt $deadline){throw 'CPU sampler approval/startup timed out'}
    Start-Sleep -Milliseconds 300
  }
  Write-Output "CPU sampler ready. Results: $OutputDirectory"
  $gpuProcess=Start-Process powershell.exe -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+(Join-Path $profileRoot 'gpu-sample.ps1')+'"'),'-Output',('"'+$gpu+'"'),'-Seconds','300') -WindowStyle Hidden -PassThru
  $env:PLAYWRIGHT_MODULE='C:\Users\Cheyenne\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\node_modules\playwright'
  if(!$env:CHROME_EXE){$env:CHROME_EXE=Get-Process chrome -ErrorAction SilentlyContinue | Where-Object Path | Select-Object -First 1 -ExpandProperty Path}
  $node='C:\Program Files\nodejs\node.exe'
  $env:PROFILE_NO_BROWSER_TRACE=if($NoBrowserTrace){'1'}else{''}
  $reports=[System.Collections.Generic.List[string]]::new()
  foreach($trial in 1,2){
    $order=if($trial -eq 1){@('native','browser')}else{@('browser','native')}
    foreach($implementation in $order){
      $output=Join-Path $OutputDirectory "$implementation-$trial.json"
      Write-Output "Profiling $implementation, trial $trial"
      if($implementation -eq 'native'){
        $app=Start-Process $dotnet -ArgumentList @(('"'+$native+'"'),'--windowed','--profile',('"'+$output+'"')) -WindowStyle Hidden -PassThru
        if(!$app.WaitForExit(65000)){ $app.Kill();throw 'Native profile timed out' }
        if($app.ExitCode -ne 0){throw "Native profile failed: $output"}
        $env:PROFILE_SCALE=(Get-Content $output -Raw | ConvertFrom-Json).deviceScaleFactor.ToString([Globalization.CultureInfo]::InvariantCulture)
      } else {
        & $node (Join-Path $profileRoot 'profile-browser.cjs') $output
        if($LASTEXITCODE){throw 'Browser profile failed'}
      }
      $reports.Add($output)
      Start-Sleep -Seconds 2
    }
  }
} finally {
  if($collector){Set-Content ($trace+'.stop') 'stop';$collector.WaitForExit(20000)|Out-Null}
  if($gpuProcess){Set-Content ($gpu+'.stop') 'stop';$gpuProcess.WaitForExit(8000)|Out-Null}
}
& $dotnet $reader analyze $trace (Join-Path $OutputDirectory 'cpu-attribution.json') @reports
if($LASTEXITCODE){throw 'CPU trace analysis failed'}
Write-Output "Completed: $OutputDirectory"
