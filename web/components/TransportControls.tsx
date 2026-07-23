// SPDX-License-Identifier: Apache-2.0
"use client";
import type { Control, PlayerState } from "@/lib/types";
import {
  Play, Pause, Next, Prev, Stop, Shuffle, Repeat, RepeatOne,
  Headphones, VolumeHigh, VolumeMute,
} from "@/components/icons";
import type { ComponentType } from "react";

type Ico = ComponentType<{ className?: string }>;

const IconBtn = ({ icon: Icon, onClick, active, label }: {
  icon: Ico; onClick: () => void; active?: boolean; label: string;
}) => (
  <button
    type="button"
    aria-label={label}
    title={label}
    onClick={onClick}
    className={`grid h-11 w-11 place-items-center rounded-full transition active:scale-90 ${
      active
        ? "accent-bg text-white"
        : "glass text-[var(--text-dim)] hover:text-[var(--text)]"
    }`}
  >
    <Icon className="h-5 w-5" />
  </button>
);

const Chip = ({ icon: Icon, onClick, active, label }: {
  icon: Ico; onClick: () => void; active: boolean; label: string;
}) => (
  <button
    type="button"
    onClick={onClick}
    className={`flex items-center gap-2 rounded-full px-3.5 py-2 text-sm font-medium transition-all duration-200 active:scale-95 ${
      active ? "accent-bg text-white" : "glass text-[var(--text-dim)]"
    }`}
  >
    <Icon className="h-4 w-4" />
    {label}
  </button>
);

export function TransportControls({ state, control }: {
  state: PlayerState; control: (a: Control) => void;
}) {
  const LoopIcon = state.loop === 1 ? RepeatOne : Repeat;
  return (
    <div className="flex flex-col items-center gap-6">
      {/* transport */}
      <div className="flex items-center gap-4">
        <IconBtn icon={Shuffle} label="シャッフル" active={state.shuffle} onClick={() => control("Shuffle")} />
        <IconBtn icon={Prev} label="前へ" onClick={() => control("Previous")} />
        <button
          type="button"
          aria-label={state.isPlaying ? "一時停止" : "再生"}
          onClick={() => control("PlayPause")}
          className="grid h-16 w-16 place-items-center rounded-full accent-bg text-white shadow-xl transition-transform duration-150 hover:brightness-110 active:scale-90"
        >
          {state.isPlaying ? <Pause className="h-7 w-7" /> : <Play className="ml-0.5 h-7 w-7" />}
        </button>
        <IconBtn icon={Next} label="スキップ" onClick={() => control("Skip")} />
        <IconBtn icon={LoopIcon} label="リピート" active={state.loop !== 0} onClick={() => control("Loop")} />
        <IconBtn icon={Stop} label="停止" onClick={() => control("Stop")} />
      </div>

      {/* volume */}
      <div className="flex items-center gap-2.5">
        <IconBtn icon={VolumeMute} label="音量を下げる" onClick={() => control("VolumeDown")} />
        <span className="w-12 text-center text-sm tabular-nums text-[var(--text-dim)]">{state.volume}%</span>
        <IconBtn icon={VolumeHigh} label="音量を上げる" onClick={() => control("VolumeUp")} />
      </div>

      {/* playback modes */}
      <div className="flex flex-wrap items-center justify-center gap-2">
        <Chip icon={Headphones} label="Aura" active={state.auraEnabled} onClick={() => control("ToggleAura")} />
      </div>
    </div>
  );
}
