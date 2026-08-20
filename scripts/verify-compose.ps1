[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$modelPath = [System.IO.Path]::GetTempFileName()
$overrides = @{
    PEPEAUDIO_DOMAIN = 'audio.example.test'
    PEPEAUDIO_DISCORD_CLIENT_ID = '100000000000000002'
    PEPEAUDIO_VALKEY_KEYSPACE = 'pepeaudio-production'
    PEPEAUDIO_SHARD_TOTAL = '4'
    PEPEAUDIO_RUNTIME_GID = '10001'
    PEPEAUDIO_DISCORD_CLIENT_SECRET_SOURCE = Join-Path $repositoryRoot 'secrets\discord_client_secret.txt.example'
    PEPEAUDIO_SPOTIFY_CLIENT_ID = 'compose-contract-client-id'
    PEPEAUDIO_SPOTIFY_CLIENT_SECRET_SOURCE = Join-Path $repositoryRoot 'secrets\spotify_client_secret.txt.example'
    PEPEAUDIO_SPOTIFY_MARKET = 'JP'
    PEPEAUDIO_APPLE_MUSIC_TEAM_ID = 'ABCDE12345'
    PEPEAUDIO_APPLE_MUSIC_KEY_ID = 'KEY1234567'
    PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_SOURCE = Join-Path $repositoryRoot 'secrets\apple_music_private_key.p8.example'
}
$previous = @{}

function Invoke-Checked {
    param([Parameter(Mandatory)][string]$Executable, [string[]]$Arguments = @())

    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Executable exited with code $LASTEXITCODE"
    }
}

Push-Location $repositoryRoot
try {
    foreach ($name in $overrides.Keys) {
        $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
        [Environment]::SetEnvironmentVariable($name, $overrides[$name], 'Process')
    }

    Invoke-Checked docker @('compose', 'version')
    Invoke-Checked docker @(
        'compose', '-f', 'compose.yaml', '--profile', 'development-api',
        'config', '--quiet'
    )
    Invoke-Checked docker @(
        'compose', '-f', 'compose.yaml', '-f', 'compose.discord.yaml',
        '--profile', 'development-api', '--profile', 'discord',
        'config', '--quiet'
    )
    Invoke-Checked docker @(
        'compose', '-f', 'compose.yaml', '-f', 'compose.discord.yaml',
        '-f', 'compose.catalog.spotify.yaml', '--profile', 'discord',
        'config', '--quiet'
    )
    Invoke-Checked docker @(
        'compose', '-f', 'compose.yaml', '-f', 'compose.discord.yaml',
        '-f', 'compose.catalog.apple.yaml', '--profile', 'discord',
        'config', '--quiet'
    )
    Invoke-Checked docker @(
        'compose', '-f', 'compose.yaml', '-f', 'compose.discord.yaml',
        '-f', 'compose.catalog.public-metadata.yaml', '--profile', 'discord',
        'config', '--quiet'
    )

    Invoke-Checked docker @(
        'compose', '-f', 'compose.yaml', '-f', 'compose.discord.yaml',
        '-f', 'compose.production.yaml', '--profile', 'production',
        'config', '--quiet'
    )
    $model = & docker compose `
        -f compose.yaml `
        -f compose.discord.yaml `
        -f compose.production.yaml `
        --profile production `
        config --format json
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose config --format json exited with code $LASTEXITCODE"
    }
    [System.IO.File]::WriteAllLines(
        $modelPath,
        [string[]]$model,
        [System.Text.UTF8Encoding]::new($false)
    )
    Invoke-Checked node @('scripts/assert-compose-model.mjs', $modelPath)

    $model = & docker compose `
        -f compose.yaml `
        -f compose.discord.yaml `
        -f compose.catalog.spotify.yaml `
        -f compose.catalog.apple.yaml `
        -f compose.production.yaml `
        --profile production `
        config --format json
    if ($LASTEXITCODE -ne 0) {
        throw "provider docker compose config --format json exited with code $LASTEXITCODE"
    }
    [System.IO.File]::WriteAllLines(
        $modelPath,
        [string[]]$model,
        [System.Text.UTF8Encoding]::new($false)
    )
    Invoke-Checked node @(
        'scripts/assert-provider-compose-model.mjs', $modelPath, 'credentials'
    )

    $model = & docker compose `
        -f compose.yaml `
        -f compose.discord.yaml `
        -f compose.catalog.public-metadata.yaml `
        -f compose.production.yaml `
        --profile production `
        config --format json
    if ($LASTEXITCODE -ne 0) {
        throw "public-metadata docker compose config exited with code $LASTEXITCODE"
    }
    [System.IO.File]::WriteAllLines(
        $modelPath,
        [string[]]$model,
        [System.Text.UTF8Encoding]::new($false)
    )
    Invoke-Checked node @(
        'scripts/assert-provider-compose-model.mjs', $modelPath, 'public-metadata'
    )
}
finally {
    foreach ($name in $overrides.Keys) {
        [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process')
    }
    Remove-Item -LiteralPath $modelPath -Force -ErrorAction SilentlyContinue
    Pop-Location
}
