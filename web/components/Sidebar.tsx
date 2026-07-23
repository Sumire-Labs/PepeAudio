// SPDX-License-Identifier: Apache-2.0
"use client";
import Image from "next/image";
import { useGuilds, useMe } from "@/hooks/useAuth";
import { api } from "@/lib/api";
import { guildIconUrl } from "@/lib/format";
import { Logout } from "@/components/icons";

export function Sidebar({ selected, onSelect }: { selected: string | null; onSelect: (id: string) => void }) {
  const { data: me } = useMe();
  const { data: guilds, isLoading } = useGuilds();

  return (
    <aside
      className="glass-strong flex w-72 shrink-0 flex-col gap-4 p-3"
      style={{ boxShadow: "0 0 40px var(--shadow)" }}
    >
      <header className="mb-1 flex h-9 items-center gap-2.5 px-1">
        <div className="h-9 w-9 flex-none overflow-hidden rounded-xl bg-white">
          <Image src="/pepeaudio-icon.png" alt="PepeAudio" width={36} height={36} className="h-full w-full object-cover" priority />
        </div>
        <span className="whitespace-nowrap text-lg font-semibold tracking-tight">PepeAudio</span>
      </header>

      <div className="flex items-center justify-between px-2">
        <span className="text-xs font-semibold uppercase tracking-wide text-[var(--text-faint)]">サーバー</span>
        {guilds ? <span className="text-xs text-[var(--text-faint)]">{guilds.length}</span> : null}
      </div>

      <nav className="min-h-0 flex-1 space-y-0.5 overflow-y-auto soft-scroll">
        {isLoading && <div className="px-2 text-sm text-[var(--text-faint)]">読み込み中…</div>}
        {guilds?.length === 0 && (
          <div className="px-2 py-4 text-sm text-[var(--text-faint)]">Botを管理できるサーバーがありません。</div>
        )}
        {guilds?.map((g) => {
          const icon = guildIconUrl(g.id, g.icon);
          const isSelected = selected === g.id;
          return (
            <button
              key={g.id}
              onClick={() => onSelect(g.id)}
              title={g.name}
              style={isSelected ? { background: "color-mix(in srgb, var(--accent) 20%, transparent)" } : undefined}
              className={`flex w-full items-center gap-2.5 rounded-xl p-1.5 text-left transition ${
                isSelected ? "" : "hover:bg-[var(--track-bg)]"
              }`}
            >
              {icon ? (
                <Image src={icon} alt="" width={36} height={36} className="h-9 w-9 flex-none rounded-xl object-cover" unoptimized />
              ) : (
                <span className="grid h-9 w-9 flex-none place-items-center rounded-xl bg-[var(--track-bg)] text-sm font-semibold text-[var(--text-dim)]">
                  {g.name.slice(0, 1).toUpperCase()}
                </span>
              )}
              <span className="min-w-0 flex-1 truncate text-sm font-medium">{g.name}</span>
            </button>
          );
        })}
      </nav>

      <div className="mt-2 flex items-center gap-2 border-t border-[var(--hairline)] pt-2">
        <div className="grid h-9 w-9 flex-none place-items-center rounded-full bg-[var(--track-bg)] text-sm text-[var(--text-dim)]">
          {me?.username?.slice(0, 1).toUpperCase()}
        </div>
        <span className="min-w-0 flex-1 truncate text-sm text-[var(--text-dim)]">{me?.username}</span>
        <button
          onClick={() => api.logout().then(() => location.reload())}
          aria-label="ログアウト"
          title="ログアウト"
          className="grid h-8 w-8 flex-none place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)] hover:accent"
        >
          <Logout className="h-4 w-4" />
        </button>
      </div>
    </aside>
  );
}
