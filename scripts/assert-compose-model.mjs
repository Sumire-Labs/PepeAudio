import { readFileSync } from "node:fs";

const source = process.argv[2];
if (!source) {
  throw new Error("usage: node scripts/assert-compose-model.mjs <compose-model.json>");
}

const model = JSON.parse(readFileSync(source, "utf8"));

function assert(condition, message) {
  if (!condition) {
    throw new Error(`invalid production Compose model: ${message}`);
  }
}

function service(name) {
  const value = model.services?.[name];
  assert(value, `service ${name} is missing`);
  return value;
}

function names(items = []) {
  return items.map((item) => (typeof item === "string" ? item : item.source));
}

function assertExact(actual, expected, label) {
  const left = [...actual].sort();
  const right = [...expected].sort();
  assert(JSON.stringify(left) === JSON.stringify(right), `${label} differs: ${left.join(", ")}`);
}

function assertAbsent(object, keys, label) {
  for (const key of keys) {
    assert(!(key in (object ?? {})), `${label} must not contain ${key}`);
  }
}

function volumeAt(container, target) {
  return (container.volumes ?? []).find((volume) => volume.target === target);
}

const api = service("api");
const bot = service("bot");
const caddy = service("caddy");
const postgres = service("postgres");
const valkey = service("valkey");

for (const [name, container] of Object.entries({ postgres, valkey, migrate: service("migrate"), api, bot })) {
  assertExact((container.group_add ?? []).map(String), ["10001"], `${name} secret-reader groups`);
}

assertExact(api.profiles, ["production"], "API profiles");
assertExact(bot.profiles, ["production"], "Bot profiles");
assertExact(caddy.profiles, ["production"], "Caddy profiles");

assert(api.environment.PEPEAUDIO_API_AUTH_MODE === "production", "API auth mode is not production");
assert(api.environment.PEPEAUDIO_PUBLIC_BASE_URL === "https://audio.example.test", "API public URL is not the verified HTTPS origin");
assert(api.environment.PEPEAUDIO_DISCORD_OAUTH_REDIRECT_URL === "https://audio.example.test/auth/callback", "OAuth callback does not match the public origin");
assert(api.environment.PEPEAUDIO_DISCORD_CLIENT_ID === "100000000000000002", "OAuth client ID placeholder was not propagated");
assert(api.environment.PEPEAUDIO_VALKEY_KEYSPACE === "pepeaudio-production", "API keyspace is not production-scoped");
assert(bot.environment.PEPEAUDIO_VALKEY_KEYSPACE === api.environment.PEPEAUDIO_VALKEY_KEYSPACE, "API and Bot keyspaces differ");

for (const key of [
  "PEPEAUDIO_API_BIND",
  "PEPEAUDIO_DATABASE_URL_FILE",
  "PEPEAUDIO_VALKEY_URL_FILE",
  "PEPEAUDIO_DISCORD_CLIENT_SECRET_FILE",
  "PEPEAUDIO_SHARD_TOTAL",
  "PEPEAUDIO_AUTH_SUCCESS_PATH",
  "PEPEAUDIO_SESSION_ABSOLUTE_SECONDS",
  "PEPEAUDIO_SESSION_IDLE_SECONDS",
  "PEPEAUDIO_OAUTH_STATE_SECONDS",
]) {
  assert(key in api.environment, `API environment is missing ${key}`);
}

const sessionAbsoluteSeconds = Number(api.environment.PEPEAUDIO_SESSION_ABSOLUTE_SECONDS);
const sessionIdleSeconds = Number(api.environment.PEPEAUDIO_SESSION_IDLE_SECONDS);
assert(
  Number.isSafeInteger(sessionAbsoluteSeconds) && sessionAbsoluteSeconds >= 60 && sessionAbsoluteSeconds <= 1800,
  "API absolute session lifetime must stay between 60 and 1800 seconds",
);
assert(
  Number.isSafeInteger(sessionIdleSeconds) && sessionIdleSeconds >= 60 && sessionIdleSeconds <= sessionAbsoluteSeconds,
  "API idle session lifetime must stay between 60 seconds and its absolute lifetime",
);

assertAbsent(api.environment, [
  "PEPEAUDIO_DATABASE_URL",
  "PEPEAUDIO_VALKEY_URL",
  "PEPEAUDIO_DISCORD_CLIENT_SECRET",
  "PEPEAUDIO_DISCORD_TOKEN",
  "PEPEAUDIO_DISCORD_TOKEN_FILE",
  "PEPEAUDIO_COMPONENT_SIGNING_KEY",
  "PEPEAUDIO_COMPONENT_SIGNING_KEY_FILE",
  "PEPEAUDIO_SESSION_KEY",
  "PEPEAUDIO_SESSION_KEY_FILE",
  "PEPEAUDIO_DEV_USER_ID",
  "PEPEAUDIO_DEV_GUILD_ID",
  "PEPEAUDIO_DEV_CSRF_TOKEN",
], "API environment");

