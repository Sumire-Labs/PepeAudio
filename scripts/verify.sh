#!/bin/sh
set -eu

with_docker_integration=0
keep_docker_services=0
skip_docker_config=0

usage() {
    cat <<'EOF'
Usage: scripts/verify.sh [options]

Options:
  --with-docker-integration  Start isolated PostgreSQL/Valkey services, run
                             migrations, and execute ignored live storage tests.
  --keep-docker-services     Keep that isolated Compose project after testing.
  --skip-docker-config       Skip development and production Compose checks.
  -h, --help                 Show this help.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --with-docker-integration)
            with_docker_integration=1
            ;;
        --keep-docker-services)
            keep_docker_services=1
            ;;
        --skip-docker-config)
            skip_docker_config=1
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown option: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
web_root="$repository_root/web"
secret_directory="$repository_root/secrets"
compose_project="pepeaudio-verify-$$"
docker_started=0
cargo_registry_created=0
cargo_git_created=0
cargo_registry_volume="${compose_project}-cargo-registry"
cargo_git_volume="${compose_project}-cargo-git"
oauth_secret_path=''

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf "Required command '%s' was not found on PATH.\n" "$1" >&2
        exit 1
    fi
}

cleanup() {
    status=$?
    if [ "$docker_started" -eq 1 ] && [ "$keep_docker_services" -eq 0 ]; then
        printf 'Removing isolated Docker project %s and its test volumes.\n' \
            "$compose_project"
        docker compose --project-name "$compose_project" \
            down --volumes --remove-orphans || \
            printf 'Warning: Docker cleanup for %s failed.\n' "$compose_project" >&2
        if [ "$cargo_registry_created" -eq 1 ]; then
            docker volume rm "$cargo_registry_volume" >/dev/null || \
                printf 'Warning: Cargo registry volume cleanup failed.\n' >&2
        fi
        if [ "$cargo_git_created" -eq 1 ]; then
            docker volume rm "$cargo_git_volume" >/dev/null || \
                printf 'Warning: Cargo Git volume cleanup failed.\n' >&2
        fi
    elif [ "$docker_started" -eq 1 ]; then
        printf 'Warning: Docker test project was kept: %s\n' \
            "$compose_project" >&2
    fi
    if [ -n "$oauth_secret_path" ]; then
        rm -f "$oauth_secret_path"
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

assert_toolchain() {
    require_command cargo
    require_command rustc
    require_command node
    require_command pnpm

    rust_version=$(rustc --version)
    case "$rust_version" in
        'rustc 1.97.0 '*) ;;
        *)
            printf 'Rust 1.97.0 is required; found %s.\n' "$rust_version" >&2
            exit 1
            ;;
    esac

    node_version=$(node --version)
    case "$node_version" in
        v24.*) ;;
        *)
            printf 'Node.js 24.x is required; found %s.\n' "$node_version" >&2
            exit 1
            ;;
    esac

    pnpm_version=$(pnpm --version)
    if [ "$pnpm_version" != '11.3.0' ]; then
        printf 'pnpm 11.3.0 is required; found %s.\n' "$pnpm_version" >&2
        exit 1
    fi

    printf 'Repository: %s\n' "$repository_root"
    rustc --version
    cargo --version
    printf 'node %s\n' "$node_version"
    printf 'pnpm %s\n' "$pnpm_version"
}

verify_release_contract() {
    printf '%s\n' '== Release version contract =='
    node --test scripts/verify-release-tag.test.mjs
    node scripts/verify-release-tag.mjs
}

verify_rust() {
    printf '%s\n' '== Rust formatting =='
    cargo fmt --all -- --check
    printf '%s\n' '== Rust tests =='
    cargo test --workspace --all-targets --locked
    printf '%s\n' '== Rust Clippy =='
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
}

verify_media_runtime() {
    require_command ffmpeg
    require_command ffprobe
    printf '%s\n' '== Real FFmpeg/ffprobe adapter smoke tests =='
    cargo test -p pepeaudio-media --test ffmpeg_smoke --locked \
        probes_and_decodes_a_generated_audio_fixture -- \
        --ignored --exact
    cargo test -p pepeaudio-pipeline --lib --locked \
        decoder::tests::installed_ffmpeg_decodes_f32_pcm_and_reaps -- \
        --ignored --exact
}

verify_web() {
    printf '%s\n' '== Web frozen install, type check, tests, and build =='
    (
        cd "$web_root"
        pnpm install --frozen-lockfile
        pnpm check
        pnpm test
        pnpm build
    )
}

verify_licenses() {
    printf '%s\n' '== First-party and distributed dependency licenses =='
    sh "$script_dir/verify-licenses.sh"
}

