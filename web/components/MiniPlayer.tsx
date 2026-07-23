// SPDX-License-Identifier: Apache-2.0
"use client";
import { EqualizerBars, IconButton, Icons } from "@/components/ui";
import type { PlayerSession } from "@/hooks/usePlayerHub";

// Compact now-playing bar shown when the player view isn't active.
export function MiniPlayer({ session, onExpand }: { session: PlayerSession; onExpand: () => void }) {
  const { snapshot, cmd } = session;
  const current = snapshot?.current;
  if (!current) return null;
  const playing = snapshot?.status === "playing";

  return (
    <div className="glass-strong m-3 flex items-center gap-3 rounded-2xl p-2.5" style={{ boxShadow: "0 12px 40px var(--shadow)" }}>
      <button onClick={onExpand} className="flex min-w-0 flex-1 items-center gap-3 text-left" title="プレイヤーを開く">
        {current.thumbnailUrl ? (
          <img src={current.thumbnailUrl} alt="" className="h-11 w-11 flex-none rounded-lg object-cover" />
        ) : (
          <div className="grid h-11 w-11 flex-none place-items-center rounded-lg bg-[var(--track-bg)]">
            <Icons.Headphones className="h-5 w-5 text-[var(--text-faint)]" />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">{current.title}</div>
          <div className="truncate text-xs text-[var(--text-dim)]">{current.artist}</div>
        </div>
        {playing ? <EqualizerBars className="mr-1 h-3.5 flex-none" /> : null}
      </button>
      <IconButton icon={Icons.Prev} label="前へ" size="sm" onClick={() => cmd.previous()} />
      <button
        onClick={() => cmd.playPause()}
        aria-label={playing ? "一時停止" : "再生"}
        className="grid h-10 w-10 flex-none place-items-center rounded-full accent-bg text-white transition active:scale-90"
      >
        {playing ? <Icons.Pause className="h-5 w-5" /> : <Icons.Play className="ml-0.5 h-5 w-5" />}
      </button>
      <IconButton icon={Icons.Next} label="スキップ" size="sm" onClick={() => cmd.skip()} />
    </div>
  );
}
