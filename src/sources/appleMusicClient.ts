export class AppleMusicResolutionError extends Error {}

/**
 * Apple has no public streaming API for third-party playback, but the classic
 * iTunes Lookup API (https://itunes.apple.com/lookup) is free, key-less, and
 * still live — it resolves a numeric track/collection id to metadata
 * (verified directly: curl'd a real Apple Music track id and a real album id
 * with entity=song and got back trackName/artistName/track listings). We only
 * ever build this URL ourselves from a validated numeric id extracted from the
 * pasted link — the user's raw URL is never itself fetched, matching the same
 * SSRF-safe pattern as the YouTube resolver's canonical-URL rebuild.
 */
const ITUNES_LOOKUP_URL = 'https://itunes.apple.com/lookup';

export interface ItunesResult {
  wrapperType?: string;
  trackName?: string;
  artistName?: string;
}

export async function lookup(id: string, extraParams?: Record<string, string>): Promise<ItunesResult[]> {
  const lookupUrl = new URL(ITUNES_LOOKUP_URL);
  lookupUrl.searchParams.set('id', id);
  if (extraParams) {
    for (const [key, value] of Object.entries(extraParams)) lookupUrl.searchParams.set(key, value);
  }
  const res = await fetch(lookupUrl);
  if (!res.ok) {
    throw new Error(`iTunes Lookup API returned ${res.status}`);
  }
  const data = (await res.json()) as { results?: ItunesResult[] };
  return data.results ?? [];
}
