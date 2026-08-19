#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
model_path=$(mktemp "${TMPDIR:-/tmp}/pepeaudio-compose.XXXXXX")

cleanup() {
    rm -f "$model_path"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

cd "$repository_root"
export PEPEAUDIO_RUNTIME_GID=10001
docker compose version
docker compose \
    -f compose.yaml \
    --profile development-api \
    config --quiet
docker compose \
    -f compose.yaml \
    -f compose.discord.yaml \
    --profile development-api \
    --profile discord \
    config --quiet

PEPEAUDIO_SPOTIFY_CLIENT_ID=compose-contract-client-id \
PEPEAUDIO_SPOTIFY_CLIENT_SECRET_SOURCE=./secrets/spotify_client_secret.txt.example \
    docker compose \
        -f compose.yaml \
        -f compose.discord.yaml \
        -f compose.catalog.spotify.yaml \
        --profile discord \
        config --quiet

PEPEAUDIO_APPLE_MUSIC_TEAM_ID=ABCDE12345 \
PEPEAUDIO_APPLE_MUSIC_KEY_ID=KEY1234567 \
PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_SOURCE=./secrets/apple_music_private_key.p8.example \
    docker compose \
        -f compose.yaml \
        -f compose.discord.yaml \
        -f compose.catalog.apple.yaml \
        --profile discord \
        config --quiet

PEPEAUDIO_DOMAIN=audio.example.test \
PEPEAUDIO_DISCORD_CLIENT_ID=100000000000000002 \
PEPEAUDIO_VALKEY_KEYSPACE=pepeaudio-production \
PEPEAUDIO_SHARD_TOTAL=4 \
PEPEAUDIO_DISCORD_CLIENT_SECRET_SOURCE=./secrets/discord_client_secret.txt.example \
    docker compose \
        -f compose.yaml \
        -f compose.discord.yaml \
        -f compose.production.yaml \
        --profile production \
        config --quiet

PEPEAUDIO_DOMAIN=audio.example.test \
PEPEAUDIO_DISCORD_CLIENT_ID=100000000000000002 \
PEPEAUDIO_VALKEY_KEYSPACE=pepeaudio-production \
PEPEAUDIO_SHARD_TOTAL=4 \
PEPEAUDIO_DISCORD_CLIENT_SECRET_SOURCE=./secrets/discord_client_secret.txt.example \
    docker compose \
        -f compose.yaml \
        -f compose.discord.yaml \
        -f compose.production.yaml \
        --profile production \
        config --format json > "$model_path"

node scripts/assert-compose-model.mjs "$model_path"
