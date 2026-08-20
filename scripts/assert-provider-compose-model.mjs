import { readFileSync } from "node:fs";

const source = process.argv[2];
const mode = process.argv[3] ?? "credentials";
if (!source || !["credentials", "public-metadata"].includes(mode)) {
  throw new Error(
    "usage: node scripts/assert-provider-compose-model.mjs " +
      "<compose-model.json> [credentials|public-metadata]",
  );
}

const model = JSON.parse(readFileSync(source, "utf8"));

function assert(condition, message) {
  if (!condition) {
    throw new Error(`invalid provider Compose model: ${message}`);
  }
}

function secretNames(service) {
  return (service.secrets ?? []).map((secret) =>
    typeof secret === "string" ? secret : secret.source,
  );
}

function assertExact(actual, expected, label) {
  const left = [...actual].sort();
  const right = [...expected].sort();
  assert(
    JSON.stringify(left) === JSON.stringify(right),
    `${label} differs: ${left.join(", ")}`,
  );
}

const bot = model.services?.bot;
const api = model.services?.api;
assert(bot, "Bot service is missing");
assert(api, "API service is missing");

assert(
  bot.environment.PEPEAUDIO_ENABLE_SITE_EXTRACTORS === "true",
  "site extractors are not enabled",
);
assert(
  bot.environment.PEPEAUDIO_ENABLE_CROSS_SERVICE_MATCHING === "true",
  "cross-service matching is not enabled",
);

const baseBotSecrets = [
  "discord_token",
  "component_signing_key",
  "database_runtime_url",
  "valkey_url",
];
const providerSecrets = ["spotify_client_secret", "apple_music_private_key"];
const providerEnvironment = [
  "PEPEAUDIO_SPOTIFY_CLIENT_ID",
  "PEPEAUDIO_SPOTIFY_CLIENT_SECRET",
  "PEPEAUDIO_SPOTIFY_CLIENT_SECRET_FILE",
  "PEPEAUDIO_SPOTIFY_MARKET",
  "PEPEAUDIO_APPLE_MUSIC_TEAM_ID",
  "PEPEAUDIO_APPLE_MUSIC_KEY_ID",
  "PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY",
  "PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_FILE",
];
const publicMetadataEnvironment = [
  "PEPEAUDIO_ENABLE_SPOTIFY_PUBLIC_METADATA",
  "PEPEAUDIO_ENABLE_APPLE_MUSIC_PUBLIC_METADATA",
];

for (const key of [...providerEnvironment, ...publicMetadataEnvironment]) {
  assert(!(key in (api.environment ?? {})), `API environment contains ${key}`);
}
for (const secret of providerSecrets) {
  assert(!secretNames(api).includes(secret), `API secret list contains ${secret}`);
}

if (mode === "public-metadata") {
  assert(
    bot.environment.PEPEAUDIO_ENABLE_SPOTIFY_PUBLIC_METADATA === "true",
    "Spotify public metadata is not explicitly enabled",
  );
  assert(
    bot.environment.PEPEAUDIO_ENABLE_APPLE_MUSIC_PUBLIC_METADATA === "true",
    "Apple Music public metadata is not explicitly enabled",
  );
  for (const key of providerEnvironment) {
    assert(!(key in bot.environment), `keyless Bot environment contains ${key}`);
  }
  assertExact(secretNames(bot), baseBotSecrets, "keyless Bot secrets");
  for (const secret of providerSecrets) {
    assert(!(secret in (model.secrets ?? {})), `keyless model declares ${secret}`);
  }

  console.log("Keyless public-metadata Compose assertions passed.");
} else {
  assert(
    bot.environment.PEPEAUDIO_ENABLE_SPOTIFY_PUBLIC_METADATA === "false",
    "credential mode implicitly enables Spotify public metadata",
  );
  assert(
    bot.environment.PEPEAUDIO_ENABLE_APPLE_MUSIC_PUBLIC_METADATA === "false",
    "credential mode implicitly enables Apple Music public metadata",
  );
  assert(
    bot.environment.PEPEAUDIO_SPOTIFY_CLIENT_ID ===
      "compose-contract-client-id",
    "Spotify client ID was not propagated",
  );
  assert(
    bot.environment.PEPEAUDIO_SPOTIFY_MARKET === "JP",
    "Spotify market was not propagated",
  );
  assert(
    bot.environment.PEPEAUDIO_SPOTIFY_CLIENT_SECRET_FILE ===
      "/run/secrets/spotify_client_secret",
    "Spotify secret path is incorrect",
  );
  assert(
    bot.environment.PEPEAUDIO_APPLE_MUSIC_TEAM_ID === "ABCDE12345",
    "Apple Music team ID was not propagated",
  );
  assert(
    bot.environment.PEPEAUDIO_APPLE_MUSIC_KEY_ID === "KEY1234567",
    "Apple Music key ID was not propagated",
  );
  assert(
    bot.environment.PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY_FILE ===
      "/run/secrets/apple_music_private_key",
    "Apple Music private-key path is incorrect",
  );

  for (const directSecret of [
    "PEPEAUDIO_SPOTIFY_CLIENT_SECRET",
    "PEPEAUDIO_APPLE_MUSIC_PRIVATE_KEY",
  ]) {
    assert(
      !(directSecret in bot.environment),
      `Bot environment contains ${directSecret}`,
    );
  }

  assertExact(
    secretNames(bot),
    [...baseBotSecrets, ...providerSecrets],
    "credential Bot secrets",
  );
  for (const secret of providerSecrets) {
    assert(
      model.secrets?.[secret]?.file,
      `top-level secret ${secret} has no source file`,
    );
  }

  console.log("Credential-backed Spotify and Apple Music assertions passed.");
}
