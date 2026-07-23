# PepeAudio WebGUI

Apple-Music-inspired web remote for the PepeAudio Discord bot.
Next.js (App Router, Turbopack) + React 19 + TypeScript + Tailwind CSS v4 +
`@microsoft/signalr` + TanStack Query + Zustand.

## Develop

```bash
cp .env.example .env.local           # set BACKEND_ORIGIN if not localhost:5080
pnpm install
pnpm dev                             # http://localhost:3000
```

The Next server proxies `/api/*` and `/hubs/*` to the ASP.NET backend
(`BACKEND_ORIGIN`) so the session cookie and SignalR handshake stay first-party.
Enable the backend WebGUI (`WebGui:Enabled=true` + OAuth creds) — see
[../docs/self-hosting.md](../docs/self-hosting.md).

## Behind a proxy / Cloudflare Tunnel

Route `/api` and `/hubs` to the backend and everything else to this app on the
same hostname (keeps cookies first-party, and WebSockets work end-to-end).

## Status

Runnable core: Discord login, server switcher, now-playing with live SignalR
state, transport controls, queue view, and play-by-link/search. Drag-to-reorder
and per-track remove need matching hub methods (follow-up).