verify_compose_config() {
    if [ "$skip_docker_config" -eq 1 ]; then
        printf '%s\n' 'Skipping Docker Compose config validation by request.'
        return
    fi

    require_command docker
    sh "$script_dir/verify-compose.sh"
}

initialize_integration_secrets() {
    missing=0
    for name in \
        postgres_superuser_password.txt \
        postgres_runtime_password.txt \
        postgres_migrator_password.txt \
        database_migrator_url.txt \
        database_runtime_url.txt \
        valkey_password.txt \
        valkey_url.txt
    do
        if [ ! -f "$secret_directory/$name" ]; then
            missing=1
        fi
    done

    if [ "$missing" -eq 1 ]; then
        printf '%s\n' \
            'Creating missing local integration secrets without replacing existing files.'
        "$script_dir/init-dev-secrets.sh"
    fi
}

run_docker_integration() {
    if [ "$with_docker_integration" -eq 0 ]; then
        return
    fi

    require_command docker
    docker info --format '{{.ServerVersion}}' >/dev/null
    initialize_integration_secrets

    printf '== Docker integration project: %s ==\n' "$compose_project"
    docker_started=1
    docker compose --project-name "$compose_project" \
        up --detach --wait postgres valkey
    docker compose --project-name "$compose_project" build migrate api caddy
    docker compose --project-name "$compose_project" \
        -f compose.yaml \
        -f compose.discord.yaml \
        --profile discord \
        build bot
    docker compose --project-name "$compose_project" \
        run --rm --no-deps migrate

    docker volume create \
        --label "pepeaudio.verify.project=$compose_project" \
        "$cargo_registry_volume" >/dev/null
    cargo_registry_created=1
    docker volume create \
        --label "pepeaudio.verify.project=$compose_project" \
        "$cargo_git_volume" >/dev/null
    cargo_git_created=1
    docker run --rm \
        --env RUSTUP_TOOLCHAIN=1.97.0 \
        --mount "type=bind,source=$repository_root,target=/workspace,readonly" \
        --mount \
            "type=volume,source=$cargo_registry_volume,target=/usr/local/cargo/registry" \
        --mount \
            "type=volume,source=$cargo_git_volume,target=/usr/local/cargo/git" \
        --workdir /workspace \
        rust:1.97.0-bookworm \
        cargo fetch --locked

    network_name="${compose_project}_data"
    docker run --rm \
        --env RUSTUP_TOOLCHAIN=1.97.0 \
        --network "$network_name" \
        --mount "type=bind,source=$repository_root,target=/workspace,readonly" \
        --mount \
            "type=bind,source=$secret_directory,target=/run/pepeaudio-secrets,readonly" \
        --mount \
            "type=volume,source=$cargo_registry_volume,target=/usr/local/cargo/registry" \
        --mount \
            "type=volume,source=$cargo_git_volume,target=/usr/local/cargo/git" \
        --mount type=volume,target=/workspace-target \
        --workdir /workspace \
        rust:1.97.0-bookworm \
        sh -euc '
            export CARGO_TARGET_DIR=/workspace-target
            sh scripts/run-live-dependency-tests.sh
        '

    oauth_secret_path=$(mktemp "${TMPDIR:-/tmp}/pepeaudio-oauth-smoke.XXXXXX")
    umask 077
    printf '%s' 'local-integration-oauth-secret-not-for-production' > "$oauth_secret_path"
    PEPEAUDIO_DOMAIN=audio.example.test \
    PEPEAUDIO_DISCORD_CLIENT_ID=100000000000000002 \
    PEPEAUDIO_VALKEY_KEYSPACE=pepeaudio-production \
    PEPEAUDIO_DISCORD_CLIENT_SECRET_SOURCE="$oauth_secret_path" \
        docker compose \
            --project-name "$compose_project" \
            -f compose.yaml \
            -f compose.discord.yaml \
            -f compose.production.yaml \
            --profile production \
            up --detach --wait --no-build api
    PEPEAUDIO_DOMAIN=audio.example.test \
    PEPEAUDIO_DISCORD_CLIENT_ID=100000000000000002 \
    PEPEAUDIO_VALKEY_KEYSPACE=pepeaudio-production \
    PEPEAUDIO_DISCORD_CLIENT_SECRET_SOURCE="$oauth_secret_path" \
        docker compose \
            --project-name "$compose_project" \
            -f compose.yaml \
            -f compose.discord.yaml \
            -f compose.production.yaml \
            --profile production \
            exec -T api sh -s < scripts/smoke-production-api.sh
}

cd "$repository_root"
assert_toolchain
verify_release_contract
verify_rust
verify_media_runtime
verify_web
verify_licenses
verify_compose_config
run_docker_integration
printf '%s\n' 'All requested verification stages passed.'
