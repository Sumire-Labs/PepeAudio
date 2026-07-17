# syntax=docker/dockerfile:1

# ---- build stage: compile TypeScript → dist/ (needs devDependencies) ----
# dist/ is gitignored and never shipped, so the image builds it itself: a fresh
# `git clone` + `docker compose up --build` works with nothing pre-built on the
# host. This stage installs the full dependency set (incl. the TypeScript
# compiler) and is discarded — only its dist/ output is copied into the runtime.
FROM node:22-bookworm-slim AS builder
WORKDIR /app
# pnpm-workspace.yaml carries the overrides + minimumReleaseAge supply-chain
# policy that the lockfile was resolved against — without it `--frozen-lockfile`
# fails with ERR_PNPM_LOCKFILE_CONFIG_MISMATCH.
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile
COPY tsconfig.json ./
COPY src ./src
RUN pnpm build

# ---- build the optional web dashboard frontend ----
# web-client/ is a STANDALONE package (its own lockfile + .npmrc), independent of
# the bot's pnpm workspace, so `--ignore-workspace` keeps its install isolated
# from the root supply-chain policy. Vite emits into /app/dist/web-client, which
# the `COPY --from=builder /app/dist` below carries into the runtime image. The
# dashboard is inert unless WEB_DASHBOARD_ENABLED=true at runtime, so building it
# here just makes it available; it costs a small static bundle, nothing at idle.
COPY web-client ./web-client
RUN cd web-client \
    && pnpm install --ignore-workspace --frozen-lockfile \
    && pnpm rebuild esbuild \
    && pnpm build

# ---- runtime image ----
# The binaural 3D-audio path uses ffmpeg's stock `afir` convolution (bring-your-
# own BRIR) plus a spatial filter chain built entirely from standard filters, so
# it runs on Debian's packaged ffmpeg. The raw-HRTF `sofalizer` (libmysofa)
# filter was retired, so this image no longer compiles FFmpeg from source
# (removing a from-source C build, its toolchain, and a ~15-25 min build step).
FROM node:22-bookworm-slim AS runtime
# python3: the yt-dlp binary that ytdlp-nodejs stages is the pure-Python zipapp
# (shebang `#!/usr/bin/env python3`), so it needs a system Python 3 interpreter —
# without it every extraction dies with "exit code 127" (python3 not found) and
# playback is silent. ffmpeg is the fallback binary / ffprobe; the HRIR path uses
# the bundled ffmpeg-static (see the FFMPEG_PATH note below).
RUN apt-get update && apt-get install -y --no-install-recommends \
      ffmpeg python3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
# Copy pnpm-workspace.yaml too: it declares `allowBuilds` (better-sqlite3 /
# @discordjs/opus native builds — blocked by default in pnpm otherwise) plus the
# overrides + minimumReleaseAge policy the lockfile was resolved against.
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile --prod
COPY --from=builder /app/dist ./dist
COPY assets ./assets

# Persistent, writable data dir for the SQLite DB, kept OUT of the code tree.
# Mount a named volume here (see docker-compose.yml) so guild settings survive
# container recreation. chown happens BEFORE `VOLUME` so the volume inherits
# node ownership. `ytdlp-nodejs` stages its binary under /app, so that stays
# writable by the runtime user too.
ENV DATA_DIR=/data
RUN mkdir -p /data && chown -R node:node /data /app
VOLUME /data

# NB: FFMPEG_PATH is deliberately NOT pinned to the apt ffmpeg. Debian bookworm
# ships ffmpeg 5.1, whose `afir` filter lacks the `irlink` option the HRIR
# convolution graph needs (it fails with "Option 'irlink' not found" → silence).
# With no override, ffmpegResolver falls back to the bundled ffmpeg-static
# (7.0.2, full-featured, has `irlink`), which prism-media already uses too. The
# apt ffmpeg above stays only as a last-resort fallback / for ffprobe.
ENV NODE_ENV=production
USER node

CMD ["node", "dist/shard.js"]
