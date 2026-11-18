$ErrorActionPreference = 'Stop'
$app = Join-Path (Split-Path -Parent $PSScriptRoot) 'publish/CantoWidget.exe'
$root = Split-Path -Parent $PSScriptRoot
if ((Test-Path -LiteralPath (Join-Path $root '.git')) -and (Test-Path -LiteralPath (Join-Path $root 'creator/CantoWidget.exe'))) {
    $localCreator = & git -C $root config --local --bool canto.creator 2>$null
    if ($localCreator -eq 'true') { $app = Join-Path $root 'creator/CantoWidget.exe' }
}
if (-not (Test-Path -LiteralPath $app)) { throw 'Execute scripts/Build.ps1 antes de iniciar.' }
Start-Process -FilePath $app -WindowStyle Hidden
