// SPDX-License-Identifier: Apache-2.0
"use client";
import { useCallback, useEffect, useState } from "react";
import { useMe, useGuilds } from "@/hooks/useAuth";
import { usePlayerHub } from "@/hooks/usePlayerHub";
import { useAccent } from "@/hooks/useAccent";
import { useKeyboardShortcuts } from "@/hooks/useKeyboard";
import { useToast } from "@/components/toast";
import Ambient from "@/components/Ambient";
import { Sidebar } from "@/components/Sidebar";
import { Player } from "@/components/Player";
import { Queue } from "@/components/Queue";
import { MiniPlayer } from "@/components/MiniPlayer";
import { Playlists } from "@/components/Playlists";
import { AddToPlaylistModal } from "@/components/AddToPlaylistModal";
import { LoginScreen } from "@/components/LoginScreen";
import { Icons, Spinner } from "@/components/ui";
import type { QueueItem } from "@/lib/types";

type View = "player" | "playlists";

export default function Home() {
  const { data: me, isLoading, isError } = useMe();

  if (isLoading)
    return (
      <>
        <Ambient thumb={null} />
        <div className="grid h-screen place-items-center">
          <Spinner className="h-8 w-8 text-[var(--text-dim)]" />
        </div>
      </>
    );
  if (isError || !me) return <LoginScreen />;
  return <Shell />;
}

function Shell() {
  const { data: me } = useMe();
  const { data: guilds } = useGuilds();
  const toast = useToast();

  const [selectedGuildId, setSelectedGuildId] = useState<string | null>(null);
  const [view, setView] = useState<View>("player");
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [savingTrack, setSavingTrack] = useState<QueueItem | null>(null);
  const [playlistsVersion, setPlaylistsVersion] = useState(0);

  useEffect(() => {
    const g = localStorage.getItem("pepe-guild");
    if (g) setSelectedGuildId(g);
  }, []);

  const session = usePlayerHub(selectedGuildId);
  useAccent(session.snapshot?.current?.thumbnailUrl ?? null);
  useKeyboardShortcuts(session, Boolean(selectedGuildId) && view === "player");

  const selectGuild = (id: string) => {
    setSelectedGuildId(id);
    localStorage.setItem("pepe-guild", id);
    setView("player");
    setSidebarOpen(false);
  };

  const logout = async () => {
    try {
      const { api } = await import("@/lib/api");
      await api.logout();
    } catch {
      /* ignore */
    }
    location.reload();
  };

  const loadToQueue = selectedGuildId
    ? async (sourceUrls: string[]): Promise<boolean> => {
        // Success toast is left to the caller (it has the playlist name); we only surface errors.
        try {
          for (const url of sourceUrls) await session.cmd.play(url);
          return true;
        } catch (e) {
          toast(e instanceof Error ? e.message : "追加に失敗しました。", "error");
          return false;
        }
      }
    : null;

  if (!me) return null;
  const current = session.snapshot?.current ?? null;

  return (
    <>
      <Ambient thumb={current?.thumbnailUrl ?? null} />
      <div className="flex h-screen">
        <button
          type="button"
          aria-label="メニュー"
          onClick={() => setSidebarOpen(true)}
          className="glass-strong fixed left-3 top-3 z-20 grid h-10 w-10 place-items-center rounded-xl md:hidden"
        >
          <Icons.Menu className="h-5 w-5" />
        </button>
        {sidebarOpen ? (
          <div className="fixed inset-0 z-30 bg-black/40 md:hidden" onClick={() => setSidebarOpen(false)} />
        ) : null}

        <Sidebar
          me={me}
          guilds={guilds ?? []}
          selectedGuildId={selectedGuildId}
          view={view}
          open={sidebarOpen}
          onSelectGuild={selectGuild}
          onView={(v) => {
            setView(v);
            setSidebarOpen(false);
          }}
          onCloseSidebar={() => setSidebarOpen(false)}
          onLogout={logout}
        />
        <div className="side-spacer flex-none" />

        <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
          <div className="relative min-h-0 flex-1">
            {view === "playlists" ? (
              <Playlists onLoadToQueue={loadToQueue} reloadKey={playlistsVersion} />
            ) : selectedGuildId ? (
              <PlayerLayout session={session} onSaveTrack={setSavingTrack} />
            ) : (
              <NoGuildPrompt hasGuilds={(guilds?.length ?? 0) > 0} onOpenSidebar={() => setSidebarOpen(true)} />
            )}

            {selectedGuildId && view === "player" && !session.connected ? (
              <div className="pointer-events-none absolute left-1/2 top-3 z-10 -translate-x-1/2">
                <div className="glass-strong flex items-center gap-2 rounded-full px-3 py-1.5 text-xs text-[var(--text-dim)]">
                  <span className="h-2 w-2 animate-pulse rounded-full bg-amber-400" /> 接続中…
                </div>
              </div>
            ) : null}
          </div>

          {view !== "player" && current ? (
            <MiniPlayer session={session} onExpand={() => setView("player")} />
          ) : null}
        </main>
      </div>

      {savingTrack ? (
        <AddToPlaylistModal
          track={savingTrack}
          onClose={() => setSavingTrack(null)}
          onAdded={() => setPlaylistsVersion((v) => v + 1)}
        />
      ) : null}
    </>
  );
}

function PlayerLayout({
  session,
  onSaveTrack,
}: {
  session: ReturnType<typeof usePlayerHub>;
  onSaveTrack: (t: QueueItem) => void;
}) {
  return (
    <div className="h-full overflow-y-auto soft-scroll lg:overflow-hidden">
      <div className="grid min-h-full gap-4 p-4 lg:h-full lg:grid-cols-[1fr_23rem]">
        <div className="min-h-0">
          <Player session={session} onSaveTrack={onSaveTrack} />
        </div>
        <div className="glass flex min-h-[60vh] flex-col rounded-3xl p-4 lg:min-h-0">
          <Queue session={session} onSaveTrack={onSaveTrack} />
        </div>
      </div>
    </div>
  );
}

function NoGuildPrompt({ hasGuilds, onOpenSidebar }: { hasGuilds: boolean; onOpenSidebar: () => void }) {
  return (
    <div className="grid h-full place-items-center px-6 text-center">
      <div className="fade-in">
        <div className="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl bg-[var(--track-bg)] text-[var(--text-dim)]">
          <Icons.Server className="h-7 w-7" />
        </div>
        <h2 className="text-lg font-semibold">{hasGuilds ? "サーバーを選択" : "共通のサーバーがありません"}</h2>
        <p className="mt-1 max-w-xs text-sm text-[var(--text-dim)]">
          {hasGuilds
            ? "サイドバーから操作するサーバーを選んでください。"
            : "Bot が参加しているサーバーにあなたも参加している必要があります。"}
        </p>
        {hasGuilds ? (
          <button onClick={onOpenSidebar} className="mt-4 rounded-2xl accent-bg px-4 py-2 text-sm font-medium text-white md:hidden">
            サーバーを選ぶ
          </button>
        ) : null}
      </div>
    </div>
  );
}
