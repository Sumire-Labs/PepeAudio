export class AppleMusicResolutionError extends Error {}

// SSRF-safe: URL is built here from a validated numeric id, never by fetching the user's raw pasted link.
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