assertExact(names(api.secrets), [
  "database_runtime_url",
  "valkey_url",
  "discord_client_secret",
], "API secrets");
assert(api.environment.PEPEAUDIO_DATABASE_URL_FILE === "/run/secrets/database_runtime_url", "API database secret path is incorrect");
assert(api.environment.PEPEAUDIO_VALKEY_URL_FILE === "/run/secrets/valkey_url", "API Valkey secret path is incorrect");
assert(api.environment.PEPEAUDIO_DISCORD_CLIENT_SECRET_FILE === "/run/secrets/discord_client_secret", "API OAuth secret path is incorrect");

for (const key of [
  "PEPEAUDIO_DISCORD_TOKEN_FILE",
  "PEPEAUDIO_COMPONENT_SIGNING_KEY_FILE",
  "PEPEAUDIO_SHARD_TOTAL",
  "PEPEAUDIO_SHARD_START",
  "PEPEAUDIO_INSTANCE_ID",
  "PEPEAUDIO_DATABASE_URL_FILE",
  "PEPEAUDIO_VALKEY_URL_FILE",
  "PEPEAUDIO_IDLE_DISCONNECT_SECONDS",
  "PEPEAUDIO_DEFAULT_VOLUME_PERCENT",
  "PEPEAUDIO_DEFAULT_HRIR_PRESET",
  "PEPEAUDIO_DEFAULT_SPATIAL_AUDIO_ENABLED",
  "PEPEAUDIO_MAX_QUEUE_ITEMS",
  "PEPEAUDIO_MAX_TRACK_DURATION_SECONDS",
  "PEPEAUDIO_MAX_UPLOAD_BYTES",
  "PEPEAUDIO_FFMPEG_PATH",
  "PEPEAUDIO_FFPROBE_PATH",
  "PEPEAUDIO_HRIR_DIRECTORY",
  "PEPEAUDIO_UPLOAD_DIRECTORY",
  "PEPEAUDIO_ENABLE_SITE_EXTRACTORS",
  "PEPEAUDIO_MAX_SITE_MEDIA_BYTES",
  "PEPEAUDIO_MAX_PLAYLIST_ITEMS",
  "PEPEAUDIO_YTDLP_PATH",
  "PEPEAUDIO_DENO_PATH",
  "PEPEAUDIO_DENO_DIR",
  "PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING",
  "PEPEAUDIO_ENABLE_SPOTIFY_PUBLIC_METADATA",
  "PEPEAUDIO_ENABLE_APPLE_MUSIC_PUBLIC_METADATA",
  "PEPEAUDIO_CATALOG_MAX_ITEMS",
]) {
  assert(key in bot.environment, `Bot environment is missing ${key}`);
}

assert(bot.environment.PEPEAUDIO_SHARD_TOTAL === "4", "Bot shard total override was not propagated");
assert(
  bot.environment.PEPEAUDIO_SHARD_TOTAL === api.environment.PEPEAUDIO_SHARD_TOTAL,
  "API and Bot shard totals differ",
);
assert(bot.environment.PEPEAUDIO_SHARD_START === "0", "Bot shard start default changed");
assert(bot.build?.dockerfile === "deploy/rust/Dockerfile.bot", "Bot must use its media-tools image");
assert(bot.environment.PEPEAUDIO_ENABLE_SITE_EXTRACTORS === "false", "site extractors must default off");
assert(
  bot.environment.PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING === "false",
  "cross-service matching must default off",
);
assert(
  bot.environment.PEPEAUDIO_ENABLE_SPOTIFY_PUBLIC_METADATA === "false",
  "Spotify public metadata must default off",
);
assert(
  bot.environment.PEPEAUDIO_ENABLE_APPLE_MUSIC_PUBLIC_METADATA === "false",
  "Apple Music public metadata must default off",
);
assert(bot.environment.PEPEAUDIO_YTDLP_PATH === "/usr/local/bin/yt-dlp", "Bot yt-dlp path changed");
assert(bot.environment.PEPEAUDIO_DENO_PATH === "/usr/local/bin/deno", "Bot Deno path changed");
assert(bot.environment.PEPEAUDIO_DENO_DIR === "/tmp/pepeaudio-deno", "Bot Deno cache path changed");
assert(
  !("PEPEAUDIO_SHARD_END_EXCLUSIVE" in bot.environment),
  "single-process Bot must let the validated runtime default shard end to shard total",
);

