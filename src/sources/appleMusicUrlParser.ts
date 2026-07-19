export interface ParsedAppleMusicLink {
  kind: 'track' | 'album' | 'unsupported';
  id: string;
}

// Bare playlist links (pl.u-<id>, non-numeric) aren't resolvable via the free
// Lookup API and are unsupported; a track shared from a playlist still carries
// a numeric ?i= and resolves like any other track.
export function parseAppleMusicUrl(url: string): ParsedAppleMusicLink {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return { kind: 'unsupported', id: '' };
  }
  const segments = parsed.pathname.split('/').filter(Boolean);

  // Must run before the playlist rejection below: a playlist share can still
  // carry a resolvable track id via ?i=.
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
