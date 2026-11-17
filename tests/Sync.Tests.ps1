$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('canto-sync-test-' + [Guid]::NewGuid().ToString('N'))
$repo = Join-Path $fixture 'checkout'
$remote = Join-Path $fixture 'remote.git'
New-Item -ItemType Directory -Path $repo -Force | Out-Null
function Test-Git([string[]]$Arguments) { & git @Arguments | Out-Null; if ($LASTEXITCODE -ne 0) { throw "Git falhou: $Arguments" } }
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message }; Write-Host "PASS $Message" }
try {
    Test-Git @('init', '--bare', $remote)
    Test-Git @('-C', $repo, 'init', '-b', 'main')
    Test-Git @('-C', $repo, 'config', 'user.name', 'Canto Sync Test')
    Test-Git @('-C', $repo, 'config', 'user.email', 'test@example.invalid')
    Test-Git @('-C', $repo, 'remote', 'add', 'origin', $remote)
    [IO.File]::WriteAllText((Join-Path $repo 'README.md'), 'Primeira versão')
    [IO.File]::WriteAllText((Join-Path $repo '.env'), 'TEST_ONLY=do-not-upload')
    [IO.File]::WriteAllText((Join-Path $repo 'credentials.json'), '{"test":"do-not-upload"}')
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'scripts/Sync-GitHub.ps1') -Repository $repo -Once
    Assert ($LASTEXITCODE -eq 0) 'commit e push para um remoto de teste'
    $tracked = & git -C $repo ls-tree -r --name-only HEAD
    Assert ($tracked -contains 'README.md') 'código autorizado enviado'
    Assert ($tracked -notcontains '.env' -and $tracked -notcontains 'credentials.json') 'credenciais fora do commit'
    [IO.File]::WriteAllText((Join-Path $repo 'README.md'), 'Segunda versão')
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'scripts/Sync-GitHub.ps1') -Repository $repo -Once
    Assert ($LASTEXITCODE -eq 0) 'segunda alteração sincronizada'
    $count = & git -C $repo rev-list --count HEAD
    Assert ($count -eq '2') 'histórico preserva as duas versões'
    $remoteHead = & git --git-dir=$remote rev-parse refs/heads/main
    $head = & git -C $repo rev-parse HEAD
    Assert ($head -eq $remoteHead) 'HEAD local e remoto coincidem'
    $watchArguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ('"' + (Join-Path $root 'scripts/Sync-GitHub.ps1') + '"'), '-Repository', ('"' + $repo + '"'), '-QuietSeconds', '1')
    $watcher = Start-Process -FilePath powershell.exe -ArgumentList $watchArguments -WindowStyle Hidden -PassThru
    try {
        Start-Sleep -Seconds 2
        [IO.File]::WriteAllText((Join-Path $repo 'README.md'), 'Terceira versão salva durante o monitoramento')
        $deadline = [DateTimeOffset]::Now.AddSeconds(25)
        do { Start-Sleep -Seconds 1; $count = & git -C $repo rev-list --count HEAD } while ($count -ne '3' -and [DateTimeOffset]::Now -lt $deadline)
        Assert ($count -eq '3') 'salvamento detectado automaticamente pelo monitor'
        $head = & git -C $repo rev-parse HEAD
        $deadline = [DateTimeOffset]::Now.AddSeconds(10)
        do { Start-Sleep -Milliseconds 500; $remoteHead = & git --git-dir=$remote rev-parse refs/heads/main } while ($head -ne $remoteHead -and [DateTimeOffset]::Now -lt $deadline)
        Assert ($head -eq $remoteHead) 'push automático levou o salvamento ao remoto'
    }
    finally {
        $hash = [Security.Cryptography.SHA256]::Create()
        $key = ([BitConverter]::ToString($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes($repo.ToLowerInvariant())))).Replace('-', '').Substring(0, 12).ToLowerInvariant()
        $hash.Dispose()
        $stopFile = Join-Path $env:LOCALAPPDATA "CantoWidget/sync-$key.stop"
        [IO.File]::WriteAllText($stopFile, 'stop')
        $stopped = $watcher.WaitForExit(10000)
        if (-not $stopped) { $watcher | Stop-Process }
        $watcher.Dispose()
        Remove-Item -LiteralPath $stopFile -ErrorAction SilentlyContinue
        Assert $stopped 'monitor encerra de forma cooperativa'
    }
    Test-Git @('-C', $repo, 'add', '.env')
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'scripts/Sync-GitHub.ps1') -Repository $repo -Once 2>$null
    Assert ($LASTEXITCODE -ne 0) 'arquivo sensível preparado manualmente bloqueia o envio automático'
    Test-Git @('-C', $repo, 'reset', 'HEAD', '--', '.env')
    # A remote edit must be reported, never overwritten by an automatic force push.
    $other = Join-Path $fixture 'other'
    Test-Git @('clone', '-b', 'main', $remote, $other)
    Test-Git @('-C', $other, 'config', 'user.name', 'Canto Remote Test')
    Test-Git @('-C', $other, 'config', 'user.email', 'test@example.invalid')
    [IO.File]::WriteAllText((Join-Path $other 'README.md'), 'Alteração remota')
    Test-Git @('-C', $other, 'add', 'README.md')
    Test-Git @('-C', $other, 'commit', '-m', 'remote edit')
    Test-Git @('-C', $other, 'push', 'origin', 'main')
    $before = & git --git-dir=$remote rev-parse refs/heads/main
    [IO.File]::WriteAllText((Join-Path $repo 'README.md'), 'Alteração local divergente')
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'scripts/Sync-GitHub.ps1') -Repository $repo -Once 2>$null
    Assert ($LASTEXITCODE -ne 0) 'divergência remota é sinalizada'
    $after = & git --git-dir=$remote rev-parse refs/heads/main
    Assert ($before -eq $after) 'nenhum histórico remoto foi sobrescrito'
}
finally {
    # Delete only the fresh fixture created above, after checking its absolute parent.
    $resolvedFixture = [IO.Path]::GetFullPath($fixture)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if ($resolvedFixture.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and (Split-Path -Leaf $resolvedFixture) -like 'canto-sync-test-*') {
        Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
    }
}
