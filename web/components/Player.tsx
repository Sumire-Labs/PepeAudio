// SPDX-License-Identifier: Apache-2.0
"use client";
import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import type { LoopMode, QueueItem } from "@/lib/types";
import type { PlayerSession } from "@/hooks/usePlayerHub";
import { formatMs } from "@/lib/format";
import { cx, EqualizerBars, Icons, IconButton, Menu } from "@/components/ui";

// 250ms ticker — re-renders while playing so the seek bar advances smoothly.
function useTicker(active: boolean): number {
  const [, set] = useState(0);
  useEffect(() => {
    if (!active) return;
    const id = setInterval(() => set((t) => t + 1), 250);
    return () => clearInterval(id);
  }, [active]);
  return Date.now();
}

export function Player({ session, onSaveTrack }: { session: PlayerSession; onSaveTrack: (t: QueueItem) => void }) {
  const { snapshot, receivedAt, cmd } = session;

  const current = snapshot?.current ?? null;
  const playing = snapshot?.status === "playing";
  const now = useTicker(playing);
  // durationMs is 0 for live/unknown streams — treat as null (no cap, ticks freely).
  const duration = current && current.durationMs > 0 ? current.durationMs : null;
  const elapsed = current
    ? Math.min(duration ?? Number.MAX_SAFE_INTEGER, snapshot!.positionMs + (playing ? now - receivedAt : 0))
    : 0;

  return (
    <div className="relative flex h-full flex-col items-center justify-center px-6 py-8 fade-in">
      {/* top-right: save + overflow (stop) — keeps the main control area uncluttered */}
      <div className="absolute right-4 top-4 z-10 flex items-center gap-1.5">
        <button
          type="button"
          aria-label="プレイリストに保存"
          title="現在の曲をプレイリストに保存"
          disabled={!current}
          onClick={() => current && onSaveTrack(current)}
          className="glass grid h-9 w-9 place-items-center rounded-full text-[var(--text-dim)] transition hover:text-[var(--text)] active:scale-90 disabled:opacity-40"
        >
          <Icons.Bookmark className="h-5 w-5" />
        </button>
        <Menu
          items={[
            {
              label: "停止して退出",
              icon: Icons.Stop,
              danger: true,
              disabled: !current,
              onClick: () => cmd.stop(),
            },
          ]}
        />
      </div>

      <Artwork track={current} playing={playing} />

      <div className="mt-7 w-full max-w-md text-center">
        <h2 className="truncate text-2xl font-semibold tracking-tight">{current?.title ?? "再生していません"}</h2>
        <p className="mt-1 truncate text-[var(--text-dim)]">{current?.artist ?? ""}</p>
        {current?.requesterName ? (
          <div className="mt-2 inline-flex items-center gap-1.5 text-xs text-[var(--text-faint)]">
            {current.requesterAvatarUrl ? <img src={current.requesterAvatarUrl} alt="" className="h-4 w-4 rounded-full" /> : null}
            <span>{current.requesterName} がリクエスト</span>
          </div>
        ) : null}
      </div>

      <SeekBar
        elapsed={elapsed}
        duration={duration}
        isLive={Boolean(current?.isLive)}
        canSeek={!!current}
        onSeek={(ms) => cmd.seek(ms)}
      />

      {/* transport */}
      <div className="mt-6 flex items-center gap-4">
        <IconButton icon={Icons.Shuffle} label="シャッフル" size="md" active={Boolean(snapshot?.shuffle)} onClick={() => cmd.toggleShuffle()} />
        <IconButton icon={Icons.Prev} label="前へ" size="md" onClick={() => cmd.previous()} />
        <button
          type="button"
          aria-label={playing ? "一時停止" : "再生"}
          disabled={!current}
          onClick={() => cmd.playPause()}
          className={cx(
            "grid h-16 w-16 place-items-center rounded-full accent-bg text-white shadow-xl transition-transform duration-150 active:scale-90",
            !current ? "opacity-40" : "hover:brightness-110",
          )}
        >
          {playing ? <Icons.Pause className="h-7 w-7" /> : <Icons.Play className="ml-0.5 h-7 w-7" />}
        </button>
        <IconButton icon={Icons.Next} label="スキップ" size="md" onClick={() => cmd.skip()} />
        <LoopButton mode={snapshot?.loopMode ?? "off"} onCycle={(mode) => cmd.setLoop(mode)} />
      </div>

      {/* volume */}
      <div className="mt-6 w-full max-w-md">
        <VolumeControl value={snapshot?.volume ?? 100} onChange={(v) => cmd.setVolume(v)} />
      </div>

      {/* playback modes — centered row */}
      <div className="mt-6 flex w-full max-w-md flex-wrap items-center justify-center gap-2">
        <Chip icon={Icons.Radio} label="オートプレイ" active={Boolean(snapshot?.autoplay)} onClick={() => cmd.setAutoplay(!snapshot?.autoplay)} />
        <Chip icon={Icons.Spatial} label="Aura" active={Boolean(snapshot?.auraEnabled)} onClick={() => cmd.toggleAura()} />
      </div>
    </div>
  );
}

