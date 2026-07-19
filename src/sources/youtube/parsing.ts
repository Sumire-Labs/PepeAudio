/** Per QueueItem's contract, durationMs is null for live/unknown-duration content — YouTube/yt-dlp report 0 for in-progress livestreams. */
export function secondsToMs(seconds: number | null | undefined): number | null {
  if (seconds === null || seconds === undefined || Number.isNaN(seconds) || seconds <= 0) return null;
  return Math.round(seconds * 1000);
}

export const VIDEO_ID_RE = /^[A-Za-z0-9_-]{11}$/;

export function extractVideoId(url: string): string | null {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return null;
  }
  // Security boundary: result is handed to yt-dlp, so match host exactly (not
  // substring `includes`) to block look-alikes like `youtu.be.attacker.com`.
  const host = parsed.hostname.toLowerCase();
  if (host === 'youtu.be') {
    return parsed.pathname.split('/').filter(Boolean)[0] ?? null;
  }
  if (host !== 'youtube.com' && !host.endsWith('.youtube.com')) {
    return null;
  }
  const v = parsed.searchParams.get('v');
  if (v) return v;
  const shortsMatch = parsed.pathname.match(/\/shorts\/([\w-]+)/);
  if (shortsMatch) return shortsMatch[1] ?? null;
  return null;
}
