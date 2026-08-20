# syntax=docker/dockerfile:1.12

FROM node:24.19.0-bookworm-slim AS web-builder
WORKDIR /web

RUN npm install --global pnpm@11.3.0
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store,sharing=locked \
    pnpm install --frozen-lockfile
COPY LICENSE /LICENSE
COPY docs/third-party.md /docs/third-party.md
COPY web ./
ARG VITE_PEPEAUDIO_DEMO_MODE=false
ARG VITE_API_BASE_URL=/api/v1
ARG VITE_PEPEAUDIO_GUILD_ID
ARG VITE_PEPEAUDIO_DEV_USER_ID
ARG PEPEAUDIO_WEB_DEV_BOOTSTRAP
RUN VITE_PEPEAUDIO_DEMO_MODE="${VITE_PEPEAUDIO_DEMO_MODE}" \
    VITE_API_BASE_URL="${VITE_API_BASE_URL}" \
    VITE_PEPEAUDIO_GUILD_ID="${VITE_PEPEAUDIO_GUILD_ID}" \
    VITE_PEPEAUDIO_DEV_USER_ID="${VITE_PEPEAUDIO_DEV_USER_ID}" \
    VITE_PEPEAUDIO_DEV_CSRF_TOKEN="${PEPEAUDIO_WEB_DEV_BOOTSTRAP}" \
    pnpm build

FROM caddy:2.10.2-alpine
RUN setcap -r /usr/bin/caddy && test -z "$(getcap /usr/bin/caddy)"
COPY deploy/caddy/Caddyfile /etc/caddy/Caddyfile
COPY deploy/caddy/Caddyfile.tunnel /etc/caddy/Caddyfile.tunnel
COPY deploy/caddy/PepeAudio.caddy /etc/caddy/PepeAudio.caddy
COPY --from=web-builder /web/dist /srv
COPY LICENSE /usr/share/licenses/pepeaudio/LICENSE
COPY docs/third-party.md /usr/share/doc/pepeaudio/THIRD-PARTY.md
COPY --from=web-builder /web/dist/licenses /usr/share/licenses/pepeaudio-web-dependencies
