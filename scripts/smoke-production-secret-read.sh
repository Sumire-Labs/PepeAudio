#!/bin/sh
set -eu

usage() {
    cat <<'EOF'
Usage: sh scripts/smoke-production-secret-read.sh [--public-metadata]
       sh scripts/smoke-production-secret-read.sh [--spotify] [--apple-music]

Verify every selected production Compose service can read only its file-backed
secrets under the supported Ubuntu/rootful-Docker ownership contract. Select
the keyless public-metadata overlay or the credential provider overlays used by
the deployment.
Secret values are never printed and infrastructure services are not started.
EOF
}

include_spotify=false
include_apple_music=false
include_public_metadata=false
while [ "$#" -gt 0 ]; do
    case "$1" in
        --public-metadata)
            include_public_metadata=true
            ;;
        --spotify)
            include_spotify=true
            ;;
        --apple-music)
            include_apple_music=true
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            exit 2
            ;;
    esac
    shift
done
if [ "$include_public_metadata" = true ]; then
    if [ "$include_spotify" = true ] || [ "$include_apple_music" = true ]; then
        printf '%s\n' \
            '--public-metadata cannot be combined with credential provider options.' >&2
        exit 2
    fi
fi

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
. "$script_dir/lib/production-secrets.sh"
runtime_gid=${PEPEAUDIO_RUNTIME_GID:-10001}

case "$runtime_gid" in
    ''|*[!0-9]*)
        printf '%s\n' 'PEPEAUDIO_RUNTIME_GID must be a positive numeric GID.' >&2
        exit 2
        ;;
esac
if [ "$runtime_gid" -eq 0 ] || [ "$runtime_gid" -gt 2147483647 ]; then
    printf '%s\n' 'PEPEAUDIO_RUNTIME_GID must be between 1 and 2147483647.' >&2
    exit 2
fi
if [ "$(uname -s)" != Linux ]; then
    printf '%s\n' \
        'This smoke requires the supported Linux rootful Docker ownership model.' >&2
    exit 1
fi
if ! command -v docker >/dev/null 2>&1; then
    printf '%s\n' 'docker is required for the production secret smoke.' >&2
    exit 1
fi

daemon_os=$(docker info --format '{{.OSType}}')
operating_system=$(docker info --format '{{.OperatingSystem}}')
security_options=$(docker info --format '{{range .SecurityOptions}}{{println .}}{{end}}')
if [ "$daemon_os" != linux ]; then
    printf '%s\n' 'The production secret smoke requires a Linux Docker daemon.' >&2
    exit 1
fi
if printf '%s\n' "$operating_system" | grep -iq 'docker desktop'; then
    printf '%s\n' \
        'Docker Desktop is outside the production secret ownership contract.' >&2
    exit 1
fi
if printf '%s\n' "$security_options" | grep -Eiq 'rootless|userns'; then
    printf '%s\n' \
        'Rootless Docker and user-namespace remapping are outside this ownership contract.' >&2
    exit 1
fi

cd "$repository_root"
check_selected_secrets() {
    set -- --check
    if [ "$include_spotify" = true ]; then
        set -- "$@" --spotify
    fi
    if [ "$include_apple_music" = true ]; then
        set -- "$@" --apple-music
    fi
    sh "$script_dir/prepare-production-secrets.sh" "$@"
}

selected_secret_sources() {
    production_secret_sources "$1" "$2"
    if [ "$include_spotify" = true ]; then
        production_spotify_secret_sources "$1" "$2"
    fi
    if [ "$include_apple_music" = true ]; then
        production_apple_music_secret_sources "$1" "$2"
    fi
}

check_selected_secrets

negative_image=${PEPEAUDIO_SECRET_PROBE_IMAGE:-${PEPEAUDIO_API_IMAGE:-pepeaudio-api:local}}
docker image inspect "$negative_image" >/dev/null
if [ "$runtime_gid" -eq 65534 ]; then
    unrelated_gid=65533
else
    unrelated_gid=65534
fi

deny_unrelated_user() {
    label=$1
    path=$(production_secret_absolute_path "$repository_root" "$2")

    docker run --rm \
        --network none \
        --read-only \
        --cap-drop ALL \
        --security-opt no-new-privileges \
        --user "65534:$unrelated_gid" \
        --mount \
            "type=bind,source=$path,target=/run/pepeaudio-secret-probe,readonly" \
        --env "PEPEAUDIO_EXPECTED_RUNTIME_GID=$runtime_gid" \
        --env "PEPEAUDIO_UNRELATED_GID=$unrelated_gid" \
        --entrypoint /bin/sh \
        "$negative_image" \
        -euc '
            secret=/run/pepeaudio-secret-probe
            test "$(id -u)" = 65534
            test "$(id -g)" = "$PEPEAUDIO_UNRELATED_GID"
            case " $(id -G) " in
                *" $PEPEAUDIO_EXPECTED_RUNTIME_GID "*) exit 20 ;;
            esac
            test "$(stat -c "%u:%g:%a" -- "$secret")" = \
                "0:$PEPEAUDIO_EXPECTED_RUNTIME_GID:440"
            test ! -r "$secret"
            if head -c 1 "$secret" >/dev/null 2>&1; then
                exit 21
            fi
        '

    printf 'Unrelated-user denial passed for %s\n' "$label"
}

