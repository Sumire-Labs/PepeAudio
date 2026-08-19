import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Center } from "@astryxdesign/core/Center";
import { Icon } from "@astryxdesign/core/Icon";
import { Item } from "@astryxdesign/core/Item";
import { AudioLines } from "lucide-react";
import { useState } from "react";

import type { PlayerState, TrackView } from "../app/types";
import { playerBarStyles } from "./player-bar.styles";

interface PlayerTrackSummaryProps {
  readonly state: PlayerState;
  readonly track: TrackView | null;
}

export function PlayerTrackSummary({ state, track }: PlayerTrackSummaryProps) {
  return (
    <Item
      density="compact"
      label={track?.title ?? (state === "loading" ? "読み込み中" : "再生待ち")}
      description={track?.artist ?? idleDescription(state)}
      labelLines={1}
      descriptionLines={1}
      startContent={<PlayerArtwork track={track} />}
      xstyle={playerBarStyles.trackSummary}
    />
  );
}

function PlayerArtwork({ track }: { readonly track: TrackView | null }) {
  const artworkUrl = displayableArtworkUrl(track?.artworkUrl ?? null);
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const showArtwork = artworkUrl !== null && artworkUrl !== failedUrl;

  return (
    <AspectRatio
      ratio={1}
      {...(showArtwork ? { fit: "cover" as const } : {})}
      xstyle={playerBarStyles.artwork}
      data-testid="player-artwork"
    >
      {showArtwork ? (
        <img
          src={artworkUrl}
          alt=""
          decoding="async"
          referrerPolicy="no-referrer"
          onError={() => setFailedUrl(artworkUrl)}
        />
      ) : (
        <Center
          width="100%"
          height="100%"
          xstyle={playerBarStyles.artworkPlaceholder}
        >
          <Icon icon={AudioLines} color="secondary" />
        </Center>
      )}
    </AspectRatio>
  );
}

function idleDescription(state: PlayerState): string {
  return state === "loading"
    ? "次の曲を準備しています"
    : "Discordの /play から追加できます";
}

function displayableArtworkUrl(value: string | null): string | null {
  if (value === null) return null;

  try {
    const url = new URL(value);
    return url.protocol === "https:" ? url.href : null;
  } catch {
    return null;
  }
}
