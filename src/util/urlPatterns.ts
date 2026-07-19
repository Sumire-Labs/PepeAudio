export type LinkKind = 'youtube' | 'spotify' | 'soundcloud' | 'applemusic' | 'search';

// Convenience for prepending a scheme, NOT the security boundary (that's classifyInput's
// parsed-host check). Anchored to start so a known host later in the string can't trick it.
const KNOWN_HOST_NO_SCHEME_RE =
  /^(?:(?:www|m|music)\.)?(?:youtube\.com|youtu\.be|open\.spotify\.com|soundcloud\.com)\/|^music\.apple\.com\//i;

// Security boundary: input is only treated as a provider link (handed to a fetching resolver)
// when its parsed hostname matches. Suffix checks accept subdomains (www/m/music) while
// rejecting look-alikes like `youtu.be.attacker.com`.
function isYouTubeHost(host: string): boolean {
  return host === 'youtu.be' || host === 'youtube.com' || host.endsWith('.youtube.com');
}
export function isSpotifyHost(host: string): boolean {
  return host === 'spotify.com' || host.endsWith('.spotify.com');
}
export function isSoundCloudHost(host: string): boolean {
  return host === 'soundcloud.com' || host.endsWith('.soundcloud.com');
}
function isAppleMusicHost(host: string): boolean {
  return host === 'music.apple.com';
}

// SSRF guard: classify by parsed hostname, never a substring match.
// `https://169.254.169.254/?v=x&r=youtube.com/watch?` contains "youtube.com/watch?" but must
// NOT be treated as YouTube — yt-dlp would fetch the raw host. Non-provider hosts fall through
// to 'search', which is only used as search text and never fetched as a URL.
export function classifyInput(query: string): LinkKind {
  const normalized = normalizeUrlInput(query);
  let host: string;
  try {
    host = new URL(normalized).hostname.toLowerCase();
  } catch {
    return 'search';
  }
  if (isYouTubeHost(host)) return 'youtube';
  if (isSpotifyHost(host)) return 'spotify';
  if (isSoundCloudHost(host)) return 'soundcloud';
  if (isAppleMusicHost(host)) return 'applemusic';
  return 'search';
}

// URL parsing (new URL(), spotify-uri) needs a scheme and throws on e.g. "youtube.com/watch?v=..."
// pasted without https://. Only known-host inputs get a scheme prepended; search text is untouched.
export function normalizeUrlInput(query: string): string {
  const trimmed = query.trim();
  if (/^https?:\/\//i.test(trimmed)) return trimmed;
  if (KNOWN_HOST_NO_SCHEME_RE.test(trimmed)) return `https://${trimmed}`;
  return trimmed;
}
