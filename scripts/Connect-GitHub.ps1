param([string]$Account = 'andradehub', [string]$RepositoryName = 'canto-widget')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
function Require-Success([string]$message) { if ($LASTEXITCODE -ne 0) { throw $message } }
$git = Get-Command git -ErrorAction SilentlyContinue
if (-not $git) { throw 'Instale o Git for Windows em https://git-scm.com/download/win e execute novamente.' }
$ghCommand = Get-Command gh -ErrorAction SilentlyContinue
$toolRoot = Join-Path $env:LOCALAPPDATA 'CantoWidget/tools'
$bundledGh = Join-Path $toolRoot 'github-cli/bin/gh.exe'
if ($ghCommand) { $gh = $ghCommand.Source }
elseif (Test-Path -LiteralPath $bundledGh) { $gh = $bundledGh }
else {
    Write-Host 'Preparando o GitHub CLI oficial para a autenticação...'
    New-Item -ItemType Directory -Force $toolRoot | Out-Null
    $release = Invoke-RestMethod 'https://api.github.com/repos/cli/cli/releases/latest'
    $asset = $release.assets | Where-Object name -Match 'windows_amd64.zip$' | Select-Object -First 1
    $checksumsAsset = $release.assets | Where-Object name -Match 'checksums.txt$' | Select-Object -First 1
    if (-not $asset -or -not $checksumsAsset) { throw 'Não foi possível localizar o GitHub CLI oficial.' }
    $archive = Join-Path $toolRoot $asset.name
    Invoke-WebRequest $asset.browser_download_url -OutFile $archive -UseBasicParsing
    $checksums = (Invoke-WebRequest $checksumsAsset.browser_download_url -UseBasicParsing).Content
    if ($checksums -is [byte[]]) { $checksums = [Text.Encoding]::UTF8.GetString($checksums) }
    $line = $checksums -split "`n" | Where-Object { $_ -match ([regex]::Escape($asset.name) + '\s*$') } | Select-Object -First 1
    $expected = ($line -split '\s+')[0]
    $algorithm = [Security.Cryptography.SHA256]::Create()
    $stream = [IO.File]::OpenRead($archive)
    try { $actual = [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '') } finally { $stream.Dispose(); $algorithm.Dispose() }
    if (-not $expected -or $actual -ne $expected) { throw 'A verificação do download do GitHub CLI falhou.' }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::ExtractToDirectory($archive, (Join-Path $toolRoot 'github-cli'))
    Remove-Item -LiteralPath $archive
    $gh = $bundledGh
}
if (-not (Test-Path -LiteralPath $gh)) { throw 'GitHub CLI indisponível.' }
$previousPreference = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
& $gh auth status --hostname github.com 2>$null
$authExit = $LASTEXITCODE
$ErrorActionPreference = $previousPreference
if ($authExit -ne 0) {
    Write-Host 'Entre com a conta andradehub na página de autenticação e autorize o código apresentado.'
    & $gh auth login --hostname github.com --git-protocol https --web
    Require-Success 'A autenticação não foi concluída.'
}
$login = & $gh api user --jq '.login'
Require-Success 'Não foi possível verificar sua conta.'
if ($login -ne $Account) { throw "A conta ativa é $login. Entre como $Account no GitHub CLI antes de publicar." }
& $gh auth setup-git --hostname github.com
Require-Success 'Não foi possível configurar o acesso Git.'
if (-not (Test-Path -LiteralPath (Join-Path $root '.git'))) { & git -C $root init -b main; Require-Success 'Não foi possível iniciar o repositório.' }
& git -C $root config user.name $login
$id = & $gh api user --jq '.id'
Require-Success 'Não foi possível obter o ID da sua conta.'
& git -C $root config user.email "$id+$login@users.noreply.github.com"
$target = "$Account/$RepositoryName"
$previousPreference = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
& $gh repo view $target --json name 2>$null | Out-Null
$viewExit = $LASTEXITCODE
$ErrorActionPreference = $previousPreference
if ($viewExit -ne 0) {
    & $gh repo create $target --public --description 'Música e bateria do mouse em um widget compacto para Windows.'
    Require-Success 'Não foi possível criar o repositório público.'
}
$previousPreference = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
$origin = & git -C $root remote get-url origin 2>$null
$originExit = $LASTEXITCODE
$ErrorActionPreference = $previousPreference
if ($originExit -ne 0) { & git -C $root remote add origin "https://github.com/$target.git" }
elseif ($origin -notmatch ('github\.com[:/]' + [regex]::Escape($target) + '(\.git)?$')) { throw 'Este projeto já aponta para outro origin. Revise antes de publicar.' }
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'Sync-GitHub.ps1') -Repository $root -Once
Require-Success 'O envio não foi concluído. Os commits locais foram preservados; revise a mensagem acima.'
Write-Host "Publicado: https://github.com/$target" -ForegroundColor Green
