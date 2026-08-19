# syntax=docker/dockerfile:1.12

FROM rust:1.97.0-bookworm AS builder
WORKDIR /build

ARG PEPEAUDIO_BINARY
RUN test -n "${PEPEAUDIO_BINARY}"

# Songbird's static Opus backend needs CMake. Serenity's native TLS backend
# links OpenSSL on Linux, so its headers and pkg-config stay builder-only.
RUN apt-get update && \
    apt-get install --yes --no-install-recommends cmake libssl-dev pkg-config && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY vendor ./vendor
COPY migrations ./migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --locked --release --bin "${PEPEAUDIO_BINARY}" && \
    install -Dm755 "target/release/${PEPEAUDIO_BINARY}" /out/pepeaudio-service

FROM debian:bookworm-slim AS runtime
RUN apt-get update && \
    apt-get install --yes --no-install-recommends ca-certificates curl ffmpeg libssl3 && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --system --gid 10001 pepeaudio && \
    useradd --system --uid 10001 --gid pepeaudio --home-dir /app --shell /usr/sbin/nologin pepeaudio && \
    install -d -o pepeaudio -g pepeaudio /app /app/storage/uploads /app/assets/hrir

COPY --from=builder /out/pepeaudio-service /usr/local/bin/pepeaudio-service
COPY LICENSE /usr/share/licenses/pepeaudio/LICENSE
COPY docs/third-party.md /usr/share/doc/pepeaudio/THIRD-PARTY.md
COPY vendor/hpke-rs-0.6.1-security-patch/LICENSE-MPL-2.0 /usr/share/licenses/hpke-rs/LICENSE-MPL-2.0
COPY vendor/hpke-rs-0.6.1-security-patch /usr/share/src/pepeaudio/hpke-rs-0.6.1-security-patch

WORKDIR /app
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/pepeaudio-service"]
