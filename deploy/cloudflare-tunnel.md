# Cloudflare Tunnel deployment guide (PepeAudio WebGUI)

Publishes the dashboard through a **remotely-managed Cloudflare Tunnel**. The routing/config lives
in the Cloudflare dashboard, and the **`cloudflared` connector runs as a host service** on your
Linux server (not inside the compose). It makes an outbound connection to Cloudflare — no inbound
ports are opened on the server, and Cloudflare serves HTTPS at the edge.

```
browser ──HTTPS──> Cloudflare edge ──tunnel──> cloudflared (host systemd service)
                                                   │  reaches the app via loopback:
                                                   ├─ everything ─────> http://127.0.0.1:3000  (web / Next.js dashboard)
                                                   └─ /hubs,/api (opt) ─> http://127.0.0.1:5080  (bot-shard-0 backend + SignalR)
```

The `web` container already proxies `/api` and `/hubs` to `bot-shard-0:5080` **inside** the compose
network, so the simplest setup routes *everything* to `127.0.0.1:3000` and you only publish one
port. Routing `/hubs` straight to `:5080` is an optional optimisation for native WebSockets
(otherwise SignalR still works — it negotiates a fallback transport through the Next proxy).

---

## Ports — what you actually route

The compose binds both ports to **loopback (`127.0.0.1`) only**, so they're reachable by the
host-run cloudflared but never exposed on the server's public/LAN IP (the tunnel + Access stay the
only way in).

| Port | Service | Route in the tunnel? |
|---|---|---|
| **3000** | `web` — Next.js dashboard (proxies `/api` + `/hubs` internally) | **Yes — required.** Point every public hostname here (`http://127.0.0.1:3000`). |
| **5080** | `bot-shard-0` — WebGUI backend + SignalR hub | **Optional.** Only if you split `/hubs/*` (and/or `/api/*`) off for native WebSockets, or want direct health/metrics. |

⚠ **Use `127.0.0.1`, not `localhost`, as the Service URL.** Docker publishes on IPv4 `0.0.0.0` by
default and may not bind IPv6 `::1`; `localhost` can resolve to `::1` first and cloudflared's Go
dialer then gets *connection refused*. The literal `127.0.0.1` avoids that.

---

## Decision 0 — subdomain scheme (you chose **flatten / free**)

Free **Universal SSL** covers only the apex + **one** subdomain level (`*.s12kuma01.com`). A
third-level name like `player.audio.s12kuma01.com` is two labels deep and is **not** covered
(TLS error). You picked the free, flat scheme:

- `audio.s12kuma01.com` → landing
- `player.s12kuma01.com` → player
- `admin.s12kuma01.com` → admin

`web/middleware.ts` routes by the **first label**, so these map correctly with no app changes.
(If you ever want the nested `*.audio.` scheme, you'd need Advanced Certificate Manager, $10/mo per
zone, and an advanced cert for `*.audio.s12kuma01.com`.)
Ref: https://developers.cloudflare.com/ssl/edge-certificates/universal-ssl/limitations/

---

## Step 1 — Bring up the app (Docker, no cloudflared)

Copy the repo (or at least `deploy/`, `config/`, `assets/`, `web/`, `docker/`) to the server.
`deploy/docker-compose.yml` already has everything **except** cloudflared: postgres, valkey,
bgutil, bot-shard-0, web — with `web` on `127.0.0.1:3000` and `bot-shard-0` on `127.0.0.1:5080`.

Point OAuth at the public URL in `config/.env` (replace the localhost values):

```
WEBGUI__BASEURL=https://player.s12kuma01.com
WEBGUI__OAUTH__REDIRECTURI=https://player.s12kuma01.com/api/auth/callback
```

Then:

```
docker compose -f deploy/docker-compose.yml up -d --build
```

Notes:
- `web`'s `BACKEND_ORIGIN` stays `http://bot-shard-0:5080` (internal, baked at build) — unchanged.
- Optional hardening (code change, not required to work): make the backend honour Cloudflare's
  `X-Forwarded-Proto` so the session cookie is flagged `Secure`. Ask and I'll add
  `UseForwardedHeaders`. Without it login still works; the cookie just isn't `Secure`-flagged.

---

## Step 2 — Create the tunnel + run cloudflared on the host

The tunnel is *configured* in the dashboard, but the **connector still runs as a lightweight daemon
on your server** — you just install it as a systemd service instead of a container.

1. **Zero Trust dashboard → Networks → Tunnels → Create a tunnel** → connector **Cloudflared** →
   name it (e.g. `pepeaudio`) → **Save tunnel**.
   Ref: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/get-started/create-remote-tunnel/
