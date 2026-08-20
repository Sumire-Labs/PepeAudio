# Cloudflare Tunnel deployment

PepeAudio can expose the production dashboard without opening ports 80 or 443
on the Ubuntu host. This deployment uses the host's systemd-managed
`cloudflared` connector and publishes Caddy only on IPv4 loopback:

```text
browser -> Cloudflare -> host cloudflared -> 127.0.0.1:18080 -> Caddy -> API
```

The Docker overlay does not start another connector and does not receive the
Tunnel token.

## Cloudflare configuration

Use a named, remotely managed Tunnel. Configure its Published Application as:

| Setting | Value |
| --- | --- |
| Public hostname | the value of `PEPEAUDIO_DOMAIN` |
| Service type | HTTP |
| Service URL | `http://127.0.0.1:18080` |
| HTTP Host Header | the value of `PEPEAUDIO_DOMAIN` |

For `audio.s12kuma01.com`, the Discord OAuth redirect URI is exactly:

```text
https://audio.s12kuma01.com/auth/callback
```

Do not select HTTPS for the local service and do not enable `noTLSVerify`.
Browser traffic is still HTTPS to Cloudflare; only the loopback hop from
`cloudflared` to Caddy uses HTTP.

Enable Cloudflare's edge HTTP-to-HTTPS redirect for the hostname. Keep
`/api/*`, `/auth/*`, and `/health/*` outside Cache Everything and Edge TTL
override rules. These routes contain private, authenticated, or live state.

## Host connector

Store the remotely managed Tunnel token outside the repository and Compose
secret set. The supported host path and permissions are:

```text
/etc/cloudflared/token  root:root  0400
```

The systemd service should read that file and run one connector. Never put the
token in `.env`, a Compose command, source control, logs, shell history, or
support messages. Rotate it in Cloudflare immediately if it is exposed.

Before starting PepeAudio, verify the host connector:

```sh
sudo systemctl is-active cloudflared
sudo systemctl show cloudflared \
  --property=ActiveState,SubState,NRestarts \
  --no-pager
```

## PepeAudio configuration

Keep the loopback listener in `.env`:

```dotenv
PEPEAUDIO_DOMAIN=audio.s12kuma01.com
PEPEAUDIO_TUNNEL_HTTP_BIND=127.0.0.1:18080
```

Confirm that port 18080 is not used by another service before startup:

```sh
if sudo ss -H -ltn '( sport = :18080 )' | grep -q .; then
  echo 'TCP 18080 is already occupied'
  sudo ss -ltnp '( sport = :18080 )'
else
  echo 'TCP 18080 is free'
fi
```

Prepare the normal production secrets; no Cloudflare token option is needed:

```sh
sudo sh scripts/prepare-production-secrets.sh
sudo sh scripts/prepare-production-secrets.sh --check
```

Add `--spotify` or `--apple-music` only when using the corresponding
credential-backed provider overlay. The public-metadata overlay needs neither
provider secret.

## Compose model

Place the Tunnel overlay after the production overlay so it replaces Caddy's
public 80/443 bindings with one loopback-only 18080 binding:

```sh
sudo docker compose \
  -f compose.yaml \
  -f compose.discord.yaml \
  -f compose.catalog.public-metadata.yaml \
  -f compose.production.yaml \
  -f compose.cloudflare-tunnel.yaml \
  --profile production \
  config --quiet
```

Use the same file order for `pull`, `build`, `up`, `ps`, and `logs`. In this
mode, Caddy runs as UID/GID 10001, has no writable volumes or added Linux
capabilities, and is reachable from the host only at `127.0.0.1:18080`.
PostgreSQL, Valkey, the API, and the Bot publish no host ports.

After startup, verify each hop:

```sh
sudo systemctl is-active cloudflared

sudo docker compose \
  -f compose.yaml \
  -f compose.discord.yaml \
  -f compose.catalog.public-metadata.yaml \
  -f compose.production.yaml \
  -f compose.cloudflare-tunnel.yaml \
  --profile production \
  ps

curl --fail --silent --show-error \
  -H 'Host: audio.s12kuma01.com' \
  http://127.0.0.1:18080/health/ready

curl --fail --silent --show-error \
  https://audio.s12kuma01.com/health/ready
```

A Cloudflare `1033` response means no healthy Tunnel connector is connected.
A `502` response with an active connector usually means the local origin is
not listening yet or the Published Application points to the wrong port.
