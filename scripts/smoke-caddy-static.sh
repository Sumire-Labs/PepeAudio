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

for config in /etc/caddy/Caddyfile /etc/caddy/Caddyfile.tunnel; do
    docker run --rm --network none --entrypoint caddy "$image" \
        validate --config "$config" --adapter caddyfile >/dev/null
done
docker run --rm --network none --entrypoint sh "$image" \
    -euc 'test -z "$(getcap /usr/bin/caddy)"'

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

docker rm --force "$container_name" >/dev/null

docker run --detach --name "$container_name" \
    --env PEPEAUDIO_DOMAIN=audio.example.test \
    --publish 127.0.0.1::8080 \
    --user 10001:10001 \
    --read-only \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    --pids-limit 64 \
    --entrypoint caddy \
    "$image" run --config /etc/caddy/Caddyfile.tunnel \
        --adapter caddyfile >/dev/null

port=$(docker port "$container_name" 8080/tcp | sed -n 's/.*://p' | head -n 1)
test -n "$port"
base_url="http://127.0.0.1:$port"

attempt=0
until curl --silent --show-error --fail \
    --header 'Host: audio.example.test' \
    --output /dev/null "$base_url/"; do
    attempt=$((attempt + 1))
    test "$attempt" -lt 20
    sleep 1
done

unexpected_host_status=$(curl --silent --show-error \
    --header 'Host: unexpected.example.test' \
    --output /dev/null --write-out '%{http_code}' "$base_url/")
test "$unexpected_host_status" = 421

printf '%s\n' \
    'Caddy app shell, tunnel host isolation, security headers, and notices passed.'
