import { Grid } from "@astryxdesign/core/Grid";
import { VStack } from "@astryxdesign/core/Stack";
import { useEffect, useRef, useState } from "react";

import { interpolatedPositionMs } from "../app/progress";
import type { DashboardModel } from "../app/types";
import { useClock } from "../app/use-clock";
import { playerBarStyles } from "./player-bar.styles";
import { PlayerSeekControl, PlayerVolumeControl } from "./PlayerRangeControls";
import { PlayerTrackSummary } from "./PlayerTrackSummary";
import { PlayerTransportControls } from "./PlayerTransportControls";

interface PlayerBarProps {
  readonly model: DashboardModel;
}

export function PlayerBar({ model }: PlayerBarProps) {
  const nowUnixMs = useClock();
  const { snapshot } = model;
  const { track } = snapshot;
  const positionMs = interpolatedPositionMs(snapshot, nowUnixMs);
  const durationMs = track?.durationMs ?? null;
  const canSeek = track?.seekable === true && durationMs !== null && durationMs > 0;
  const controlsUnavailable = !model.connected || model.commandPending;
  const unavailableMessage = controlDisabledMessage(model.connected, model.commandPending);
  const [seekDraft, setSeekDraft] = useState<number | null>(null);
  const [volumeDraft, setVolumeDraft] = useState<number | null>(null);
  const [lastSeekCommit, setLastSeekCommit] = useState<number | null>(null);
  const [lastVolumeCommit, setLastVolumeCommit] = useState<number | null>(null);
  const previousCommandPending = useRef(model.commandPending);

  useEffect(() => {
    setSeekDraft(null);
    setLastSeekCommit(null);
  }, [snapshot.revision, track?.id]);
  useEffect(() => {
    setVolumeDraft(null);
    setLastVolumeCommit(null);
  }, [snapshot.revision, snapshot.volumePercent]);
  useEffect(() => {
    const commandFinished = previousCommandPending.current && !model.commandPending;
    previousCommandPending.current = model.commandPending;
    if (commandFinished) {
      setSeekDraft(null);
      setVolumeDraft(null);
      setLastSeekCommit(null);
      setLastVolumeCommit(null);
    }
  }, [model.commandPending]);

  const displayedPosition = Math.max(0, seekDraft ?? positionMs);
  const displayedVolume = clampPercent(volumeDraft ?? snapshot.volumePercent);
  const commitSeek = (value: number) => {
    const clamped = Math.max(0, Math.min(value, durationMs ?? 0));
    setSeekDraft(clamped);
    if (clamped !== positionMs && clamped !== lastSeekCommit) {
      setLastSeekCommit(clamped);
      void model.seek(clamped);
    }
  };
  const commitVolume = (value: number) => {
    const clamped = clampPercent(value);
    setVolumeDraft(clamped);
    if (clamped !== snapshot.volumePercent && clamped !== lastVolumeCommit) {
      setLastVolumeCommit(clamped);
      void model.setVolume(clamped);
    }
  };

  return (
    <VStack gap={1} xstyle={playerBarStyles.root}>
      <Grid
        columns={{ minWidth: 240, max: 3, repeat: "fit" }}
        columnGap={4}
        rowGap={1}
        align="center"
        xstyle={playerBarStyles.playerGrid}
      >
        <PlayerTrackSummary state={snapshot.state} track={track} />
        <PlayerTransportControls
          model={model}
          controlsUnavailable={controlsUnavailable}
          unavailableMessage={unavailableMessage}
        />
        <PlayerVolumeControl
          value={displayedVolume}
          isDisabled={controlsUnavailable}
          disabledMessage={unavailableMessage}
          onChange={setVolumeDraft}
          onChangeEnd={commitVolume}
        />
      </Grid>
      <PlayerSeekControl
        positionMs={displayedPosition}
        durationMs={durationMs}
        hasTrack={track !== null}
        canSeek={canSeek}
        isDisabled={controlsUnavailable}
        disabledMessage={seekDisabledMessage(
          model.connected,
          track !== null,
          canSeek,
          model.commandPending
        )}
        onChange={setSeekDraft}
        onChangeEnd={commitSeek}
      />
    </VStack>
  );
}

function clampPercent(value: number): number {
  return Math.max(0, Math.min(100, Math.round(value)));
}

function seekDisabledMessage(
  connected: boolean,
  hasTrack: boolean,
  canSeek: boolean,
  commandPending: boolean
): string {
  if (!connected) return "プレイヤー状態の同期後に操作できます。";
  if (commandPending) return "別のプレイヤー操作を反映しています。";
  if (!hasTrack) return "再生中の曲がありません。";
  if (!canSeek) return "この曲はシークできません。";
  return "シークできません。";
}

function controlDisabledMessage(connected: boolean, commandPending: boolean): string {
  if (!connected) return "プレイヤー状態の同期後に操作できます。";
  if (commandPending) return "別のプレイヤー操作を反映しています。";
  return "現在は操作できません。";
}