assertAbsent(bot.environment, [
  "PEPEAUDIO_DISCORD_TOKEN",
  "PEPEAUDIO_COMPONENT_SIGNING_KEY",
  "PEPEAUDIO_DISCORD_CLIENT_ID",
  "PEPEAUDIO_DISCORD_CLIENT_SECRET",
  "PEPEAUDIO_DISCORD_CLIENT_SECRET_FILE",
  "PEPEAUDIO_DISCORD_OAUTH_REDIRECT_URL",
  "PEPEAUDIO_PUBLIC_BASE_URL",
  "PEPEAUDIO_PUBLIC_ORIGIN",
  "PEPEAUDIO_API_BIND",
  "PEPEAUDIO_SESSION_KEY",
  "PEPEAUDIO_SESSION_KEY_FILE",
  "PEPEAUDIO_ADAPTERS_READY",
  "PEPEAUDIO_SPOTIFY_CLIENT_SECRET",
  "PEPEAUDIO_SPOTIFY_CLIENT_SECRET_FILE",
  "PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY",
  "PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_FILE",
], "Bot environment");

assertExact(names(bot.secrets), [
  "discord_token",
  "component_signing_key",
  "database_runtime_url",
  "valkey_url",
], "Bot secrets");
assert(bot.environment.PEPEAUDIO_DISCORD_TOKEN_FILE === "/run/secrets/discord_token", "Bot token secret path is incorrect");
assert(bot.environment.PEPEAUDIO_COMPONENT_SIGNING_KEY_FILE === "/run/secrets/component_signing_key", "Bot component-signing secret path is incorrect");
assert(bot.environment.PEPEAUDIO_DATABASE_URL_FILE === "/run/secrets/database_runtime_url", "Bot database secret path is incorrect");
assert(bot.environment.PEPEAUDIO_VALKEY_URL_FILE === "/run/secrets/valkey_url", "Bot Valkey secret path is incorrect");
assertExact(bot.healthcheck?.test ?? [], ["CMD-SHELL", "kill -0 1"], "Bot liveness command");

assert((api.ports ?? []).length === 0, "API must not publish host ports");
assert((bot.ports ?? []).length === 0, "Bot must not publish host ports");
assert((postgres.ports ?? []).length === 0, "PostgreSQL must not publish host ports");
assert((valkey.ports ?? []).length === 0, "Valkey must not publish host ports");

const uploads = volumeAt(bot, "/app/storage/uploads");
const hrir = volumeAt(bot, "/app/assets/hrir");
assert(uploads?.source === "uploads" && uploads.read_only !== true, "Bot upload volume must be writable");
assert(hrir?.source === "hrir_data" && hrir.read_only === true, "Bot HRIR volume must be read-only");

assertExact(Object.keys(api.networks ?? {}), ["edge", "data"], "API networks");
assertExact(Object.keys(bot.networks ?? {}), ["edge", "data"], "Bot networks");
assert(model.networks?.data?.internal === true, "data network must be internal");

assert(
  JSON.stringify(postgres.entrypoint) ===
    JSON.stringify(["/bin/sh", "/usr/local/bin/pepeaudio-secret-group-entrypoint"]),
  "PostgreSQL must retain the secret-reader group across its privilege drop",
);
assert(
  JSON.stringify(valkey.entrypoint) ===
    JSON.stringify(["/bin/sh", "/usr/local/bin/pepeaudio-secret-group-entrypoint"]),
  "Valkey must drop privilege while retaining the secret-reader group",
);
assertExact(postgres.command ?? [], ["postgres"], "PostgreSQL wrapper command");
assertExact(
  valkey.command ?? [],
  ["/usr/local/bin/pepeaudio-valkey-entrypoint"],
  "Valkey wrapper command",
);
assert(valkey.environment.PEPEAUDIO_SECRET_DROP_USER === "valkey", "Valkey secret wrapper must drop to valkey");
assertExact(valkey.cap_add ?? [], ["CHOWN", "SETGID", "SETUID"], "Valkey setup capabilities");
for (const [name, container] of Object.entries({ postgres, valkey })) {
  const wrapper = volumeAt(container, "/usr/local/bin/pepeaudio-secret-group-entrypoint");
  assert(
    wrapper?.type === "bind" && wrapper.read_only === true,
    `${name} secret-group entrypoint must be a read-only bind mount`,
  );
}

const ports = (caddy.ports ?? []).map((port) => `${port.target}/${port.protocol}`).sort();
assertExact(ports, ["80/tcp", "443/tcp", "443/udp"], "Caddy published ports");

console.log("Production Compose privilege and configuration assertions passed.");
