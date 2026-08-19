#!/bin/sh
set -eu

secret_root=${PEPEAUDIO_TEST_SECRET_ROOT:-/run/pepeaudio-secrets}
database_file="$secret_root/database_migrator_url.txt"
valkey_file="$secret_root/valkey_url.txt"

test -r "$database_file"
test -r "$valkey_file"

PEPEAUDIO_TEST_DATABASE_URL="$(tr -d '\r\n' < "$database_file")"
PEPEAUDIO_TEST_VALKEY_URL="$(tr -d '\r\n' < "$valkey_file")"
export PEPEAUDIO_TEST_DATABASE_URL PEPEAUDIO_TEST_VALKEY_URL

cargo test --locked --offline -p pepeaudio-storage \
    --test postgres_integration -- --ignored --test-threads=1
cargo test --locked --offline -p pepeaudio-storage \
    --test valkey_integration -- --ignored --test-threads=1
cargo test --locked --offline -p pepeaudio-auth \
    --test valkey_live -- --ignored --test-threads=1

unset PEPEAUDIO_TEST_DATABASE_URL PEPEAUDIO_TEST_VALKEY_URL
