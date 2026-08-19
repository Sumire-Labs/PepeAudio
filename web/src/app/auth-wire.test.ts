import { describe, expect, it } from "vitest";

import {
  discordGuildIconUrl,
  discordUserAvatarUrl,
  parseAuthGuilds,
  parseAuthSession
} from "./auth-wire";

const GUILD_ID = "18446744073709551615";
const CSRF_TOKEN = "A".repeat(43);

describe("auth wire parsing", () => {
  it("keeps Discord snowflakes as strings and validates the full session shape", () => {
    expect(parseAuthSession({
      userId: GUILD_ID,
      username: "pepe-listener",
      displayName: "Pepe Listener",
      avatar: "a_profile_hash",
      csrfToken: CSRF_TOKEN,
      createdAtMs: 1_000,
      expiresAtMs: 2_000
    })).toEqual({
      userId: GUILD_ID,
      username: "pepe-listener",
      displayName: "Pepe Listener",
      avatar: "a_profile_hash",
      csrfToken: CSRF_TOKEN,
      createdAtMs: 1_000,
      expiresAtMs: 2_000
    });

    expect(() => parseAuthSession({
      userId: Number(GUILD_ID),
      csrfToken: CSRF_TOKEN,
      createdAtMs: 1_000,
      expiresAtMs: 2_000
    })).toThrow("Auth response is invalid");
    expect(parseAuthSession({
      userId: "1",
      csrfToken: CSRF_TOKEN,
      createdAtMs: 1_000,
      expiresAtMs: 2_000
    })).toMatchObject({
      username: null,
      displayName: null,
      avatar: null
    });
    expect(() => parseAuthSession({
      userId: "1",
      csrfToken: "short",
      createdAtMs: 2_000,
      expiresAtMs: 1_000
    })).toThrow("Auth response is invalid");
  });

  it("accepts safe guild data and creates only a fixed Discord CDN URL", () => {
    const [guild] = parseAuthGuilds({
      guilds: [{
        id: GUILD_ID,
        name: "Listening Room",
        icon: "a_0123ABC_def",
        owner: false,
        permissions: GUILD_ID,
        botPresent: true
      }]
    });

    if (guild === undefined) throw new Error("parsed guild is missing");
    expect(guild.id).toBe(GUILD_ID);
    expect(discordGuildIconUrl(guild.id, guild.icon)).toBe(
      `https://cdn.discordapp.com/icons/${GUILD_ID}/a_0123ABC_def.webp?size=64`
    );
    expect(discordGuildIconUrl("1/../../secret", "safe_hash")).toBeNull();
    expect(discordGuildIconUrl("1", "unsafe/hash")).toBeNull();
    expect(discordUserAvatarUrl(GUILD_ID, "a_0123ABC_def")).toBe(
      `https://cdn.discordapp.com/avatars/${GUILD_ID}/a_0123ABC_def.webp?size=64`
    );
    expect(discordUserAvatarUrl("1/../../secret", "safe_hash")).toBeNull();
  });

  it("rejects duplicate guilds, unsafe hashes, and out-of-range permissions", () => {
    const guild = {
      id: "1",
      name: "Room",
      icon: null,
      owner: true,
      permissions: "0",
      botPresent: true
    };
    expect(() => parseAuthGuilds({ guilds: [guild, guild] })).toThrow(
      "Auth response is invalid"
    );
    expect(() => parseAuthGuilds({
      guilds: [{ ...guild, icon: "../../avatar" }]
    })).toThrow("Auth response is invalid");
    expect(() => parseAuthGuilds({
      guilds: [{ ...guild, permissions: "18446744073709551616" }]
    })).toThrow("Auth response is invalid");
    expect(() => parseAuthSession({
      userId: "1",
      username: "bad\nname",
      displayName: null,
      avatar: null,
      csrfToken: CSRF_TOKEN,
      createdAtMs: 1_000,
      expiresAtMs: 2_000
    })).toThrow("Auth response is invalid");
  });
});
