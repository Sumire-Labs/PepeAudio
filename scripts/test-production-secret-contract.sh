#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
volume="pepeaudio-secret-contract-test-$$"
volume_created=0

cleanup() {
    if [ "$volume_created" -eq 1 ]; then
        docker volume rm "$volume" >/dev/null
    fi
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

docker volume create "$volume" >/dev/null
volume_created=1

MSYS_NO_PATHCONV=1 docker run --rm --network none \
    --mount "type=volume,source=$volume,target=/workspace/secrets" \
    --mount "type=bind,source=$script_dir,target=/workspace/scripts,readonly" \
    debian:bookworm-slim sh -euc '
        for name in \
            postgres_superuser_password.txt \
            postgres_runtime_password.txt \
            postgres_migrator_password.txt \
            database_migrator_url.txt \
            database_runtime_url.txt \
            valkey_password.txt \
            valkey_url.txt \
            discord_token.txt \
            component_signing_key.txt
        do
            umask 077
            printf x > "/workspace/secrets/$name"
        done

        if sh /workspace/scripts/prepare-production-secrets.sh >/dev/null 2>&1; then
            echo "prepare accepted a missing secret" >&2
            exit 1
        fi
        test "$(stat --printf "%u:%g:%a" \
            /workspace/secrets/database_runtime_url.txt)" = "0:0:600"

        printf x > /workspace/secrets/discord_client_secret.txt
        chmod 0600 /workspace/secrets/discord_client_secret.txt
        sh /workspace/scripts/prepare-production-secrets.sh >/dev/null
        sh /workspace/scripts/prepare-production-secrets.sh --check >/dev/null

        chmod 0644 /workspace/secrets/database_runtime_url.txt
        if sh /workspace/scripts/prepare-production-secrets.sh \
            --check >/dev/null 2>&1
        then
            echo "check accepted mode 0644" >&2
            exit 1
        fi
    '

printf '%s\n' 'Production secret preparation contract tests passed.'
