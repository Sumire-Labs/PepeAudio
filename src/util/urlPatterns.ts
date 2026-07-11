export type LinkKind = 'youtube' | 'spotify' | 'soundcloud' | 'applemusic' | 'search';

/**
 * Used only to prepend a scheme so bare "youtube.com/..." style input can be
 * parsed as a URL. This is a convenience, NOT the security boundary — the real
 * host check happens in classifyInput() after parsing. Anchored to the start so
 * it can't be tricked by a known host appearing later in the string.
 */
const KNOWN_HOST_NO_SCHEME_RE =
  /^(?:(?:www|m|music)\.)?(?:youtube\.com|youtu\.be|open\.spotify\.com|soundcloud\.com)\/|^music\.apple\.com\//i;

/**
 * Host allowlists. These are the security boundary: an input is only treated as
 * a provider link (and thus handed to a resolver that will fetch it) when its
 * *parsed hostname* is one of these. Suffix checks (`.youtube.com`) cover
 * legitimate subdomains (www/m/music) while rejecting look-alikes such as
 * `youtu.be.attacker.com` or `evil.com/?x=youtube.com/watch?`.
 */
function isYouTubeHost(host: string): boolean {
  return host === 'youtu.be' || host === 'youtube.com' || host.endsWith('.youtube.com');
}
function isSpotifyHost(host: string): boolean {
  return host === 'spotify.com' || host.endsWith('.spotify.com');
}
function isSoundCloudHost(host: string): boolean {
  return host === 'soundcloud.com' || host.endsWith('.soundcloud.com');
}
function isAppleMusicHost(host: string): boolean {
  return host === 'music.apple.com';
}

/**
 * Classifies an input by the URL's ACTUAL parsed hostname, never by a substring
 * match. This is a deliberate SSRF guard: a string like
 * `https://169.254.169.254/?v=x&r=youtube.com/watch?` merely *contains*
 * "youtube.com/watch?" but resolves to a non-provider host — it must NOT be
 * treated as a YouTube link, because the raw URL would otherwise be handed to
 * yt-dlp (a generic downloader that will fetch any host). Such inputs fall
 * through to 'search', which is only ever used as search *text* and never
 * fetched as a URL.
 */
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

/**
 * classifyInput()/downstream URL parsing (new URL(), spotify-uri's parse())
 * require a full scheme and throw on e.g. "youtube.com/watch?v=..." pasted
 * without "https://" — a real, confirmed failure mode. Only known-host inputs
 * get a scheme prepended; plain search text is left untouched.
 */
export function normalizeUrlInput(query: string): string {
  const trimmed = query.trim();
  if (/^https?:\/\//i.test(trimmed)) return trimmed;
  if (KNOWN_HOST_NO_SCHEME_RE.test(trimmed)) return `https://${trimmed}`;
  return trimmed;
}
