/**
 * Parses a YouTube URL's start-time parameter into milliseconds.
 * Supports both conventions YouTube actually uses:
 *   - plain seconds: ?t=90 / &start=90
 *   - letter-suffixed: ?t=1h2m3s / &t=2m3s / &t=45s (any subset, in order)
 */
export function parseYouTubeTimestamp(url: string): number | null {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return null;
  }
  const raw = parsed.searchParams.get('t') ?? parsed.searchParams.get('start');
  if (!raw) return null;

  if (/^\d+$/.test(raw)) {
    return parseInt(raw, 10) * 1000;
  }

  const match = raw.match(/^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s)?$/i);
  if (match && (match[1] ?? match[2] ?? match[3])) {
    const hours = parseInt(match[1] ?? '0', 10);
    const minutes = parseInt(match[2] ?? '0', 10);
    const seconds = parseInt(match[3] ?? '0', 10);
    return (hours * 3600 + minutes * 60 + seconds) * 1000;
  }
  return null;
}

/**
 * Parses a SoundCloud "share at this time" URL fragment into milliseconds.
 * SoundCloud's own share links use #t=1:23 (mm:ss) or #t=83 (plain seconds).
 */
export function parseSoundCloudTimestamp(url: string): number | null {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return null;
  }
  const hash = parsed.hash; // e.g. "#t=1:23"
  const mmss = hash.match(/^#t=(\d+):(\d{1,2})$/);
  if (mmss) {
    const minutes = parseInt(mmss[1]!, 10);
    const seconds = parseInt(mmss[2]!, 10);
    return (minutes * 60 + seconds) * 1000;
  }
  const plain = hash.match(/^#t=(\d+)$/);
  if (plain) {
    return parseInt(plain[1]!, 10) * 1000;
  }
  return null;
}
