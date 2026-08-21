import { displayablePublicMediaPage } from "./public-media-page";
import type { TrackProvenance } from "./types";

export function primaryTrackPageUrl(
  provenance: TrackProvenance | null | undefined
): string | null {
  const origin = displayablePublicMediaPage(provenance?.origin);
  if (origin !== null) return origin.url;
  return displayablePublicMediaPage(provenance?.playback)?.url ?? null;
}

export function trackArtworkUrl(
  provenance: TrackProvenance | null | undefined
): string | null {
  const playback = displayablePublicMediaPage(provenance?.playback);
  if (playback?.provider !== "youtube") return null;
  const url = new URL(playback.url);
  const id = url.hostname === "youtu.be"
    ? url.pathname.split("/").filter(Boolean)[0]
    : url.searchParams.get("v");
  return id && /^[A-Za-z0-9_-]{11}$/u.test(id)
    ? `https://i.ytimg.com/vi/${id}/hqdefault.jpg`
    : null;
}
