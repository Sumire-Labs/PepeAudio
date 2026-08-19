[CmdletBinding()]
param(
    [switch]$WithDockerIntegration,
    [switch]$KeepDockerServices,
    [switch]$SkipDockerConfig
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$webRoot = Join-Path $repositoryRoot 'web'
$secretDirectory = Join-Path $repositoryRoot 'secrets'
$composeProject = "pepeaudio-verify-$PID"
$dockerStarted = $false
$dockerCacheVolumes = @()
$temporaryOAuthSecret = $null

function Invoke-Checked {
    param([Parameter(Mandatory)][string]$Executable, [string[]]$Arguments = @())

    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Executable exited with code $LASTEXITCODE"
    }
}

function Assert-Command {
    param([Parameter(Mandatory)][string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found on PATH."
    }
}

function Assert-Toolchain {
    Assert-Command cargo
    Assert-Command rustc
    Assert-Command node
    Assert-Command pnpm

    $rustVersion = (& rustc --version).Trim()
    if ($LASTEXITCODE -ne 0 -or $rustVersion -notmatch '^rustc 1\.97\.0\b') {
        throw "Rust 1.97.0 is required; found '$rustVersion'."
    }
    $nodeVersion = (& node --version).Trim()
    if ($LASTEXITCODE -ne 0 -or $nodeVersion -notmatch '^v24\.') {
        throw "Node.js 24.x is required; found '$nodeVersion'."
    }
    $pnpmVersion = (& pnpm --version).Trim()
    if ($LASTEXITCODE -ne 0 -or $pnpmVersion -ne '11.3.0') {
        throw "pnpm 11.3.0 is required; found '$pnpmVersion'."
    }

    Write-Host "Repository: $repositoryRoot"
    Invoke-Checked rustc @('--version')
    Invoke-Checked cargo @('--version')
    Write-Host "node $nodeVersion"
    Write-Host "pnpm $pnpmVersion"
}

function Invoke-RustVerification {
    Write-Host '== Rust formatting =='
    Invoke-Checked cargo @('fmt', '--all', '--', '--check')
    Write-Host '== Rust tests =='
    Invoke-Checked cargo @('test', '--workspace', '--all-targets', '--locked')
    Write-Host '== Rust Clippy =='
    Invoke-Checked cargo @(
        'clippy', '--workspace', '--all-targets', '--all-features', '--locked',
        '--', '-D', 'warnings'
    )
}

function Invoke-ReleaseContractVerification {
    Write-Host '== Release version contract =='
    Invoke-Checked node @('--test', 'scripts/verify-release-tag.test.mjs')
    Invoke-Checked node @('scripts/verify-release-tag.mjs')
}

function Invoke-MediaRuntimeVerification {
    Assert-Command ffmpeg
    Assert-Command ffprobe
    Write-Host '== Real FFmpeg/ffprobe adapter smoke tests =='
    Invoke-Checked cargo @(
        'test', '-p', 'pepeaudio-media', '--test', 'ffmpeg_smoke', '--locked',
        'probes_and_decodes_a_generated_audio_fixture', '--', '--ignored', '--exact'
    )
    Invoke-Checked cargo @(
        'test', '-p', 'pepeaudio-pipeline', '--lib', '--locked',
        'decoder::tests::installed_ffmpeg_decodes_f32_pcm_and_reaps',
        '--', '--ignored', '--exact'
    )
}

function Invoke-WebVerification {
    Write-Host '== Web frozen install, type check, tests, and build =='
    Push-Location $webRoot
    try {
        Invoke-Checked pnpm @('install', '--frozen-lockfile')
        Invoke-Checked pnpm @('check')
        Invoke-Checked pnpm @('test')
        Invoke-Checked pnpm @('build')
    }
    finally {
        Pop-Location
    }
}

function Invoke-LicenseVerification {
    Write-Host '== First-party and distributed dependency licenses =='
    & (Join-Path $PSScriptRoot 'verify-licenses.ps1')
    if (-not $?) {
        throw 'License verification failed.'
    }
}

function Invoke-ComposeConfigVerification {
    if ($SkipDockerConfig) {
        Write-Host 'Skipping Docker Compose config validation by request.'
        return
    }

    Assert-Command docker
    & (Join-Path $PSScriptRoot 'verify-compose.ps1')
    if (-not $?) {
        throw 'Docker Compose model verification failed.'
    }
}

function Initialize-IntegrationSecrets {
    $requiredFiles = @(
        'postgres_superuser_password.txt',
        'postgres_runtime_password.txt',
        'postgres_migrator_password.txt',
        'database_migrator_url.txt',
        'database_runtime_url.txt',
        'valkey_password.txt',
        'valkey_url.txt'
    )
    $missing = $requiredFiles | Where-Object {
        -not (Test-Path -LiteralPath (Join-Path $secretDirectory $_))
    }
    if (@($missing).Count -gt 0) {
        Write-Host 'Creating missing local integration secrets without replacing existing files.'
        & (Join-Path $PSScriptRoot 'init-dev-secrets.ps1')
        if (-not $?) {
            throw 'Local integration secret initialization failed.'
        }
    }
}

function Invoke-DockerIntegration {
    if (-not $WithDockerIntegration) {
        return
    }

    Assert-Command docker
    Invoke-Checked docker @('info', '--format', '{{.ServerVersion}}')
    Initialize-IntegrationSecrets

    Write-Host "== Docker integration project: $composeProject =="
    $script:dockerStarted = $true
    Invoke-Checked docker @(
        'compose',
        '--project-name',
        $composeProject,
        'up',
        '--detach',
        '--wait',
        'postgres',
        'valkey'
    )
    Invoke-Checked docker @(
        'compose',
        '--project-name',
        $composeProject,
        'build',
        'migrate',
        'api',
        'caddy'
    )
    Invoke-Checked docker @(
        'compose',
        '--project-name',
        $composeProject,
        '-f',
        'compose.yaml',
        '-f',
        'compose.discord.yaml',
        '--profile',
        'discord',
        'build',
        'bot'
    )
    Invoke-Checked docker @(
        'compose',
        '--project-name',
        $composeProject,
        'run',
        '--rm',
        '--no-deps',
        'migrate'
    )

    $cargoRegistryVolume = "$composeProject-cargo-registry"
    $cargoGitVolume = "$composeProject-cargo-git"
    foreach ($volume in @($cargoRegistryVolume, $cargoGitVolume)) {
        Invoke-Checked docker @(
            'volume',
            'create',
            '--label',
            "pepeaudio.verify.project=$composeProject",
            $volume
        )
        $script:dockerCacheVolumes += $volume
    }
    Invoke-Checked docker @(
        'run',
        '--rm',
        '--env',
        'RUSTUP_TOOLCHAIN=1.97.0',
        '--mount',
        "type=bind,source=$repositoryRoot,target=/workspace,readonly",
        '--mount',
        "type=volume,source=$cargoRegistryVolume,target=/usr/local/cargo/registry",
        '--mount',
        "type=volume,source=$cargoGitVolume,target=/usr/local/cargo/git",
        '--workdir',
        '/workspace',
        'rust:1.97.0-bookworm',
        'cargo',
        'fetch',
        '--locked'
    )

    $networkName = "${composeProject}_data"
    $containerScript = @'
export CARGO_TARGET_DIR=/workspace-target
sh scripts/run-live-dependency-tests.sh
# Keep PowerShell's native-pipeline CR outside the final command argument.
'@
    $dockerArguments = @(
        'run',
        '--rm',
        '--interactive',
        '--env',
        'RUSTUP_TOOLCHAIN=1.97.0',
        '--network',
        $networkName,
        '--mount',
        "type=bind,source=$repositoryRoot,target=/workspace,readonly",
        '--mount',
        "type=bind,source=$secretDirectory,target=/run/pepeaudio-secrets,readonly",
        '--mount',
        "type=volume,source=$cargoRegistryVolume,target=/usr/local/cargo/registry",
        '--mount',
        "type=volume,source=$cargoGitVolume,target=/usr/local/cargo/git",
        '--mount',
        'type=volume,target=/workspace-target',
        '--workdir',
        '/workspace',
        'rust:1.97.0-bookworm',
        'sh',
        '-eu'
    )
    $containerScript | & docker @dockerArguments
    if ($LASTEXITCODE -ne 0) {
        throw "Docker live storage tests exited with code $LASTEXITCODE"
    }

    $script:temporaryOAuthSecret = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        "pepeaudio-oauth-smoke-$PID.txt"
    $placeholder = [Convert]::ToHexString(
        [System.Security.Cryptography.RandomNumberGenerator]::GetBytes(32)
    )
    [System.IO.File]::WriteAllText(
        $temporaryOAuthSecret,
        $placeholder,
        [System.Text.UTF8Encoding]::new($false)
    )
    $productionEnvironment = @{
        PEPEAUDIO_DOMAIN = 'audio.example.test'
        PEPEAUDIO_DISCORD_CLIENT_ID = '100000000000000002'
        PEPEAUDIO_VALKEY_KEYSPACE = 'pepeaudio-production'
        PEPEAUDIO_DISCORD_CLIENT_SECRET_SOURCE = $temporaryOAuthSecret
    }
    $previousEnvironment = @{}
    foreach ($name in $productionEnvironment.Keys) {
        $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
        [Environment]::SetEnvironmentVariable($name, $productionEnvironment[$name], 'Process')
    }
    try {
        $productionCompose = @(
            'compose', '--project-name', $composeProject,
            '-f', 'compose.yaml', '-f', 'compose.discord.yaml',
            '-f', 'compose.production.yaml', '--profile', 'production'
        )
        Invoke-Checked docker ($productionCompose + @(
            'up', '--detach', '--wait', '--no-build', 'api'
        ))
        $smokeScript = (Get-Content -Raw `
            (Join-Path $PSScriptRoot 'smoke-production-api.sh')).Replace("`r`n", "`n")
        # A PowerShell string pipeline appends CRLF. Since the source already
        # ends in LF, keep the otherwise standalone CR inside a shell comment.
        $smokeScript = $smokeScript.TrimEnd("`r", "`n") + `
            "`n# PowerShell native-pipeline terminator"
        $smokeScript |
            & docker @($productionCompose + @('exec', '-T', 'api', 'sh', '-s'))
        if ($LASTEXITCODE -ne 0) {
            throw "Production API smoke exited with code $LASTEXITCODE"
        }
    }
    finally {
        foreach ($name in $productionEnvironment.Keys) {
            [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], 'Process')
        }
    }
}

Push-Location $repositoryRoot
try {
    Assert-Toolchain
    Invoke-ReleaseContractVerification
    Invoke-RustVerification
    Invoke-MediaRuntimeVerification
    Invoke-WebVerification
    Invoke-LicenseVerification
    Invoke-ComposeConfigVerification
    Invoke-DockerIntegration
    Write-Host 'All requested verification stages passed.'
}
finally {
    Pop-Location
    if ($dockerStarted -and -not $KeepDockerServices) {
        Write-Host "Removing isolated Docker project $composeProject and its test volumes."
        & docker compose `
            --project-directory $repositoryRoot `
            --project-name $composeProject `
            down --volumes --remove-orphans
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "Docker cleanup for $composeProject failed with code $LASTEXITCODE."
        }
        if ($dockerCacheVolumes.Count -gt 0) {
            & docker volume rm @dockerCacheVolumes
            if ($LASTEXITCODE -ne 0) {
                Write-Warning "Cargo cache cleanup for $composeProject failed."
            }
        }
    }
    elseif ($dockerStarted) {
        Write-Warning "Docker test project was kept: $composeProject"
        if ($dockerCacheVolumes.Count -gt 0) {
            Write-Warning "Docker test cache volumes were kept: $dockerCacheVolumes"
        }
    }
    if ($temporaryOAuthSecret) {
        Remove-Item -LiteralPath $temporaryOAuthSecret -Force -ErrorAction SilentlyContinue
    }
}
