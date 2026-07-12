# Runtime image for the bot. The binaural 3D-audio path uses ffmpeg's stock
# `afir` convolution (bring-your-own BRIR) plus a spatial filter chain built
# entirely from standard filters, so it runs on any full ffmpeg build. The
# raw-HRTF `sofalizer` (libmysofa) filter was retired, so this image no longer
# compiles FFmpeg + libmysofa from source (removing a from-source C build, its
# toolchain, and a ~15-25 min build step). Debian's packaged ffmpeg provides
# ffmpeg + ffprobe with every filter the bot uses and is security-patched
# through the base image.

FROM node:22-bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
      ffmpeg ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY package.json pnpm-lock.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile --prod
COPY dist ./dist
COPY assets ./assets

# Persistent, writable data dir for the SQLite DB, kept OUT of the code tree.
# Mount a named volume here (see docker-compose.yml) so guild settings survive
# container recreation. chown happens BEFORE `VOLUME` so the volume inherits
# node ownership. `ytdlp-nodejs` stages its binary under /app, so that stays
# writable by the runtime user too.
ENV DATA_DIR=/data
RUN mkdir -p /data && chown -R node:node /data /app
VOLUME /data

# Drop root: the container needs no elevated privileges at runtime, which limits
# blast radius if the process is ever compromised.
ENV FFMPEG_PATH=/usr/bin/ffmpeg
ENV NODE_ENV=production
USER node

CMD ["node", "dist/shard.js"]
