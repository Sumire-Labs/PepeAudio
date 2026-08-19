#!/bin/sh
set -eu

read_secret() {
    variable_name="$1"
    secret_path="$2"
    if [ -z "$secret_path" ]; then
        echo "missing secret file variable $variable_name" >&2
        exit 1
    fi
    if [ ! -r "$secret_path" ]; then
        echo "secret file configured by $variable_name is unreadable" >&2
        exit 1
    fi
    secret_value=$(sed -e 's/[[:space:]]*$//' "$secret_path")
    if [ -z "$secret_value" ]; then
        echo "secret file configured by $variable_name is empty" >&2
        exit 1
    fi
    printf '%s' "$secret_value"
}

runtime_password=$(read_secret \
    PEPEAUDIO_RUNTIME_DB_PASSWORD_FILE \
    "${PEPEAUDIO_RUNTIME_DB_PASSWORD_FILE:-}")
migrator_password=$(read_secret \
    PEPEAUDIO_MIGRATOR_DB_PASSWORD_FILE \
    "${PEPEAUDIO_MIGRATOR_DB_PASSWORD_FILE:-}")

psql --set ON_ERROR_STOP=1 \
    --username "$POSTGRES_USER" \
    --dbname "$POSTGRES_DB" \
    --set runtime_password="$runtime_password" \
    --set migrator_password="$migrator_password" <<-'EOSQL'
        CREATE ROLE pepeaudio_migrator LOGIN PASSWORD :'migrator_password';
        CREATE ROLE pepeaudio_runtime LOGIN PASSWORD :'runtime_password';
        ALTER DATABASE pepeaudio OWNER TO pepeaudio_migrator;
        GRANT CONNECT ON DATABASE pepeaudio TO pepeaudio_runtime;
EOSQL
