# Linux deployment path for the binaural (sofalizer/libmysofa) 3D-audio mode.
# No confirmed prebuilt static ffmpeg with libmysofa exists for Linux (see the
# plan's platform matrix), so this builds one from source in its own stage and
# copies only the resulting binaries into a slim runtime image.

FROM debian:bookworm-slim AS ffmpeg-builder
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential git pkg-config yasm nasm cmake \
      libmysofa-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
RUN git clone --depth 1 https://git.ffmpeg.org/ffmpeg.git ffmpeg
WORKDIR /build/ffmpeg
RUN ./configure \
      --enable-gpl --enable-version3 \
      --enable-libmysofa \
      --disable-doc --disable-debug \
    && make -j"$(nproc)" \
    && make install

FROM node:22-bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
      libmysofa1 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=ffmpeg-builder /usr/local/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=ffmpeg-builder /usr/local/bin/ffprobe /usr/local/bin/ffprobe

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
ENV FFMPEG_PATH=/usr/local/bin/ffmpeg
ENV NODE_ENV=production
USER node

CMD ["node", "dist/shard.js"]
