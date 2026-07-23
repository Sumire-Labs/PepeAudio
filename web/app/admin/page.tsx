// SPDX-License-Identifier: Apache-2.0
"use client";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { useMe } from "@/hooks/useAuth";
import { LoginScreen } from "@/components/LoginScreen";
import { Icons } from "@/components/ui";

export default function Admin() {
  const { data: me, isLoading, isError } = useMe();
  const overview = useQuery({ queryKey: ["admin"], queryFn: api.adminOverview, retry: false, refetchInterval: 5000, enabled: !!me });

  if (isLoading) return <Centered>読み込み中…</Centered>;
  if (isError || !me) return <LoginScreen />;
  if (overview.isError) return <Centered>管理ダッシュボードへのアクセス権限がありません。</Centered>;

  const o = overview.data;
  return (
    <main className="mx-auto max-w-5xl px-6 py-12 fade-in">
      <h1 className="text-2xl font-semibold tracking-tight">管理ダッシュボード</h1>
      <div className="mt-6 grid grid-cols-2 gap-4 sm:grid-cols-4">
        <Stat icon={Icons.Server} label="サーバー" value={o?.botGuilds} />
        <Stat icon={Icons.Headphones} label="アクティブなボイス" value={o?.activeVoices} />
        <Stat icon={Icons.Spatial} label="シャード" value={o?.shards} />
        <Stat icon={Icons.Play} label="再生中" value={o?.players.filter((p) => p.playing).length} />
      </div>

      <h2 className="mt-10 mb-3 px-1 text-xs font-semibold uppercase tracking-wide text-[var(--text-faint)]">アクティブなプレイヤー</h2>
      <div className="glass overflow-hidden rounded-3xl">
        {o?.players.length === 0 && <div className="p-6 text-sm text-[var(--text-dim)]">アクティブなプレイヤーはありません。</div>}
        {o?.players.map((p) => (
          <div key={p.id} className="hairline flex items-center gap-4 border-b px-5 py-3.5 last:border-0">
            <span className="relative flex h-2.5 w-2.5 flex-none">
              {p.playing ? <span className="absolute inline-flex h-full w-full animate-ping rounded-full accent-bg opacity-60" /> : null}
              <span className={`relative inline-flex h-2.5 w-2.5 rounded-full ${p.playing ? "accent-bg" : "bg-[var(--text-faint)]"}`} />
            </span>
            <span className="w-48 truncate font-medium">{p.name}</span>
            <span className="min-w-0 flex-1 truncate text-sm text-[var(--text-dim)]">{p.current ?? "待機中"}</span>
            <span className="flex-none text-xs text-[var(--text-faint)]">{p.queue} 件キュー待ち</span>
          </div>
        ))}
      </div>
    </main>
  );
}

function Stat({ icon: Icon, label, value }: { icon: (p: { className?: string }) => React.ReactNode; label: string; value: number | undefined }) {
  return (
    <div className="glass rounded-3xl p-5">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium uppercase tracking-wide text-[var(--text-faint)]">{label}</span>
        <span className="grid h-8 w-8 flex-none place-items-center rounded-xl bg-[var(--track-bg)] text-[var(--text-dim)]">
          <Icon className="h-4 w-4" />
        </span>
      </div>
      <div className="mt-3 text-3xl font-semibold tabular-nums">{value ?? "—"}</div>
    </div>
  );
}

function Centered({ children }: { children: React.ReactNode }) {
  return <div className="grid min-h-screen place-items-center text-[var(--text-dim)]">{children}</div>;
}
