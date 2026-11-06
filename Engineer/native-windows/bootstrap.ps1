$ErrorActionPreference = 'Stop'
# Project-local Microsoft SDK, pinned and SHA-512 checked. No installer or PATH change.
$sdkVersion = '10.0.403'
$sdkHash = '7a114c318fa7f1d9822991b534b2814d0e78386ceda816b371f5141acefe741d87ccb606ede06cadac8f9ac5070e32b708a67bbd1619ff5e5a7858a27874ef48'
$toolRoot = Join-Path $PSScriptRoot 'tools'
$sdkExe = Join-Path $toolRoot 'dotnet\dotnet.exe'
if (Test-Path -LiteralPath $sdkExe) { Write-Output 'Project-local SDK already present.'; exit 0 }
New-Item -ItemType Directory -Path $toolRoot -Force | Out-Null
$sdkArchive = Join-Path $toolRoot 'dotnet-sdk.zip'
if (!(Test-Path -LiteralPath $sdkArchive)) {
  Invoke-WebRequest -Uri "https://builds.dotnet.microsoft.com/dotnet/Sdk/$sdkVersion/dotnet-sdk-$sdkVersion-win-x64.zip" -OutFile $sdkArchive
}
if ((Get-FileHash -LiteralPath $sdkArchive -Algorithm SHA512).Hash -ne $sdkHash) { throw 'Microsoft SDK SHA-512 verification failed.' }
Expand-Archive -LiteralPath $sdkArchive -DestinationPath (Join-Path $toolRoot 'dotnet') -Force
Write-Output 'SDK ready. Run .\build.ps1 next.'