function SeekBar({
  elapsed,
  duration,
  isLive,
  canSeek,
  onSeek,
}: {
  elapsed: number;
  duration: number | null;
  isLive: boolean;
  canSeek: boolean;
  onSeek: (ms: number) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [scrub, setScrub] = useState<number | null>(null);
  const seekable = canSeek && !!duration && duration > 0;
  const value = scrub ?? elapsed;
  const fraction = duration && duration > 0 ? Math.max(0, Math.min(1, value / duration)) : 0;

  const posFromClientX = (clientX: number): number => {
    const rect = ref.current?.getBoundingClientRect();
    if (!rect || !duration) return 0;
    const f = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return f * duration;
  };

  const onDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!seekable) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setScrub(posFromClientX(e.clientX));
  };
  const onMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (scrub === null) return;
    setScrub(posFromClientX(e.clientX));
  };
  const onUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (scrub === null) return;
    const pos = posFromClientX(e.clientX);
    setScrub(null);
    onSeek(Math.floor(pos));
  };

  return (
    <div className="mt-6 w-full max-w-md">
      <div
        ref={ref}
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        className={cx("group relative flex h-4 touch-none items-center", seekable ? "cursor-pointer" : "")}
      >
        <div className="h-1.5 w-full overflow-hidden rounded-full" style={{ background: "var(--track-bg)" }}>
          <div
            className={cx("h-full rounded-full accent-bg", scrub === null ? "transition-[width] duration-200 ease-linear" : "")}
            style={{ width: `${fraction * 100}%` }}
          />
        </div>
        {seekable ? (
          <div
            className="pointer-events-none absolute h-3 w-3 -translate-x-1/2 rounded-full bg-white opacity-0 shadow transition group-hover:opacity-100"
            style={{ left: `${fraction * 100}%` }}
          />
        ) : null}
      </div>
      <div className="mt-1.5 flex justify-between text-xs tabular-nums text-[var(--text-dim)]">
        <span>{formatMs(value)}</span>
        <span>{duration ? formatMs(duration) : isLive ? "LIVE" : "--:--"}</span>
      </div>
    </div>
  );
}

function Artwork({ track, playing }: { track: QueueItem | null; playing: boolean }) {
  return (
    <div className="relative aspect-square w-full max-w-[min(58vh,22rem)] overflow-hidden rounded-[28px]" style={{ boxShadow: "0 30px 70px var(--shadow)" }}>
      {track?.thumbnailUrl ? (
        <img src={track.thumbnailUrl} alt="" className="h-full w-full object-cover" />
      ) : (
        <div className="grid h-full w-full place-items-center bg-[var(--track-bg)]">
          <Icons.Headphones className="h-16 w-16 text-[var(--text-faint)]" />
        </div>
      )}
      {playing ? (
        <div className="glass-strong absolute bottom-3 left-3 grid h-9 w-9 place-items-center rounded-full">
          <EqualizerBars className="h-3.5" />
        </div>
      ) : null}
    </div>
  );
}

function LoopButton({ mode, onCycle }: { mode: LoopMode; onCycle: (m: LoopMode) => void }) {
  const next: LoopMode = mode === "off" ? "track" : mode === "track" ? "queue" : "off";
  const Icon = mode === "track" ? Icons.RepeatOne : Icons.Repeat;
  const label = mode === "track" ? "1曲リピート" : mode === "queue" ? "キューをリピート" : "リピートオフ";
  return <IconButton icon={Icon} label={label} active={mode !== "off"} onClick={() => onCycle(next)} />;
}

// Backend volume scale is 0–200 (MaxVolume=200). Debounced commit so dragging doesn't flood the hub.
function VolumeControl({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  const [local, setLocal] = useState(value);
  const timer = useRef<number | null>(null);
  const prevVolume = useRef(value || 100);
  const dragging = useRef(false);
  // Follow server pushes unless the user is mid-drag.
  useEffect(() => {
    if (!dragging.current) setLocal(value);
  }, [value]);

  const commit = (v: number) => {
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => onChange(v), 250);
  };

  const toggleMute = () => {
    if (local > 0) {
      prevVolume.current = local;
      setLocal(0);
      onChange(0);
    } else {
      const restore = prevVolume.current || 100;
      setLocal(restore);
      onChange(restore);
    }
  };

  const Ico = local === 0 ? Icons.VolumeMute : Icons.Volume;
  const fill = (local / 200) * 100;
  return (
    <div className="flex flex-1 items-center gap-2.5">
      <button
        type="button"
        onClick={toggleMute}
        aria-label={local === 0 ? "ミュート解除" : "ミュート"}
        title={local === 0 ? "ミュート解除" : "ミュート"}
        className="grid h-8 w-8 flex-none place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)]"
      >
        <Ico className="h-5 w-5" />
      </button>
      <input
        type="range"
        min={0}
        max={200}
        step={5}
        value={local}
        onPointerDown={() => (dragging.current = true)}
        onPointerUp={() => (dragging.current = false)}
        onChange={(e) => {
          const v = Number(e.target.value);
          setLocal(v);
          commit(v);
        }}
        className="h-1.5 w-full cursor-pointer appearance-none rounded-full"
        style={{
          accentColor: "var(--accent)",
          background: `linear-gradient(to right, var(--accent) ${fill}%, var(--track-bg) ${fill}%)`,
          borderRadius: 999,
        }}
      />
      <span className="w-8 flex-none text-right text-xs tabular-nums text-[var(--text-dim)]">{local}</span>
    </div>
  );
}

function Chip({
  icon: Icon,
  label,
  active,
  onClick,
}: {
  icon: (p: { className?: string }) => ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cx(
        "flex items-center gap-2 rounded-full px-3.5 py-2 text-sm font-medium transition-all duration-200 active:scale-95",
        active ? "accent-bg text-white" : "glass text-[var(--text-dim)]",
      )}
    >
      <Icon className="h-4 w-4" />
      {label}
    </button>
  );
}
