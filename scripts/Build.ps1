param([string]$Cargo = 'cargo')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
& $Cargo build --release --locked --manifest-path (Join-Path $root 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Falha ao compilar o Canto.' }
New-Item -ItemType Directory -Force (Join-Path $root 'publish') | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'target/release/CantoWidget.exe') -Destination (Join-Path $root 'publish/CantoWidget.exe') -Force
Write-Host ('Pronto: ' + (Join-Path $root 'publish/CantoWidget.exe'))