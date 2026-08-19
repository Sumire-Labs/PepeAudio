import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { ToggleButton } from "@astryxdesign/core/ToggleButton";
import { Toolbar } from "@astryxdesign/core/Toolbar";
import {
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Square
} from "lucide-react";

import type { DashboardModel } from "../app/types";
import { playerBarStyles } from "./player-bar.styles";

interface PlayerTransportControlsProps {
  readonly model: DashboardModel;
  readonly controlsUnavailable: boolean;
  readonly unavailableMessage: string;
}

export function PlayerTransportControls({
  model,
  controlsUnavailable,
  unavailableMessage
}: PlayerTransportControlsProps) {
  const { snapshot } = model;
  const { track } = snapshot;
  const playing = snapshot.state === "playing";
  const repeatActionLabel =
    `リピート設定を切り替え（現在: ${repeatLabel(snapshot.repeatMode)}）`;

  return (
    <Toolbar
      label="再生コントロール"
      size="lg"
      gap={1}
      xstyle={playerBarStyles.transportToolbar}
      centerContent={
        <>
          <ToggleButton
            label="シャッフル"
            tooltip={controlsUnavailable ? unavailableMessage : "シャッフル"}
            icon={<Icon icon={Shuffle} />}
            isIconOnly
            isPressed={snapshot.shuffleEnabled}
            isDisabled={controlsUnavailable}
            onPressedChange={(pressed) => {
              if (pressed !== snapshot.shuffleEnabled) model.toggleShuffle();
            }}
          />
          <IconButton
            label="前の曲"
            tooltip={controlsUnavailable ? unavailableMessage : "前の曲"}
            icon={<Icon icon={SkipBack} />}
            variant="ghost"
            isDisabled={!snapshot.hasPreviousTrack || controlsUnavailable}
            onClick={model.previous}
          />
          <IconButton
            label={playing ? "一時停止" : "再生"}
            tooltip={controlsUnavailable ? unavailableMessage : playing ? "一時停止" : "再生"}
            icon={<Icon icon={playing ? Pause : Play} />}
            variant="primary"
            isDisabled={track === null || controlsUnavailable}
            onClick={model.togglePlayback}
          />
          <IconButton
            label="次の曲"
            tooltip={controlsUnavailable ? unavailableMessage : "次の曲"}
            icon={<Icon icon={SkipForward} />}
            variant="ghost"
            isDisabled={track === null || controlsUnavailable}
            onClick={model.skip}
          />
          <IconButton
            label={repeatActionLabel}
            tooltip={controlsUnavailable ? unavailableMessage : repeatActionLabel}
            icon={<Icon icon={snapshot.repeatMode === "track" ? Repeat1 : Repeat} />}
            variant={snapshot.repeatMode === "off" ? "ghost" : "secondary"}
            isDisabled={controlsUnavailable}
            onClick={model.cycleRepeat}
          />
          <IconButton
            label="停止"
            tooltip={controlsUnavailable ? unavailableMessage : "停止"}
            icon={<Icon icon={Square} />}
            variant="ghost"
            isDisabled={track === null || controlsUnavailable}
            onClick={model.stop}
          />
        </>
      }
    />
  );
}

function repeatLabel(mode: DashboardModel["snapshot"]["repeatMode"]): string {
  return mode === "off" ? "オフ" : mode === "track" ? "1曲" : "キュー";
}
