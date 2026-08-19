import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Badge } from "@astryxdesign/core/Badge";
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
import {
  AudioLines,
  CirclePlay,
  Headphones,
  Orbit,
  Radio,
  UserRound
} from "lucide-react";
import { memo, type ReactNode } from "react";

import type { GuildSummary, PlayerSnapshot } from "../app/types";
import { nowPlayingStyles } from "./now-playing.styles";
import { TrackSourceLinks } from "./TrackSourceLink";

interface NowPlayingProps {
  readonly guild: GuildSummary | undefined;
  readonly snapshot: PlayerSnapshot;
}

export const NowPlaying = memo(function NowPlaying({ guild, snapshot }: NowPlayingProps) {
  const track = snapshot.track;

  return (
    <Section variant="transparent" padding={6} xstyle={nowPlayingStyles.root}>
      <VStack gap={6}>
        <HStack gap={4} vAlign="center" hAlign="between" wrap="wrap">
          <VStack gap={1}>
            <Text type="supporting" color="secondary">
              再生ワークスペース
            </Text>
            <Heading level={1} maxLines={1}>
              {guild?.name ?? "サーバーを選択"}
            </Heading>
          </VStack>
        </HStack>

        {snapshot.state === "loading" && track === null ? (
          <Card width="100%" padding={0}>
            <Center minHeight={260}>
              <EmptyState
                headingLevel={2}
                title="次の曲を読み込んでいます"
                description="準備が完了すると、この画面へ自動的に反映されます。"
                icon={<Spinner size="lg" />}
              />
            </Center>
          </Card>
        ) : track === null ? (
          <Card width="100%" padding={0}>
            <Center minHeight={260}>
              <EmptyState
                headingLevel={2}
                title="再生待ちです"
                description="Discordで /play を使うと、この画面へリアルタイムに反映されます。"
                icon={<Icon icon={Headphones} size="lg" />}
              />
            </Center>
          </Card>
        ) : (
          <Card width="100%" padding={5}>
            <Grid columns={{ minWidth: 220, max: 2, repeat: "fit" }} gap={6} align="center">
              <VStack width="100%" maxWidth={360} hAlign="center">
                <AspectRatio ratio={1} xstyle={nowPlayingStyles.artworkFrame}>
                  <Center
                    width="100%"
                    height="100%"
                    padding={6}
                    xstyle={nowPlayingStyles.artworkSurface}
                  >
                    <VStack gap={4} hAlign="center" justify="center" height="100%">
                      <NavIcon icon={<Icon icon={AudioLines} size="lg" color="inherit" />} />
                      <Text type="supporting" color="secondary">
                        PepeAudio ストリーム
                      </Text>
                      {snapshot.spatialEnabled ? (
                        <Badge variant="info" label="360°" />
                      ) : null}
                    </VStack>
                  </Center>
                </AspectRatio>
              </VStack>

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
                <Heading level={2} type="display-3" maxLines={3} wordBreak="break-word">
                  {track.title}
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
                <TrackSourceLinks provenance={track.provenance} />
              </VStack>
            </Grid>
          </Card>
        )}

        <Grid columns={{ minWidth: 180, max: 3, repeat: "fit" }} gap={3}>
          <SignalMetric
            label="再生"
            value={describePlayback(snapshot.state)}
            variant={playbackStatusVariant(snapshot.state)}
            icon={<Icon icon={CirclePlay} size="sm" color="secondary" />}
          />
          <SignalMetric
            label="ボイス"
            value={
              !snapshot.voiceConnected
                ? "未接続"
                : snapshot.voiceChannelName ?? "接続中"
            }
            description={
              snapshot.voiceConnected &&
              guild !== undefined &&
              guild.listenerCount !== null
                ? `${guild.listenerCount}人が参加中`
                : undefined
            }
            variant={snapshot.voiceConnected ? "success" : "neutral"}
            icon={<Icon icon={Radio} size="sm" color="secondary" />}
          />
          <SignalMetric
            label="360° Audio"
            value={snapshot.spatialEnabled ? "HRIR適用中" : "オフ"}
            variant={snapshot.spatialEnabled ? "accent" : "neutral"}
            icon={<Icon icon={Orbit} size="sm" color="secondary" />}
          />
        </Grid>
      </VStack>
    </Section>
  );
});

type StatusVariant = "success" | "warning" | "error" | "accent" | "neutral";

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

function describePlayback(state: PlayerSnapshot["state"]): string {
  switch (state) {
    case "playing":
      return "再生中";
    case "paused":
      return "一時停止中";
    case "loading":
      return "読み込み中";
    case "idle_connected":
      return "待機中";
    case "disconnected":
      return "未接続";
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

function SignalMetric({
  label,
  value,
  variant,
  icon,
  description
}: {
  readonly label: string;
  readonly value: string;
  readonly variant: StatusVariant;
  readonly icon: ReactNode;
  readonly description?: string | undefined;
}) {
  return (
    <Card variant="muted" width="100%" minHeight={108} padding={4}>
      <VStack gap={2}>
        <HStack gap={2} vAlign="center">
          {icon}
          <Text type="supporting" color="secondary">
            {label}
          </Text>
        </HStack>
        <HStack gap={2} vAlign="center">
          <StatusDot variant={variant} label={value} />
          <Text type="label" maxLines={1}>
            {value}
          </Text>
        </HStack>
        {description ? (
          <Text type="supporting" color="secondary" maxLines={1}>
            {description}
          </Text>
        ) : null}
      </VStack>
    </Card>
  );
}
