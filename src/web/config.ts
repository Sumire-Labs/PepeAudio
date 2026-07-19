import path from 'node:path';
import { required, requiredSecret } from '../config/env.js';

export interface WebEnv {
  port: number;
  bindHost: string;
  /** Public base URL, no trailing slash (e.g. https://panel.example.com). */
  publicUrl: string;
  /** Origin only (scheme://host[:port]), used for CSRF Origin/Referer checks. */
  publicOrigin: string;
  oauthRedirectUri: string;
  clientId: string;
  clientSecret: string;
  sessionSecret: string;
  secureCookies: boolean;
  clientDir: string;
}

export function loadWebEnv(clientId: string, clientDir: string): WebEnv | null {
  if (process.env.WEB_DASHBOARD_ENABLED !== 'true') return null;

  const publicUrl = required('WEB_PUBLIC_URL').replace(/\/+$/, '');
  let publicOrigin: string;
  try {
    publicOrigin = new URL(publicUrl).origin;
  } catch {
    throw new Error(`WEB_PUBLIC_URL is not a valid URL: ${publicUrl}`);
  }

  const sessionSecret = requiredSecret('SESSION_SECRET');
  if (sessionSecret.length < 32) {
    throw new Error('SESSION_SECRET must be at least 32 characters (use a long random value).');
  }

  const portRaw = process.env.WEB_PORT ?? '8080';
  const port = Number(portRaw);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(`WEB_PORT is not a valid port: ${portRaw}`);
  }

  return {
    port,
    bindHost: process.env.WEB_BIND_HOST || '127.0.0.1',
    publicUrl,
    publicOrigin,
    oauthRedirectUri: process.env.OAUTH_REDIRECT_URI || `${publicUrl}/auth/callback`,
    clientId,
    clientSecret: requiredSecret('DISCORD_CLIENT_SECRET'),
    sessionSecret,
    secureCookies: publicUrl.startsWith('https://'),
    clientDir,
  };
}

// callerDir is src/ under tsx, dist/ when compiled; the frontend always builds to
// dist/web-client (sibling of dist/, ../dist/web-client from src/). WEB_CLIENT_DIR overrides.
export function resolveClientDir(callerDir: string): string {
  if (process.env.WEB_CLIENT_DIR) return path.resolve(process.env.WEB_CLIENT_DIR);
  return path.basename(callerDir) === 'src'
    ? path.resolve(callerDir, '..', 'dist', 'web-client')
    : path.resolve(callerDir, 'web-client');
}
