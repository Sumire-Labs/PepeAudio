#!/bin/sh
set -eu

image=${PEPEAUDIO_CADDY_IMAGE:-pepeaudio-caddy:local}
container_name="pepeaudio-caddy-smoke-$$"
headers=$(mktemp "${TMPDIR:-/tmp}/pepeaudio-caddy-headers.XXXXXX")
body=$(mktemp "${TMPDIR:-/tmp}/pepeaudio-caddy-body.XXXXXX")

cleanup() {
    docker rm --force "$container_name" >/dev/null 2>&1 || true
    rm -f "$headers" "$body"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

docker run --detach --name "$container_name" \
    --publish 127.0.0.1::8080 "$image" >/dev/null

port=$(docker port "$container_name" 8080/tcp | sed -n 's/.*://p' | head -n 1)
test -n "$port"
base_url="http://127.0.0.1:$port"

attempt=0
until curl --silent --show-error --fail --dump-header "$headers" \
    --output "$body" "$base_url/"; do
    attempt=$((attempt + 1))
    test "$attempt" -lt 20
    sleep 1
done

grep -Eiq '^cache-control: no-cache' "$headers"
grep -Eiq '^content-security-policy: .*form-action '\''self'\''.*img-src '\''self'\'' data: https://cdn\.discordapp\.com' "$headers"
grep -Eiq '^cross-origin-opener-policy: same-origin' "$headers"
grep -Eiq '^cross-origin-resource-policy: same-origin' "$headers"
grep -Eiq '^x-robots-tag: noindex, nofollow, noarchive' "$headers"

backend_status=$(curl --silent --show-error --dump-header "$headers" \
    --output /dev/null --write-out '%{http_code}' "$base_url/api/v1/player")
test "$backend_status" = 502
grep -Eiq '^cache-control: private, no-store' "$headers"

asset=$(sed -n 's/.*src="\([^"]*\/assets\/[^"]*\.js\)".*/\1/p' "$body" | head -n 1)
test -n "$asset"
curl --silent --show-error --fail --dump-header "$headers" \
    --output /dev/null "$base_url$asset"
grep -Eiq '^cache-control: public, max-age=31536000, immutable' "$headers"

curl --silent --show-error --fail --output /dev/null "$base_url/LICENSE.txt"
curl --silent --show-error --fail --output /dev/null "$base_url/THIRD-PARTY.md"
curl --silent --show-error --fail --output /dev/null "$base_url/licenses/manifest.json"

if docker exec "$container_name" sh -c \
    'find /srv -type f -name "*.map" -print -quit | grep -q .'; then
    printf '%s\n' 'Production image unexpectedly contains source maps.' >&2
    exit 1
fi

printf '%s\n' 'Caddy app shell, immutable assets, security headers, and notices passed.'
