export interface ParsedAppleMusicLink {
  kind: 'track' | 'album' | 'unsupported';
  id: string;
}

/**
 * Apple Music links look like:
 *   - track (within an album): /us/album/<name>/<albumId>?i=<trackId>
 *   - song-only:                /us/song/<name>/<trackId>
 *   - album (no track):         /us/album/<name>/<albumId>
 *   - track shared from within a playlist: /us/playlist/<name>/pl.u-<id>?i=<trackId>
 *     (a real, common share-sheet format — the numeric ?i= is just as
 *     resolvable via lookup(id) as any other track link)
 *   - playlist (no track):      /us/playlist/<name>/pl.u-<id>  (pl.u-<id> isn't
 *     numeric, and playlists themselves aren't resolvable via the free Lookup
 *     API — unsupported)
 */
export function parseAppleMusicUrl(url: string): ParsedAppleMusicLink {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return { kind: 'unsupported', id: '' };
  }
  const segments = parsed.pathname.split('/').filter(Boolean);

  // Checked BEFORE the playlist-path rejection below: a playlist link can
  // still carry a resolvable track id via ?i=, e.g. sharing "this song from
  // this playlist" - only a bare playlist link (no ?i=) is truly unsupported.
  const trackParam = parsed.searchParams.get('i');
  if (trackParam && /^\d+$/.test(trackParam)) {
    return { kind: 'track', id: trackParam };
  }

  if (segments.includes('playlist')) return { kind: 'unsupported', id: '' };

  const last = segments[segments.length - 1];
  if (last && /^\d+$/.test(last)) {
    if (segments.includes('song')) return { kind: 'track', id: last };
    if (segments.includes('album')) return { kind: 'album', id: last };
  }
  return { kind: 'unsupported', id: '' };
}