printf '%s\n' 'Checking raw host binds without the secret-reader group.'
selected_secret_sources deny_unrelated_user "$repository_root"

export PEPEAUDIO_RUNTIME_GID="$runtime_gid"
export PEPEAUDIO_DOMAIN=${PEPEAUDIO_DOMAIN:-audio.example.test}
export PEPEAUDIO_DISCORD_CLIENT_ID=${PEPEAUDIO_DISCORD_CLIENT_ID:-100000000000000002}
export PEPEAUDIO_VALKEY_KEYSPACE=${PEPEAUDIO_VALKEY_KEYSPACE:-pepeaudio-production}

compose() {
    if [ "$include_public_metadata" = true ]; then
        docker compose -f compose.yaml -f compose.discord.yaml \
            -f compose.catalog.public-metadata.yaml -f compose.production.yaml \
            --profile production "$@"
    elif [ "$include_spotify" = true ] && [ "$include_apple_music" = true ]; then
        docker compose -f compose.yaml -f compose.discord.yaml \
            -f compose.catalog.spotify.yaml -f compose.catalog.apple.yaml \
            -f compose.production.yaml --profile production "$@"
    elif [ "$include_spotify" = true ]; then
        docker compose -f compose.yaml -f compose.discord.yaml \
            -f compose.catalog.spotify.yaml -f compose.production.yaml \
            --profile production "$@"
    elif [ "$include_apple_music" = true ]; then
        docker compose -f compose.yaml -f compose.discord.yaml \
            -f compose.catalog.apple.yaml -f compose.production.yaml \
            --profile production "$@"
    else
        docker compose -f compose.yaml -f compose.discord.yaml \
            -f compose.production.yaml --profile production "$@"
    fi
}

probe=' 
    expected_uid=$1
    shift
    test "$(id -u)" = "$expected_uid"
    case " $(id -G) " in
        *" $PEPEAUDIO_RUNTIME_GID "*) ;;
        *) exit 20 ;;
    esac
    test "$(grep "^CapEff:" /proc/self/status | cut -f2)" = \
        "0000000000000000"
    for secret in "$@"; do
        test "$(stat -c "%u:%g:%a" -- "$secret")" = \
            "0:$PEPEAUDIO_RUNTIME_GID:440"
        test -r "$secret"
        test ! -w "$secret"
        test -s "$secret"
        head -c 1 "$secret" >/dev/null
    done
'

printf '%s\n' 'Checking PostgreSQL secret access after gosu privilege drop.'
compose run --rm --no-deps postgres \
    -e PEPEAUDIO_RUNTIME_GID="$runtime_gid" \
    gosu postgres sh -euc "$probe" -- 999 \
    /run/secrets/postgres_superuser_password \
    /run/secrets/postgres_runtime_password \
    /run/secrets/postgres_migrator_password

printf '%s\n' 'Checking non-root Valkey secret access.'
compose run --rm --no-deps \
    -e PEPEAUDIO_RUNTIME_GID="$runtime_gid" \
    -e PEPEAUDIO_SECRET_CONSUMER_ENTRYPOINT=/bin/sh \
    -e PEPEAUDIO_SECRET_CHOWN_DIRECTORY= \
    valkey -euc "$probe" -- 999 /run/secrets/valkey_password

printf '%s\n' 'Checking migration secret access.'
compose run --rm --no-deps -e PEPEAUDIO_RUNTIME_GID="$runtime_gid" \
    --entrypoint /bin/sh migrate \
    -euc "$probe" -- 10001 /run/secrets/database_migrator_url

printf '%s\n' 'Checking API secret access.'
compose run --rm --no-deps -e PEPEAUDIO_RUNTIME_GID="$runtime_gid" \
    --entrypoint /bin/sh api \
    -euc "$probe" -- 10001 \
    /run/secrets/database_runtime_url \
    /run/secrets/valkey_url \
    /run/secrets/discord_client_secret

printf '%s\n' 'Checking Bot secret access.'
set -- \
    /run/secrets/discord_token \
    /run/secrets/component_signing_key \
    /run/secrets/database_runtime_url \
    /run/secrets/valkey_url
if [ "$include_spotify" = true ]; then
    set -- "$@" /run/secrets/spotify_client_secret
fi
if [ "$include_apple_music" = true ]; then
    set -- "$@" /run/secrets/apple_music_private_key
fi
compose run --rm --no-deps -e PEPEAUDIO_RUNTIME_GID="$runtime_gid" \
    --entrypoint /bin/sh bot \
    -euc "$probe" -- 10001 "$@"

printf 'All production consumers passed the root:%s mode 0440 secret-read smoke.\n' \
    "$runtime_gid"
