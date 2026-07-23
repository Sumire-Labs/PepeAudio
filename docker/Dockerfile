# SPDX-License-Identifier: Apache-2.0
FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
WORKDIR /src
COPY . .
RUN dotnet publish src/PepeAudio.Host/PepeAudio.Host.csproj -c Release -o /app /p:UseAppHost=false

FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS runtime
# Discord.Net voice needs opus, sodium and libdave loadable by their unversioned name.
# The apt packages ship only versioned opus/sodium and libdave is not packaged at all,
# so stage all three in /usr/local/lib and point the loader there. libdave (DAVE E2EE)
# is mandatory for voice since 2026-03-01.
ARG LIBDAVE_URL=https://github.com/discord/libdave/releases/download/v1.1.1/cpp/libdave-Linux-X64-boringssl.zip
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg libopus0 libsodium23 curl ca-certificates unzip \
    && curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux -o /usr/local/bin/yt-dlp \
    && chmod a+rx /usr/local/bin/yt-dlp \
    && mkdir -p /usr/local/lib \
    && ln -sf /usr/lib/x86_64-linux-gnu/libopus.so.0 /usr/local/lib/libopus.so \
    && ln -sf /usr/lib/x86_64-linux-gnu/libsodium.so.23 /usr/local/lib/libsodium.so \
    && curl -L "$LIBDAVE_URL" -o /tmp/libdave.zip \
    && unzip -j /tmp/libdave.zip lib/libdave.so -d /usr/local/lib \
    && rm -f /tmp/libdave.zip \
    && rm -rf /var/lib/apt/lists/*
ENV LD_LIBRARY_PATH=/usr/local/lib
# curl is kept for the container HEALTHCHECK (/healthz).

WORKDIR /app
COPY --from=build /app .
COPY config ./config
COPY assets ./assets
RUN useradd -u 10001 -m pepe && chown -R pepe /app
USER pepe
ENV DOTNET_ENVIRONMENT=Production
ENTRYPOINT ["dotnet", "PepeAudio.Host.dll"]
