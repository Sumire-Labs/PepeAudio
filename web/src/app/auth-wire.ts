const MAX_U64 = "18446744073709551615";
const MAX_GUILDS = 200;
const CSRF_PATTERN = /^[A-Za-z0-9_-]{43}$/u;
const ICON_HASH_PATTERN = /^[A-Za-z0-9_]{1,128}$/u;

export interface AuthSessionWire {
  readonly userId: string;
  readonly username: string | null;
  readonly displayName: string | null;
  readonly avatar: string | null;
  readonly csrfToken: string;
  readonly createdAtMs: number;
  readonly expiresAtMs: number;
}

export interface AuthGuild {
  readonly id: string;
  readonly name: string;
  readonly icon: string | null;
  readonly owner: boolean;
  readonly permissions: string;
  readonly botPresent: boolean;
}

export function parseAuthSession(input: unknown): AuthSessionWire {
  const value = record(input);
  const createdAtMs = timestamp(value.createdAtMs);
  const expiresAtMs = timestamp(value.expiresAtMs);
  if (createdAtMs >= expiresAtMs) invalid();
  if (typeof value.csrfToken !== "string" || !CSRF_PATTERN.test(value.csrfToken)) {
    invalid();
  }
  return {
    userId: snowflake(value.userId),
    username: optionalProfileText(value.username),
    displayName: optionalProfileText(value.displayName),
    avatar: optionalAssetHash(value.avatar),
    csrfToken: value.csrfToken,
    createdAtMs,
    expiresAtMs
  };
}

export function parseAuthGuilds(input: unknown): readonly AuthGuild[] {
  const value = record(input);
  if (!Array.isArray(value.guilds) || value.guilds.length > MAX_GUILDS) invalid();

  const seen = new Set<string>();
  return value.guilds.map((candidate) => {
    const guild = record(candidate);
    const id = snowflake(guild.id);
    if (seen.has(id)) invalid();
    seen.add(id);
    const name = guildName(guild.name);
    const icon = iconHash(guild.icon);
    if (typeof guild.owner !== "boolean" || typeof guild.botPresent !== "boolean") {
      invalid();
    }
    return {
      id,
      name,
      icon,
      owner: guild.owner,
      permissions: decimalU64(guild.permissions),
      botPresent: guild.botPresent
    };
  });
}

export function discordGuildIconUrl(
  guildId: string,
  hash: string | null
): string | null {
  if (hash === null || !isSnowflake(guildId) || !ICON_HASH_PATTERN.test(hash)) {
    return null;
  }
  return `https://cdn.discordapp.com/icons/${guildId}/${hash}.webp?size=64`;
}

export function discordUserAvatarUrl(
  userId: string,
  hash: string | null
): string | null {
  if (hash === null || !isSnowflake(userId) || !ICON_HASH_PATTERN.test(hash)) {
    return null;
  }
  return `https://cdn.discordapp.com/avatars/${userId}/${hash}.webp?size=64`;
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) invalid();
  return value as Record<string, unknown>;
}

function timestamp(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) invalid();
  return value as number;
}

function snowflake(value: unknown): string {
  if (typeof value !== "string" || !isSnowflake(value)) invalid();
  return value;
}

function isSnowflake(value: string): boolean {
  return /^[1-9][0-9]{0,19}$/u.test(value)
    && (value.length < 20 || value <= MAX_U64);
}

function decimalU64(value: unknown): string {
  if (typeof value !== "string" || !/^(?:0|[1-9][0-9]{0,19})$/u.test(value)) {
    invalid();
  }
  if (value.length === 20 && value > MAX_U64) invalid();
  return value;
}

function guildName(value: unknown): string {
  if (
    typeof value !== "string"
    || value.length === 0
    || value.length > 256
    || /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    invalid();
  }
  return value;
}

function iconHash(value: unknown): string | null {
  if (value === null) return null;
  if (typeof value !== "string" || !ICON_HASH_PATTERN.test(value)) invalid();
  return value;
}

function optionalAssetHash(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  return iconHash(value);
}

function optionalProfileText(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  if (
    typeof value !== "string"
    || value.trim().length === 0
    || value.length > 128
    || /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    invalid();
  }
  return value;
}

function invalid(): never {
  throw new Error("Auth response is invalid");
}
