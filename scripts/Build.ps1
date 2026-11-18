param([string]$Cargo = 'cargo', [switch]$Creator)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$featureArguments = @()
$destination = 'publish'
if ($Creator) {
    $origin = & git -C $root remote get-url origin
    if ($LASTEXITCODE -ne 0 -or $origin -notin @('https://github.com/andradehub/canto-widget.git', 'git@github.com:andradehub/canto-widget.git')) {
        throw 'A versão do criador exige o checkout de andradehub/canto-widget.'
    }
    & git -C $root config --local canto.creator true
    if ($LASTEXITCODE -ne 0) { throw 'Não foi possível habilitar este checkout local.' }
    $featureArguments = @('--features', 'creator-tools')
    $destination = 'creator'
}
& $Cargo build --release --locked --no-default-features @featureArguments --manifest-path (Join-Path $root 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Falha ao compilar o Canto.' }
New-Item -ItemType Directory -Force (Join-Path $root $destination) | Out-Null
$executable = Join-Path $root "$destination/CantoWidget.exe"
Copy-Item -LiteralPath (Join-Path $root 'target/release/CantoWidget.exe') -Destination $executable -Force
Write-Host ('Pronto: ' + $executable)
