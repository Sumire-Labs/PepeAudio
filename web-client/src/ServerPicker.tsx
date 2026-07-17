import { useCallback, useEffect, useState } from 'react';
import { api, UnauthorizedError, type GuildSummary } from './api.ts';
import { cx, EqualizerBars, Icons, Spinner } from './ui.tsx';

export function ServerPicker({
  selectedGuildId,
  onSelect,
  onUnauthorized,
}: {
  selectedGuildId: string | null;
  onSelect: (guildId: string) => void;
  onUnauthorized: () => void;
}) {
  const [guilds, setGuilds] = useState<GuildSummary[] | null>(null);

  const load = useCallback(() => {
    api
      .getGuilds()
      .then(({ guilds }) => setGuilds(guilds))
      .catch((err) => {
        if (err instanceof UnauthorizedError) onUnauthorized();
        else setGuilds([]);
      });
  }, [onUnauthorized]);

  useEffect(load, [load]);

  if (guilds === null) {
    return (
      <div className="grid h-full place-items-center text-[var(--text-dim)]">
        <Spinner className="h-7 w-7" />
      </div>
    );
  }

  return (
    <div className="mx-auto w-full max-w-4xl px-6 py-10 fade-in">
      <div className="mb-6 flex items-end justify-between">
        <div>
          <h1 className="text-3xl font-semibold tracking-tight">サーバー</h1>
          <p className="mt-1 text-sm text-[var(--text-dim)]">操作するサーバーを選んでください。</p>
        </div>
        <button
          onClick={load}
          className="rounded-full px-3 py-1.5 text-sm text-[var(--text-dim)] transition hover:bg-[var(--track-bg)]"
        >
          更新
        </button>
      </div>

      {guilds.length === 0 ? (
        <div className="glass rounded-3xl p-10 text-center text-[var(--text-dim)]">
          <Icons.Server className="mx-auto h-10 w-10 opacity-50" />
          <p className="mt-3">共通のサーバーが見つかりませんでした。</p>
          <p className="mt-1 text-sm text-[var(--text-faint)]">Bot が参加しているサーバーにあなたも参加している必要があります。</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {guilds.map((g) => (
            <button
              key={g.guildId}
              onClick={() => onSelect(g.guildId)}
              className={cx(
                'glass group flex items-center gap-3.5 rounded-3xl p-3.5 text-left transition-all duration-200 hover:-translate-y-0.5',
                selectedGuildId === g.guildId ? 'ring-2 ring-[var(--accent)]' : '',
              )}
              style={{ boxShadow: '0 10px 30px var(--shadow)' }}
            >
              <GuildIcon guild={g} />
              <div className="min-w-0 flex-1">
                <div className="truncate font-medium">{g.name}</div>
                <div className="mt-0.5 flex items-center gap-1.5 text-xs text-[var(--text-dim)]">
                  {g.status === 'playing' ? (
                    <>
                      <EqualizerBars className="h-3" />
                      <span className="truncate">{g.currentTitle ?? '再生中'}</span>
                    </>
                  ) : g.hasActiveSession ? (
                    <span>一時停止中</span>
                  ) : (
                    <span className="text-[var(--text-faint)]">待機中</span>
                  )}
                </div>
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function GuildIcon({ guild }: { guild: GuildSummary }) {
  if (guild.iconUrl) {
    return <img src={guild.iconUrl} alt="" className="h-12 w-12 flex-none rounded-2xl object-cover" />;
  }
  return (
    <div className="grid h-12 w-12 flex-none place-items-center rounded-2xl bg-[var(--track-bg)] text-lg font-semibold text-[var(--text-dim)]">
      {guild.name.slice(0, 1).toUpperCase()}
    </div>
  );
}
