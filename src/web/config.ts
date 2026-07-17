/**
 * Web-dashboard configuration. Entirely a no-op unless WEB_DASHBOARD_ENABLED is
 * 'true' — loadWebEnv returns null and nothing else in src/web is ever
 * constructed, so existing deployments that never set these variables keep
 * booting unchanged. Secrets are validated ONLY when the dashboard is enabled
 * (lazy), and support the same `_FILE` Docker-secret indirection as DISCORD_TOKEN.
 */
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
  /** Mark the session cookie Secure when the public URL is https (production). */
  secureCookies: boolean;
  /** Directory the built frontend is served from. */
  clientDir: string;
}

/**
 * Returns the parsed web env, or null when the dashboard is disabled. `clientId`
 * is the already-validated public application id from the core env. `clientDir`
 * is where the compiled frontend lives (dist/web-client alongside the server).
 */
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

/**
 * Resolves where the built frontend lives. `callerDir` is the directory of the
 * entry module (dist/ when compiled, src/ under tsx). The frontend always builds
 * to dist/web-client, so from a compiled entry it's a sibling and from a src
 * entry it's ../dist/web-client. WEB_CLIENT_DIR overrides both.
 */
export function resolveClientDir(callerDir: string): string {
  if (process.env.WEB_CLIENT_DIR) return path.resolve(process.env.WEB_CLIENT_DIR);
  return path.basename(callerDir) === 'src'
    ? path.resolve(callerDir, '..', 'dist', 'web-client')
    : path.resolve(callerDir, 'web-client');
}
