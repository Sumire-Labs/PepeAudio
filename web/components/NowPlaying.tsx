// SPDX-License-Identifier: Apache-2.0
"use client";
import Image from "next/image";
import type { Control, PlayerState } from "@/lib/types";
import { formatTime, sourceLabel } from "@/lib/format";
import { Headphones } from "@/components/icons";
import { TransportControls } from "./TransportControls";

function EqualizerBars() {
  return (
    <div className="flex h-3.5 items-end gap-[3px]">
      {[0, 0.2, 0.4].map((d, i) => (
        <span
          key={i}
          className="w-[3px] origin-bottom rounded-full accent-bg"
          style={{ height: "100%", animation: `equalize 0.9s ease-in-out ${d}s infinite` }}
        />
      ))}
    </div>
  );
}

export function NowPlaying({ state, control }: { state: PlayerState; control: (a: Control) => void }) {
  const t = state.current;
  const pct = t && t.durationMs > 0 ? Math.min(100, (state.positionMs / t.durationMs) * 100) : 0;

  return (
    <section className="flex flex-col items-center gap-8 px-8 py-10 fade-in">
      <div
        className="relative aspect-square w-full max-w-[min(58vh,22rem)] overflow-hidden rounded-[28px]"
        style={{ boxShadow: "0 30px 70px var(--shadow)" }}
      >
        {t?.thumbnailUrl ? (
          <Image src={t.thumbnailUrl} alt="" fill className="object-cover" unoptimized />
        ) : (
          <div className="grid h-full w-full place-items-center bg-[var(--track-bg)]">
            <Headphones className="h-16 w-16 text-[var(--text-faint)]" />
          </div>
        )}
        {state.isPlaying && t ? (
          <div className="glass-strong absolute bottom-3 left-3 grid h-9 w-9 place-items-center rounded-full">
            <EqualizerBars />
          </div>
        ) : null}
      </div>

      <div className="w-full max-w-md text-center">
        <h1 className="truncate text-2xl font-semibold tracking-tight">{t?.title ?? "再生中の曲はありません"}</h1>
        <p className="mt-1 truncate text-[var(--text-dim)]">{t?.artist ?? "検索またはリンクを貼り付けて開始"}</p>
        {t ? (
          <p className="mt-2 text-xs text-[var(--text-faint)]">{sourceLabel[t.source] ?? ""}</p>
        ) : null}
      </div>

      <div className="w-full max-w-md">
        <div className="h-1.5 w-full overflow-hidden rounded-full" style={{ background: "var(--track-bg)" }}>
          <div className="h-full rounded-full accent-bg transition-[width] duration-200 ease-linear" style={{ width: `${pct}%` }} />
        </div>
        <div className="mt-1.5 flex justify-between text-xs tabular-nums text-[var(--text-dim)]">
          <span>{formatTime(state.positionMs)}</span>
          <span>{t && t.durationMs > 0 ? formatTime(t.durationMs) : t?.isLive ? "ライブ" : "--:--"}</span>
        </div>
      </div>

      <TransportControls state={state} control={control} />
    </section>
  );
}
