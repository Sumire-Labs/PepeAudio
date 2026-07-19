import type { WebEnv } from '../config.js';

const DISCORD_API = 'https://discord.com/api/v10';
export const OAUTH_SCOPE = 'identify guilds';

export interface DiscordUser {
  id: string;
  username: string;
  global_name?: string | null;
  avatar: string | null;
}

export function buildAuthorizeUrl(env: WebEnv, state: string): string {
  const params = new URLSearchParams({
    response_type: 'code',
    client_id: env.clientId,
    scope: OAUTH_SCOPE,
    redirect_uri: env.oauthRedirectUri,
    state,
    // no prompt=none: it errors for users who haven't authorized the app yet.
  });
  return `https://discord.com/oauth2/authorize?${params.toString()}`;
}

export async function exchangeCode(env: WebEnv, code: string): Promise<string> {
  const body = new URLSearchParams({
    grant_type: 'authorization_code',
    code,
    redirect_uri: env.oauthRedirectUri,
    client_id: env.clientId,
    client_secret: env.clientSecret,
  });
  const resp = await fetch(`${DISCORD_API}/oauth2/token`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body,
  });
  if (!resp.ok) throw new Error(`OAuth token exchange failed: ${resp.status}`);
  const data = (await resp.json()) as { access_token?: string };
  if (!data.access_token) throw new Error('OAuth token exchange returned no access_token');
  return data.access_token;
}

export async function fetchUser(accessToken: string): Promise<DiscordUser> {
  const resp = await fetch(`${DISCORD_API}/users/@me`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!resp.ok) throw new Error(`Fetch /users/@me failed: ${resp.status}`);
  return (await resp.json()) as DiscordUser;
}

export async function fetchGuildIds(accessToken: string): Promise<string[]> {
  const resp = await fetch(`${DISCORD_API}/users/@me/guilds`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!resp.ok) throw new Error(`Fetch /users/@me/guilds failed: ${resp.status}`);
  const data = (await resp.json()) as Array<{ id: string }>;
  return Array.isArray(data) ? data.map((g) => g.id).filter((id): id is string => typeof id === 'string') : [];
}
