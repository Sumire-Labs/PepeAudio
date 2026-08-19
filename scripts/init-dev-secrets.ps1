[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$secretDirectory = Join-Path $PSScriptRoot '..\secrets'
$secretDirectory = [System.IO.Path]::GetFullPath($secretDirectory)
[System.IO.Directory]::CreateDirectory($secretDirectory) | Out-Null

function New-UrlSafeSecret {
    $bytes = [byte[]]::new(32)
    [System.Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
    return [Convert]::ToBase64String($bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_')
}

function Write-NewSecret([string]$Name, [string]$Value) {
    $path = Join-Path $secretDirectory $Name
    if (Test-Path -LiteralPath $path) {
        Write-Host "Keeping existing $Name"
        return Get-Content -Raw -LiteralPath $path
    }
    [System.IO.File]::WriteAllText($path, $Value, [System.Text.UTF8Encoding]::new($false))
    Write-Host "Created $Name"
    return $Value
}

$postgres = Write-NewSecret 'postgres_superuser_password.txt' (New-UrlSafeSecret)
$migrator = Write-NewSecret 'postgres_migrator_password.txt' (New-UrlSafeSecret)
$runtime = Write-NewSecret 'postgres_runtime_password.txt' (New-UrlSafeSecret)
$valkey = Write-NewSecret 'valkey_password.txt' (New-UrlSafeSecret)
Write-NewSecret 'component_signing_key.txt' (New-UrlSafeSecret) | Out-Null
Write-NewSecret 'database_migrator_url.txt' "postgres://pepeaudio_migrator:${migrator}@postgres:5432/pepeaudio" | Out-Null
Write-NewSecret 'database_runtime_url.txt' "postgres://pepeaudio_runtime:${runtime}@postgres:5432/pepeaudio" | Out-Null
Write-NewSecret 'valkey_url.txt' "redis://default:${valkey}@valkey:6379/0" | Out-Null

Write-Host 'Local service secrets are ready. Discord credentials were not created.'
