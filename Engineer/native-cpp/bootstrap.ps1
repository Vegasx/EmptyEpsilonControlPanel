$ErrorActionPreference = 'Stop'
$version = '20260616'
$folder = Join-Path $PSScriptRoot "tools\llvm-mingw-$version-ucrt-x86_64"
if (Test-Path -LiteralPath (Join-Path $folder 'bin\clang++.exe')) { Write-Output "Compiler ready: $folder"; return }
$archive = Join-Path $PSScriptRoot 'tools\llvm-mingw.zip'
New-Item -ItemType Directory -Force (Split-Path $archive) | Out-Null
if (!(Test-Path -LiteralPath $archive)) {
    Invoke-WebRequest "https://github.com/mstorsjo/llvm-mingw/releases/download/$version/llvm-mingw-$version-ucrt-x86_64.zip" -OutFile $archive
}
# SHA-256 published in the upstream GitHub release asset's digest field.
$expected = 'b9b68a4d276e16fa25802aaba458e4638f64b3884c290aaccdc2d87083b6ca35'
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected) { throw 'Compiler archive checksum mismatch.' }
Expand-Archive -LiteralPath $archive -DestinationPath (Join-Path $PSScriptRoot 'tools') -Force
Write-Output "Portable compiler ready: $folder"
