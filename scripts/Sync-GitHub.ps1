param(
    [string]$Repository = (Split-Path -Parent $PSScriptRoot),
    [switch]$Once,
    [int]$QuietSeconds = 20
)
$ErrorActionPreference = 'Stop'
$Repository = [IO.Path]::GetFullPath($Repository).TrimEnd([IO.Path]::DirectorySeparatorChar)
$env:GIT_TERMINAL_PROMPT = '0'
$env:GCM_INTERACTIVE = 'never'
$env:GH_PROMPT_DISABLED = '1'
$keyBytes = [Text.Encoding]::UTF8.GetBytes($Repository.ToLowerInvariant())
$hasher = [Security.Cryptography.SHA256]::Create()
$repoKey = ([BitConverter]::ToString($hasher.ComputeHash($keyBytes))).Replace('-', '').Substring(0, 12).ToLowerInvariant()
$hasher.Dispose()
$localDirectory = Join-Path $env:LOCALAPPDATA 'CantoWidget'
New-Item -ItemType Directory -Force $localDirectory | Out-Null
$statusPath = Join-Path $localDirectory "sync-$repoKey.json"
$stopPath = Join-Path $localDirectory "sync-$repoKey.stop"
$mutex = New-Object Threading.Mutex($false, "Local\CantoWidget.Sync.$repoKey")
$waitMilliseconds = 0
if ($Once) { $waitMilliseconds = 30000 }
if (-not $mutex.WaitOne($waitMilliseconds)) { $mutex.Dispose(); if ($Once) { Write-Error 'Aguarde a sincronização atual terminar e tente novamente.'; exit 1 }; exit 0 }
function Write-Status([string]$message, [string]$state) {
    $status = @{ message = $message; state = $state; updatedAt = [DateTimeOffset]::Now.ToString('O') } | ConvertTo-Json
    $temporary = "$statusPath.tmp"
    [IO.File]::WriteAllText($temporary, $status, (New-Object Text.UTF8Encoding($false)))
    Move-Item -LiteralPath $temporary -Destination $statusPath -Force
}
function Invoke-Git([string[]]$Arguments) {
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { $result = & git -C $Repository @Arguments 2>&1; $code = $LASTEXITCODE }
    finally { $ErrorActionPreference = $previous }
    if ($code -ne 0) { throw ('Git falhou (' + $Arguments[0] + '). ' + (($result | Out-String).Trim())) }
    return ($result | Out-String).TrimEnd("`r", "`n")
}
function Test-SourcePath([string]$path) {
    if ($path -match '(^|/)(bin|obj|publish|\.git|node_modules|work|outputs)/' -or $path -match '(^|/)(\.env($|\.)|credentials|secrets)' -or $path -match '\.(pem|key|pfx|p12|log|exe|dll|zip)$') { return $false }
    return $path -match '^(src/.*\.(cs|xaml|csproj|manifest)|scripts/[^/]+\.ps1|tests/.*\.(cs|csproj|ps1)|\.github/workflows/[^/]+\.ya?ml|README\.md|LICENSE|THIRD_PARTY_NOTICES\.md|\.gitignore|\.gitattributes|global\.json|CantoWidget\.slnx)$'
}
function Get-ContentHash([string]$path) {
    $algorithm = [Security.Cryptography.SHA256]::Create()
    $stream = [IO.File]::OpenRead($path)
    try { return [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '') }
    finally { $stream.Dispose(); $algorithm.Dispose() }
}
function Invoke-Sync {
    $remote = Invoke-Git @('remote', 'get-url', 'origin')
    if (-not $remote) { throw 'Nenhum origin configurado.' }
    $branch = Invoke-Git @('branch', '--show-current')
    if (-not $branch) { throw 'O repositório precisa estar em uma branch.' }
    $staged = (Invoke-Git @('diff', '--cached', '--name-only', '-z')) -split "`0" | Where-Object { $_ }
    foreach ($path in $staged) { if (-not (Test-SourcePath $path)) { throw 'Há arquivos fora do código já preparados para commit. Revise o índice antes de sincronizar.' } }
    $files = (Invoke-Git @('ls-files', '--cached', '--others', '--exclude-standard', '-z')) -split "`0" | Where-Object { $_ -and (Test-SourcePath $_) } | Sort-Object -Unique
    foreach ($path in $files) { $null = Invoke-Git @('add', '--all', '--', $path) }
    $diff = Invoke-Git @('diff', '--cached', '--name-only')
    if ($diff) { $null = Invoke-Git @('commit', '-m', ('Atualização automática · ' + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'))) }
    # A rejected push stays local and is retried. Never rewrite the remote history.
    $null = Invoke-Git @('push', '--set-upstream', 'origin', $branch)
    Write-Status ('sincronizado às ' + (Get-Date -Format 'HH:mm')) 'synced'
}
try {
    $null = Invoke-Git @('rev-parse', '--show-toplevel')
    if ($Once) { Invoke-Sync; exit 0 }
    $lastFingerprint = ''
    $changedAt = [DateTimeOffset]::MinValue
    $nextAttempt = [DateTimeOffset]::MinValue
    $needsSync = $true
    while (-not (Test-Path -LiteralPath $stopPath)) {
        try {
            $fingerprint = Invoke-Git @('status', '--porcelain', '--untracked-files=all')
            # Content hash also notices repeated edits to an already modified file.
            $sourceFiles = (Invoke-Git @('ls-files', '--cached', '--others', '--exclude-standard', '-z')) -split "`0" | Where-Object { $_ -and (Test-SourcePath $_) } | Sort-Object -Unique
            $signature = foreach ($path in $sourceFiles) { $fullPath = Join-Path $Repository $path; if (Test-Path -LiteralPath $fullPath -PathType Leaf) { $path + ':' + (Get-ContentHash $fullPath) } else { $path + ':deleted' } }
            $fingerprint += ($signature -join '|')
            if ($fingerprint -ne $lastFingerprint) { $lastFingerprint = $fingerprint; $changedAt = [DateTimeOffset]::Now; $needsSync = $true; Write-Status 'alterações detectadas · aguardando salvar' 'pending' }
            if ($needsSync -and ([DateTimeOffset]::Now - $changedAt).TotalSeconds -ge $QuietSeconds -and [DateTimeOffset]::Now -ge $nextAttempt) {
                Invoke-Sync
                $needsSync = $false
            }
        }
        catch {
            $detail = $_.Exception.Message
            $logPath = Join-Path $localDirectory "sync-$repoKey.log"
            if ((Test-Path -LiteralPath $logPath) -and (Get-Item -LiteralPath $logPath).Length -gt 1000000) { [IO.File]::WriteAllText($logPath, '') }
            [IO.File]::AppendAllText($logPath, ([DateTimeOffset]::Now.ToString('O') + ' ' + $detail + [Environment]::NewLine), (New-Object Text.UTF8Encoding($false)))
            if ($detail -match 'remote|origin|No such|No remote') { Write-Status 'repositório ainda não conectado' 'disconnected' }
            elseif ($detail -match 'rejected|non-fast-forward|fetch first') { Write-Status 'envio recusado · há alterações remotas para revisar' 'conflict' }
            elseif ($detail -match 'identity|user.email|user.name') { Write-Status 'configure seu nome e e-mail no Git' 'identity' }
            else { Write-Status 'envio pendente · verifique conexão e login do GitHub' 'error' }
            $nextAttempt = [DateTimeOffset]::Now.AddSeconds(60)
            $needsSync = $true
        }
        Start-Sleep -Seconds 5
    }
}
catch { Write-Status 'sincronização indisponível · verifique o repositório' 'error'; Write-Error $_; exit 1 }
finally { $mutex.ReleaseMutex(); $mutex.Dispose() }
