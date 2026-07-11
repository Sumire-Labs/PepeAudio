interface RawSpotifyTrack {
  title?: string;
  subtitle?: string;
  artists?: Array<{ name?: string } | string> | string;
  show?: { publisher?: string };
}

/**
 * Deliberately does NOT use spotify-url-info's own getTracks()/getPreview() —
 * both call an internal toTrack() that does `track.isPlayable ? track.audioPreview.url
 * : undefined` with no null-guard, which throws for any track where isPlayable
 * is true but audioPreview is null (a real, observed Spotify API shape for some
 * region-restricted/removed tracks). Since a single malformed entry threw INSIDE
 * getTracks()'s own .map(), it aborted the WHOLE playlist resolution before our
 * per-track try/catch ever ran. We only need title/artist (never the preview
 * audio), so we use the lower-level getData() and extract those two fields
 * ourselves, defensively, per track.
 */
export function safeExtractTitleArtist(raw: unknown): { title: string; artist: string } | null {
  if (!raw || typeof raw !== 'object') return null;
  const track = raw as RawSpotifyTrack;
  const title = track.title;
  if (!title) return null;

  let artist = 'Unknown artist';
  try {
    if (track.show?.publisher) {
      artist = track.show.publisher;
    } else if (typeof track.artists === 'string' && track.artists.length > 0) {
      artist = track.artists;
    } else if (Array.isArray(track.artists)) {
      const names = track.artists
        .map((a) => (typeof a === 'string' ? a : a?.name))
        .filter((n): n is string => Boolean(n));
      if (names.length > 0) artist = names.join(', ');
    } else if (track.subtitle) {
      artist = track.subtitle;
    }
  } catch {
    // Leave artist as 'Unknown artist' rather than let a malformed entry abort resolution.
  }

  return { title, artist };
}
