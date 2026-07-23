// SPDX-License-Identifier: Apache-2.0
"use client";
import { useState } from "react";
import { useMe } from "@/hooks/useAuth";
import { usePlayerHub } from "@/hooks/usePlayerHub";
import { usePlayerStore } from "@/stores/playerStore";
import { useAccent } from "@/hooks/useAccent";
import Ambient from "@/components/Ambient";
import { Sidebar } from "@/components/Sidebar";
import { NowPlaying } from "@/components/NowPlaying";
import { Queue } from "@/components/Queue";
import { SearchBar } from "@/components/SearchBar";
import { LoginScreen } from "@/components/LoginScreen";
import { Server } from "@/components/icons";

export default function Home() {
  const { data: me, isLoading, isError } = useMe();
  const [guildId, setGuildId] = useState<string | null>(null);
  const state = usePlayerStore((s) => s.state);
  const { control, play, reorder, remove } = usePlayerHub(guildId);

  const thumb = state?.current?.thumbnailUrl ?? null;
  useAccent(thumb);

  if (isLoading)
    return (
      <>
        <Ambient thumb={null} />
        <div className="grid min-h-screen place-items-center text-[var(--text-dim)]">読み込み中…</div>
      </>
    );
  if (isError || !me) return <LoginScreen />;

  return (
    <>
      <Ambient thumb={thumb} />
      <div className="flex h-screen overflow-hidden">
        <Sidebar selected={guildId} onSelect={setGuildId} />

        <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
          {guildId ? (
            <>
              <SearchBar onPlay={play} />
              <div className="min-h-0 flex-1 overflow-y-auto soft-scroll">
                {state ? (
                  <NowPlaying state={state} control={control} />
                ) : (
                  <div className="grid h-full place-items-center px-6 text-center fade-in">
                    <div className="glass-strong flex items-center gap-2 rounded-full px-3 py-1.5 text-xs text-[var(--text-dim)]">
                      <span className="h-2 w-2 animate-pulse rounded-full accent-bg" /> 接続中…
                    </div>
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="grid h-full place-items-center px-6 text-center">
              <div className="fade-in">
                <div className="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl bg-[var(--track-bg)] text-[var(--text-dim)]">
                  <Server className="h-7 w-7" />
                </div>
                <p className="max-w-xs text-sm text-[var(--text-dim)]">サーバーを選択してください。</p>
              </div>
            </div>
          )}
        </main>

        {guildId && state && (
          <div className="hidden shrink-0 p-3 lg:flex">
            <div
              className="glass flex h-full overflow-hidden rounded-3xl"
              style={{ boxShadow: "0 12px 40px var(--shadow)" }}
            >
              <Queue state={state} reorder={reorder} remove={remove} />
            </div>
          </div>
        )}
      </div>
    </>
  );
}
