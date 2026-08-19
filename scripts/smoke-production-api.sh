#!/bin/sh
set -eu

base_url=${PEPEAUDIO_SMOKE_API_URL:-http://127.0.0.1:3000}
headers=$(mktemp "${TMPDIR:-/tmp}/pepeaudio-auth-headers.XXXXXX")

cleanup() {
    rm -f "$headers"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

test "$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' "$base_url/health/live")" = 200
test "$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' "$base_url/health/ready")" = 200
test "$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' "$base_url/auth/session")" = 401
test "$(curl --silent --show-error --dump-header "$headers" --output /dev/null --write-out '%{http_code}' "$base_url/auth/login")" = 303
grep -Eiq '^location: https://discord\.com/oauth2/authorize\?' "$headers"
grep -Eiq '^set-cookie: __Host-pepeaudio_oauth_state=.*; Path=/; Max-Age=[0-9]+; Secure; HttpOnly; SameSite=Lax' "$headers"

printf '%s\n' 'Production API health, unauthenticated session, and OAuth-start smoke passed.'
