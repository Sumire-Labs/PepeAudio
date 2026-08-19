import { ApiResponseError, readJsonResponse } from "./api-response";
import {
  discordUserAvatarUrl,
  parseAuthGuilds,
  parseAuthSession,
  type AuthGuild
} from "./auth-wire";
import type { DashboardAccount } from "./types";
import { runtimeConfig } from "./runtime-config";

export interface AuthBootstrap {
  readonly csrfToken: string;
  readonly guilds: readonly AuthGuild[];
  readonly account: DashboardAccount;
  readonly logoutAvailable: boolean;
}

export async function fetchAuthBootstrap(signal?: AbortSignal): Promise<AuthBootstrap> {
  if (runtimeConfig.devUserId !== null && runtimeConfig.csrfToken !== null) {
    const guildId = runtimeConfig.bootstrapGuildId;
    if (guildId === null) {
      throw new ApiResponseError(500, "開発用サーバーが設定されていません。", "invalid_config");
    }
    const guilds = parseAuthGuilds({
      guilds: [{
        id: guildId,
        name: "開発用サーバー",
        icon: null,
        owner: true,
        permissions: "0",
        botPresent: true
      }]
    });
    return {
      csrfToken: runtimeConfig.csrfToken,
      guilds,
      account: {
        source: "development",
        userId: runtimeConfig.devUserId,
        username: null,
        displayName: "開発アカウント",
        avatarUrl: null
      },
      logoutAvailable: false
    };
  }

  const sessionResponse = await fetch("/auth/session", {
    credentials: "same-origin",
    signal: signal ?? null
  });
  const session = parseAuthSession(await readJsonResponse(sessionResponse));
  const guildResponse = await fetch("/auth/guilds", {
    credentials: "same-origin",
    signal: signal ?? null
  });
  const guilds = parseAuthGuilds(await readJsonResponse(guildResponse));
  return {
    csrfToken: session.csrfToken,
    guilds,
    account: {
      source: "discord",
      userId: session.userId,
      username: session.username,
      displayName: session.displayName ?? session.username ?? "Discordアカウント",
      avatarUrl: discordUserAvatarUrl(session.userId, session.avatar)
    },
    logoutAvailable: true
  };
}

export async function logoutSession(csrfToken: string): Promise<void> {
  const response = await fetch("/auth/logout", {
    method: "POST",
    credentials: "same-origin",
    headers: { "x-csrf-token": csrfToken }
  });
  await readJsonResponse(response);
}

export type { AuthGuild } from "./auth-wire";
