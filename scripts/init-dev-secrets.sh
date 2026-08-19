#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
secret_dir="$script_dir/../secrets"
mkdir -p "$secret_dir"

new_secret() {
    openssl rand -base64 32 | tr '+/' '-_' | tr -d '=\n'
}

write_new_secret() {
    name="$1"
    value="$2"
    path="$secret_dir/$name"
    if [ -f "$path" ]; then
        printf 'Keeping existing %s\n' "$name" >&2
        sed -e 's/[[:space:]]*$//' "$path"
    else
        umask 077
        printf '%s' "$value" > "$path"
        printf 'Created %s\n' "$name" >&2
        printf '%s' "$value"
    fi
}

write_new_secret postgres_superuser_password.txt "$(new_secret)" >/dev/null
migrator=$(write_new_secret postgres_migrator_password.txt "$(new_secret)")
runtime=$(write_new_secret postgres_runtime_password.txt "$(new_secret)")
valkey=$(write_new_secret valkey_password.txt "$(new_secret)")
write_new_secret component_signing_key.txt "$(new_secret)" >/dev/null
write_new_secret database_migrator_url.txt "postgres://pepeaudio_migrator:${migrator}@postgres:5432/pepeaudio" >/dev/null
write_new_secret database_runtime_url.txt "postgres://pepeaudio_runtime:${runtime}@postgres:5432/pepeaudio" >/dev/null
write_new_secret valkey_url.txt "redis://default:${valkey}@valkey:6379/0" >/dev/null

printf '%s\n' 'Local service secrets are ready. Discord credentials were not created.'
