import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Card } from "@astryxdesign/core/Card";
import { Center } from "@astryxdesign/core/Center";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Grid } from "@astryxdesign/core/Grid";
import { Icon } from "@astryxdesign/core/Icon";
import { NavIcon } from "@astryxdesign/core/NavIcon";
import { Section } from "@astryxdesign/core/Section";
import { Spinner } from "@astryxdesign/core/Spinner";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { Heading, Text } from "@astryxdesign/core/Text";
import { Token } from "@astryxdesign/core/Token";
import { AudioLines, Headphones, UserRound } from "lucide-react";
import { memo, useState } from "react";

import type { GuildSummary, PlayerSnapshot } from "../app/types";
import { nowPlayingStyles } from "./now-playing.styles";
import { TrackTitleLink } from "./TrackTitleLink";

interface NowPlayingProps {
  readonly guild: GuildSummary | undefined;
  readonly snapshot: PlayerSnapshot;
}

export const NowPlaying = memo(function NowPlaying({ guild, snapshot }: NowPlayingProps) {
  const track = snapshot.track;

  return (
    <Section variant="transparent" padding={6} xstyle={nowPlayingStyles.root}>
      <VStack gap={5}>
        <Heading level={1} maxLines={1}>
          {guild?.name ?? "サーバーを選択"}
        </Heading>

        {snapshot.state === "loading" && track === null ? (
          <Card width="100%" padding={0} variant="muted">
            <Center minHeight={320}>
              <EmptyState
                headingLevel={2}
                title="曲を準備しています"
                description="準備が完了すると自動的に再生が始まります。"
                icon={<Spinner size="lg" />}
              />
            </Center>
          </Card>
        ) : track === null ? (
          <Card width="100%" padding={0} variant="muted">
            <Center minHeight={320}>
              <EmptyState
                headingLevel={2}
                title="まだ何も再生していません"
                description="上の検索欄に曲名またはURLを入力して追加できます。"
                icon={<Icon icon={Headphones} size="lg" color="secondary" />}
              />
            </Center>
          </Card>
        ) : (
          <Card width="100%" padding={5} variant="muted">
            <Grid columns={{ minWidth: 240, max: 2, repeat: "fit" }} gap={6} align="center">
              <HeroArtwork title={track.title} url={track.artworkUrl} />

              <VStack gap={4} xstyle={nowPlayingStyles.trackDetails}>
                <HStack gap={2} vAlign="center">
                  <StatusDot
                    variant={playbackStatusVariant(snapshot.state)}
                    label={describeTrackStatus(snapshot.state)}
                    isPulsing={snapshot.state === "playing"}
                  />
                  <Text type="label" color="secondary">
                    {describeTrackStatus(snapshot.state)}
                  </Text>
                </HStack>
                <Heading level={2} type="display-3" wordBreak="break-word">
                  <TrackTitleLink
                    title={track.title}
                    provenance={track.provenance}
                    maxLines={3}
                  />
                </Heading>
                {track.artist ? (
                  <Text type="large" color="secondary" maxLines={2}>
                    {track.artist}
                  </Text>
                ) : null}
                <HStack gap={2} wrap="wrap">
                  {track.album ? <Token size="sm" label={track.album} /> : null}
                  {track.requestedBy ? (
                    <Token
                      size="sm"
                      label={`リクエスト: ${track.requestedBy}`}
                      icon={<Icon icon={UserRound} size="xsm" />}
                    />
                  ) : null}
                </HStack>
              </VStack>
            </Grid>
          </Card>
        )}
      </VStack>
    </Section>
  );
});

function HeroArtwork({ title, url }: { readonly title: string; readonly url: string | null }) {
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const safeUrl = safeArtworkUrl(url);
  const shownUrl = safeUrl !== null && safeUrl !== failedUrl ? safeUrl : null;

  return (
    <AspectRatio ratio={1} fit="cover" xstyle={nowPlayingStyles.artworkFrame}>
      {shownUrl === null ? (
        <Center width="100%" height="100%" xstyle={nowPlayingStyles.artworkSurface}>
          <NavIcon icon={<Icon icon={AudioLines} size="lg" color="secondary" />} />
        </Center>
      ) : (
        <img
          src={shownUrl}
          alt={`${title}のアートワーク`}
          decoding="async"
          referrerPolicy="no-referrer"
          onError={() => setFailedUrl(shownUrl)}
        />
      )}
    </AspectRatio>
  );
}

function safeArtworkUrl(value: string | null): string | null {
  if (value === null) return null;
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname === "i.ytimg.com"
      ? url.href
      : null;
  } catch {
    return null;
  }
}

type StatusVariant = "success" | "warning" | "accent" | "neutral";

function playbackStatusVariant(state: PlayerSnapshot["state"]): StatusVariant {
  switch (state) {
    case "playing":
      return "success";
    case "paused":
      return "warning";
    case "loading":
      return "accent";
    case "idle_connected":
    case "disconnected":
      return "neutral";
  }
}

function describeTrackStatus(state: PlayerSnapshot["state"]): string {
  switch (state) {
    case "playing":
      return "Discordで再生中";
    case "paused":
      return "一時停止中";
    case "loading":
      return "読み込み中";
    case "idle_connected":
      return "再生待ち";
    case "disconnected":
      return "再生停止中";
  }
}
