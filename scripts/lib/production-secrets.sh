#!/bin/sh

# Keep this list aligned with the file-backed secrets declared by the base,
# Discord, and production Compose models. Provider secrets stay in separate
# lists because their Compose overlays are optional. Each callback receives a
# logical name followed by the configured source path.
production_secret_sources() {
    "$1" postgres_superuser_password \
        "${PEPEAUDIO_POSTGRES_SUPERUSER_PASSWORD_SOURCE:-$2/secrets/postgres_superuser_password.txt}"
    "$1" postgres_runtime_password \
        "${PEPEAUDIO_POSTGRES_RUNTIME_PASSWORD_SOURCE:-$2/secrets/postgres_runtime_password.txt}"
    "$1" postgres_migrator_password \
        "${PEPEAUDIO_POSTGRES_MIGRATOR_PASSWORD_SOURCE:-$2/secrets/postgres_migrator_password.txt}"
    "$1" database_migrator_url \
        "${PEPEAUDIO_DATABASE_MIGRATOR_URL_SOURCE:-$2/secrets/database_migrator_url.txt}"
    "$1" database_runtime_url \
        "${PEPEAUDIO_DATABASE_RUNTIME_URL_SOURCE:-$2/secrets/database_runtime_url.txt}"
    "$1" valkey_password \
        "${PEPEAUDIO_VALKEY_PASSWORD_SOURCE:-$2/secrets/valkey_password.txt}"
    "$1" valkey_url \
        "${PEPEAUDIO_VALKEY_URL_SOURCE:-$2/secrets/valkey_url.txt}"
    "$1" discord_token \
        "${PEPEAUDIO_DISCORD_TOKEN_SOURCE:-$2/secrets/discord_token.txt}"
    "$1" component_signing_key \
        "${PEPEAUDIO_COMPONENT_SIGNING_KEY_SOURCE:-$2/secrets/component_signing_key.txt}"
    "$1" discord_client_secret \
        "${PEPEAUDIO_DISCORD_CLIENT_SECRET_SOURCE:-$2/secrets/discord_client_secret.txt}"
}

production_spotify_secret_sources() {
    "$1" spotify_client_secret \
        "${PEPEAUDIO_SPOTIFY_CLIENT_SECRET_SOURCE:-$2/secrets/spotify_client_secret.txt}"
}

production_apple_music_secret_sources() {
    "$1" apple_music_private_key \
        "${PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_SOURCE:-$2/secrets/apple_music_private_key.p8}"
}

production_secret_absolute_path() {
    case "$2" in
        /*)
            printf '%s\n' "$2"
            ;;
        *)
            printf '%s/%s\n' "$1" "$2"
            ;;
    esac
}
