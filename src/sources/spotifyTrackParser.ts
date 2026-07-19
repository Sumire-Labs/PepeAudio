interface RawSpotifyTrack {
  title?: string;
  subtitle?: string;
  artists?: Array<{ name?: string } | string> | string;
  show?: { publisher?: string };
}

// Use getData(), not the library's getTracks()/getPreview(): those call a toTrack()
// with an unguarded `track.audioPreview.url` that throws for isPlayable-but-null-preview
// tracks (a real Spotify shape), and that throw inside their .map() aborts the whole
// playlist before our per-track try/catch runs. We only need title/artist.
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
