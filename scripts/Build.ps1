param([string]$Dotnet = 'dotnet')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
& $Dotnet publish (Join-Path $root 'src/CantoWidget/CantoWidget.csproj') -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:DebugType=None -o (Join-Path $root 'publish')
if ($LASTEXITCODE -ne 0) { throw 'Falha ao compilar o Canto.' }
Write-Host ('Pronto: ' + (Join-Path $root 'publish/CantoWidget.exe'))
