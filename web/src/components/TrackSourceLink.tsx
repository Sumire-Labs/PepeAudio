import { Link } from "@astryxdesign/core/Link";
import { HStack } from "@astryxdesign/core/Stack";

import { displayablePublicMediaPage } from "../app/public-media-page";
import type { MediaProvider, TrackProvenance } from "../app/types";

interface TrackSourceLinkProps {
  readonly provenance: TrackProvenance | null | undefined;
}

export function TrackSourceLinks({ provenance }: TrackSourceLinkProps) {
  const origin = displayablePublicMediaPage(provenance?.origin);
  const playbackCandidate = displayablePublicMediaPage(provenance?.playback);
  const playback = playbackCandidate !== null &&
      (playbackCandidate.provider === "youtube" ||
        playbackCandidate.provider === "soundcloud")
    ? playbackCandidate
    : null;
  const pages = origin === null
    ? playback === null ? [] : [{ page: playback, role: "playback" as const }]
    : [
        { page: origin, role: "origin" as const },
        ...(playback !== null && !samePage(origin, playback)
          ? [{ page: playback, role: "playback" as const }]
          : [])
      ];
  if (pages.length === 0) return null;

  return (
    <HStack gap={2} vAlign="center" wrap="wrap">
      {pages.map(({ page, role }) => (
        <Link
          key={`${role}:${page.provider}:${page.url}`}
          href={page.url}
          isExternalLink
          isStandalone
          type="supporting"
          newTabLabel="（新しいタブで開きます）"
        >
          {providerLinkLabel(page.provider, role)}
        </Link>
      ))}
    </HStack>
  );
}

function providerLinkLabel(
  provider: MediaProvider,
  role: "origin" | "playback"
): string {
  if (role === "playback") {
    return provider === "soundcloud" ? "SoundCloudで再生" : "YouTubeで再生";
  }
  switch (provider) {
    case "spotify":
      return "Spotifyで開く";
    case "apple_music":
      return "Apple Musicで開く";
    case "youtube":
      return "YouTubeで開く";
    case "soundcloud":
      return "SoundCloudで開く";
  }
}

function samePage(
  left: { readonly provider: MediaProvider; readonly url: string },
  right: { readonly provider: MediaProvider; readonly url: string }
): boolean {
  return left.provider === right.provider && left.url === right.url;
}
