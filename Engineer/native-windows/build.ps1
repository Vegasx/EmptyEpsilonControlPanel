param([switch]$SkipPublish)
$ErrorActionPreference = 'Stop'
$nativeRoot = $PSScriptRoot
$env:DOTNET_CLI_HOME = Join-Path $nativeRoot 'tools\cli-home'
$env:NUGET_PACKAGES = Join-Path $nativeRoot 'tools\packages'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_NOLOGO = '1'
$env:DOTNET_ADD_GLOBAL_TOOLS_TO_PATH = 'false'
$previousAppData = $env:APPDATA
$env:APPDATA = Join-Path $nativeRoot 'tools\appdata'
New-Item -ItemType Directory -Path $env:APPDATA -Force | Out-Null
$dotnetPath = Join-Path $nativeRoot 'tools\dotnet\dotnet.exe'
if (!(Test-Path -LiteralPath $dotnetPath)) { $dotnetPath = (Get-Command dotnet -ErrorAction Stop).Source }
Push-Location $nativeRoot
try {
  & $dotnetPath restore Epsilon.Native\Epsilon.Native.csproj --configfile NuGet.Config
  if ($LASTEXITCODE) { throw 'Native restore failed.' }
  & $dotnetPath build Epsilon.Native\Epsilon.Native.csproj -c Release --no-restore
  if ($LASTEXITCODE) { throw 'Native build failed.' }
  & $dotnetPath restore Epsilon.Tests\Epsilon.Tests.csproj --configfile NuGet.Config
  if ($LASTEXITCODE) { throw 'Test restore failed.' }
  & $dotnetPath run --project Epsilon.Tests\Epsilon.Tests.csproj -c Release --no-restore -- (Join-Path $nativeRoot '..\config.json')
  if ($LASTEXITCODE) { throw 'Core tests failed.' }
  if (!$SkipPublish) {
    & $dotnetPath restore Epsilon.Native\Epsilon.Native.csproj -r win-x64 -p:SelfContained=true --configfile NuGet.Config
    if ($LASTEXITCODE) { throw 'Runtime pack restore failed.' }
    & $dotnetPath publish Epsilon.Native\Epsilon.Native.csproj -c Release -r win-x64 --self-contained true --no-restore -p:PublishSingleFile=false -o (Join-Path $nativeRoot 'dist\Epsilon-Engineering-win-x64')
    if ($LASTEXITCODE) { throw 'Self-contained publish failed.' }
    Copy-Item -LiteralPath (Join-Path $nativeRoot 'RUNME.txt') -Destination (Join-Path $nativeRoot 'dist\Epsilon-Engineering-win-x64\RUNME.txt') -Force
  }
} finally { Pop-Location; $env:APPDATA = $previousAppData }
