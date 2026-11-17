$ErrorActionPreference = 'Stop'
$app = Join-Path (Split-Path -Parent $PSScriptRoot) 'publish/CantoWidget.exe'
if (-not (Test-Path -LiteralPath $app)) { throw 'Execute scripts/Build.ps1 antes de iniciar.' }
Start-Process -FilePath $app -WindowStyle Hidden