2. Daemonize the connector — pick **one**:

   **Option A — Docker container (simplest if Docker already runs).** `--network host` is
   **mandatory** so the container's `127.0.0.1` is the host loopback (otherwise it can't reach the
   app's published ports):

   ```bash
   docker run -d --name cloudflared --restart unless-stopped --network host \
     cloudflare/cloudflared:latest tunnel --no-autoupdate run --token <TUNNEL_TOKEN>
   ```

   **Option B — host systemd service (apt-managed, no `--network host` needed).** The `cloudflared`
   *binary* is separate from the Docker image — install it first:

   ```bash
   sudo mkdir -p --mode=0755 /usr/share/keyrings
   curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg | sudo tee /usr/share/keyrings/cloudflare-main.gpg >/dev/null
   echo 'deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared any main' | sudo tee /etc/apt/sources.list.d/cloudflared.list
   sudo apt-get update && sudo apt-get install cloudflared
   sudo cloudflared service install <TUNNEL_TOKEN>   # registers + starts a systemd unit
   ```

   Verify with `systemctl status cloudflared` (Option B) or `docker logs -f cloudflared` (Option A)
   — look for four `Registered tunnel connection` lines. Use only one option per host (two
   connectors on one token just means HA replicas). If a stale unit exists, run
   `sudo cloudflared service uninstall` first.
3. Wait for the connector to show **Healthy / Connected** in the dashboard.

> 🔐 **Token hygiene.** The connector token lets anyone run your tunnel — never paste it into chats,
> commits, or logs. If it leaks, rotate it: dashboard → **Networks → Tunnels → your tunnel →
> Overview → Refresh token**, then restart the connector with the new token (the old one stops
> working for new connections).

---

## Step 3 — Public hostnames + routing

In the tunnel's **Public Hostname** tab, **Add a public hostname** for each of the three names.
The minimal, working setup points them all at the web container:

| Subdomain | Domain | Path | Type | URL |
|---|---|---|---|---|
| `audio` | `s12kuma01.com` | *(blank)* | HTTP | `http://127.0.0.1:3000` |
| `player` | `s12kuma01.com` | *(blank)* | HTTP | `http://127.0.0.1:3000` |
| `admin` | `s12kuma01.com` | *(blank)* | HTTP | `http://127.0.0.1:3000` |

Cloudflare **auto-creates the proxied DNS CNAME** (`<tunnel-id>.cfargotunnel.com`) for each.

**Optional — native WebSockets for SignalR.** Add a *higher-priority* entry per app hostname and
drag it **above** the catch-all (rules evaluate top-down, first match wins):

| Subdomain | Domain | Path | URL |
|---|---|---|---|
| `player` | `s12kuma01.com` | `/hubs/*` | `http://127.0.0.1:5080` |

(Add `/api/*` → `http://127.0.0.1:5080` too if you want REST/OAuth to bypass Next — not required.)

WebSockets are **on by default** (Network → WebSockets toggle); no extra setting. The 120s proxy
timeout only applies to normal HTTP header waits, **not** an established WebSocket, and SignalR's
15s keepalive holds the socket open under any Cloudflare idle window.
Refs: https://developers.cloudflare.com/network/websockets/ ,
https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/routing-to-tunnel/

---

## Step 4 — Discord OAuth for the public URL

Discord Developer Portal → your app → **OAuth2 → Redirects** → add **exactly**:

```
https://player.s12kuma01.com/api/auth/callback
```

(byte-exact; must equal `WEBGUI__OAUTH__REDIRECTURI`). Restart the stack after editing `config/.env`.

---

## Step 5 — Protect the admin hostname with Cloudflare Access (free ≤50 users)

1. Zero Trust → **Access controls → Applications → Add an application → Self-hosted**.
2. **Public hostname**: Subdomain `admin`, Domain `s12kuma01.com`.
3. **Access policies** → **Add a policy** → Action **Allow** → Include → **Emails** = your
   address(es).
4. Identity provider: the built-in **One-time PIN** works with no IdP (email code). Set a session
   duration → **Save**.

Now `admin.s12kuma01.com` shows Cloudflare's login gate before any traffic reaches the tunnel.
(The app *also* checks `WEBGUI__ADMINUSERIDS__0` for `/api/admin/*` — set your Discord user id there
too for defence in depth.)
Ref: https://developers.cloudflare.com/cloudflare-one/policies/access/

---

## Validate

1. `https://audio.s12kuma01.com` → landing page (valid padlock).
2. `https://player.s12kuma01.com` → dashboard → **Discord でログイン** → returns logged in.
3. Player realtime updates work (SignalR). If they lag/stop, add the `/hubs/*` → `127.0.0.1:5080`
   rule from Step 3.
4. `https://admin.s12kuma01.com` → Cloudflare Access login first, then the admin page.

## Gotchas
- **Service URL = `http://127.0.0.1:3000`**, never `localhost` (IPv6 `::1` refusal) and never the
  Docker service name `web`/`bot-shard-0` (those only resolve *inside* the compose network — a
  host-run cloudflared can't reach them).
- **Ports are loopback-bound** in the compose (`127.0.0.1:3000`/`5080`) so nothing is public except
  through the tunnel. If you instead run cloudflared **as a container**, `127.0.0.1` would mean the
  container itself — you'd need `network_mode: host`, or join the compose network and use
  `http://web:3000`. Host service is simpler; that's what this guide uses.
- **Path entries must sit above the catch-all** in the tunnel, or the catch-all wins.
- **Third-level subdomains aren't free** — you're on the flat scheme, so this doesn't bite you.
