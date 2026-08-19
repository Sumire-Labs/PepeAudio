import { Icon } from "@astryxdesign/core/Icon";
import { Slider } from "@astryxdesign/core/Slider";
import { HStack, StackItem } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { Volume1, Volume2, VolumeX } from "lucide-react";

import { formatDuration } from "../app/progress";
import { playerBarStyles } from "./player-bar.styles";

interface PlayerVolumeControlProps {
  readonly value: number;
  readonly isDisabled: boolean;
  readonly disabledMessage: string;
  readonly onChange: (value: number) => void;
  readonly onChangeEnd: (value: number) => void;
}

export function PlayerVolumeControl({
  value,
  isDisabled,
  disabledMessage,
  onChange,
  onChangeEnd
}: PlayerVolumeControlProps) {
  const volumeIcon = value === 0 ? VolumeX : value > 35 ? Volume2 : Volume1;

  return (
    <HStack gap={2} padding={2} vAlign="center" xstyle={playerBarStyles.volumeZone}>
      <Icon
        icon={volumeIcon}
        color="secondary"
        data-testid="player-volume-icon"
      />
      <StackItem size="fill">
        <Slider
          label="音量"
          isLabelHidden
          value={value}
          min={0}
          max={100}
          step={1}
          width="100%"
          valueDisplay="none"
          formatValue={(nextValue) => `${nextValue}%`}
          isDisabled={isDisabled}
          disabledMessage={disabledMessage}
          onChange={onChange}
          onChangeEnd={onChangeEnd}
        />
      </StackItem>
      <Text
        type="code"
        color="secondary"
        hasTabularNumbers
        xstyle={playerBarStyles.rangeValue}
      >
        {value}%
      </Text>
    </HStack>
  );
}

interface PlayerSeekControlProps {
  readonly positionMs: number;
  readonly durationMs: number | null;
  readonly hasTrack: boolean;
  readonly canSeek: boolean;
  readonly isDisabled: boolean;
  readonly disabledMessage: string;
  readonly onChange: (value: number) => void;
  readonly onChangeEnd: (value: number) => void;
}

export function PlayerSeekControl({
  positionMs,
  durationMs,
  hasTrack,
  canSeek,
  isDisabled,
  disabledMessage,
  onChange,
  onChangeEnd
}: PlayerSeekControlProps) {
  const sliderMax = Math.max(1, durationMs ?? 0);
  const endLabel = hasTrack ? formatDuration(durationMs) : "0:00";

  return (
    <HStack gap={3} vAlign="center" xstyle={playerBarStyles.seekRow}>
      <Text
        type="code"
        color="secondary"
        hasTabularNumbers
        xstyle={playerBarStyles.rangeValue}
      >
        {formatDuration(positionMs)}
      </Text>
      <StackItem size="fill">
        <Slider
          label="再生位置"
          isLabelHidden
          value={Math.min(positionMs, sliderMax)}
          min={0}
          max={sliderMax}
          step={1_000}
          width="100%"
          valueDisplay="none"
          formatValue={formatDuration}
          isDisabled={!canSeek || isDisabled}
          disabledMessage={disabledMessage}
          onChange={onChange}
          onChangeEnd={onChangeEnd}
        />
      </StackItem>
      <Text
        type="code"
        color="secondary"
        hasTabularNumbers
        xstyle={playerBarStyles.rangeValue}
      >
        {endLabel}
      </Text>
    </HStack>
  );
}
