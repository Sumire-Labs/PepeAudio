import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { api, UnauthorizedError, type GuildSummary, type Me, type PlaylistSummary, type PlaylistTrackDTO, type QueueItemDTO } from './api.ts';
import { useGuildSession } from './useGuildSession.ts';
import { ToastProvider, useToast } from './toast.tsx';
import { Ambient } from './Ambient.tsx';
import { Login } from './Login.tsx';
import { Player } from './Player.tsx';
import { Queue } from './Queue.tsx';
import { Playlists } from './Playlists.tsx';
import { useAccent } from './useAccent.ts';
import { useKeyboardShortcuts } from './useKeyboard.ts';
import { cx, EqualizerBars, IconButton, Icons, Spinner } from './ui.tsx';
import type { GuildSession } from './useGuildSession.ts';

type Theme = 'auto' | 'light' | 'dark';
type View = 'player' | 'playlists';

export function App() {
  return (
    <ToastProvider>
      <Root />
    </ToastProvider>
  );
}

function Root() {
  const [me, setMe] = useState<Me | null | undefined>(undefined);
  const [Demo, setDemo] = useState<null | (() => ReactNode)>(null);
  const isDemo = import.meta.env.DEV && new URLSearchParams(window.location.search).has('demo');

  useEffect(() => {
    if (isDemo) {
      // Dynamic import so the demo module is never in a production bundle.
      void import('./Demo.tsx').then((m) => setDemo(() => m.Demo));
      return;
    }
    api
      .getMe()
      .then(setMe)
      .catch(() => setMe(null));
  }, [isDemo]);

  if (isDemo) return Demo ? <Demo /> : null;

  if (me === undefined) {
    return (
      <div className="grid h-full place-items-center">
        <Ambient url={null} />
        <Spinner className="h-8 w-8 text-[var(--text-dim)]" />
      </div>
    );
  }

  if (me === null) {
    return (
      <>
        <Ambient url={null} />
        <Login />
      </>
    );
  }

  return <Shell me={me} />;
}

function toPlaylistTrack(t: QueueItemDTO): PlaylistTrackDTO {
  return { sourceUrl: t.sourceUrl, title: t.title, artist: t.artist, thumbnailUrl: t.thumbnailUrl, sourceType: t.sourceType, durationMs: t.durationMs };
}

function Shell({ me }: { me: Me }) {
  const toast = useToast();
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem('pepe-theme') as Theme) || 'auto');
  const [selectedGuildId, setSelectedGuildId] = useState<string | null>(() => localStorage.getItem('pepe-guild'));
  const [view, setView] = useState<View>('player');
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [guilds, setGuilds] = useState<GuildSummary[]>([]);
  const [savingTrack, setSavingTrack] = useState<QueueItemDTO | null>(null);
  const [playlistsVersion, setPlaylistsVersion] = useState(0);

  const [sessionExpired, setSessionExpired] = useState(false);
  const onUnauthorized = useCallback(() => {
    // Session expired — show a gentle re-login prompt instead of a hard reload.
    setSessionExpired(true);
  }, []);

  const session = useGuildSession(selectedGuildId, onUnauthorized);
  const ambientUrl = session.snapshot?.current?.thumbnailUrl ?? null;
  useAccent(ambientUrl);
  useKeyboardShortcuts(session, Boolean(selectedGuildId));

  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'auto') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', theme);
    localStorage.setItem('pepe-theme', theme);
  }, [theme]);

  useEffect(() => {
    let active = true;
    const load = () =>
      api
        .getGuilds()
        .then(({ guilds }) => {
          if (active) setGuilds(guilds);
        })
        .catch(() => {});
    void load();
    // Refresh the server list periodically so the sidebar's now-playing badges stay live.
    const id = setInterval(() => {
      if (!document.hidden) void load();
    }, 15_000);
    return () => {
      active = false;
      clearInterval(id);
    };
  }, []);

  const selectGuild = (id: string) => {
    setSelectedGuildId(id);
    localStorage.setItem('pepe-guild', id);
    setView('player');
    setSidebarOpen(false);
  };

  const loadToQueue = selectedGuildId
    ? async (sourceUrls: string[]): Promise<boolean> => {
        const result = await session.sendCommand({ type: 'loadPlaylist', sourceUrls });
        if (!result.ok && result.error) toast(result.error, 'error');
        return result.ok;
      }
    : null;

  const logout = async () => {
    try {
      await api.logout();
    } catch {
      /* ignore */
    }
    window.location.reload();
  };

  return (
    <>
      <Ambient url={ambientUrl} />
      <div className="flex h-full">
        <button
          type="button"
          aria-label="メニュー"
          onClick={() => setSidebarOpen(true)}
          className="glass-strong fixed left-3 top-3 z-20 grid h-10 w-10 place-items-center rounded-xl md:hidden"
        >
          <Icons.Menu className="h-5 w-5" />
        </button>
        {sidebarOpen ? <div className="fixed inset-0 z-30 bg-black/40 md:hidden" onClick={() => setSidebarOpen(false)} /> : null}

        <Sidebar
          me={me}
          guilds={guilds}
          selectedGuildId={selectedGuildId}
          view={view}
          theme={theme}
          open={sidebarOpen}
          onSelectGuild={selectGuild}
          onView={(v) => {
            setView(v);
            setSidebarOpen(false);
          }}
          onCloseSidebar={() => setSidebarOpen(false)}
          onCycleTheme={() => setTheme(theme === 'auto' ? 'dark' : theme === 'dark' ? 'light' : 'auto')}
          onLogout={logout}
        />
        {/* Desktop spacer: reserves the collapsed rail's width (the sidebar itself is a fixed overlay that expands on hover). */}
        <div className="side-spacer flex-none" />

        <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
          <div className="relative min-h-0 flex-1">
            {view === 'playlists' ? (
              <Playlists onLoadToQueue={loadToQueue} onUnauthorized={onUnauthorized} reloadKey={playlistsVersion} />
            ) : selectedGuildId ? (
              <PlayerLayout session={session} onSaveTrack={setSavingTrack} />
            ) : (
              <NoGuildPrompt hasGuilds={guilds.length > 0} onOpenSidebar={() => setSidebarOpen(true)} />
            )}

            {selectedGuildId && view === 'player' && !session.loading && !session.connected ? (
              <div className="pointer-events-none absolute left-1/2 top-3 z-10 -translate-x-1/2">
                <div className="glass-strong flex items-center gap-2 rounded-full px-3 py-1.5 text-xs text-[var(--text-dim)]">
                  <span className="h-2 w-2 animate-pulse rounded-full bg-amber-400" /> 再接続中…
                </div>
              </div>
            ) : null}
          </div>

          {view !== 'player' && session.snapshot?.current ? (
            <MiniPlayer session={session} onExpand={() => setView('player')} />
          ) : null}
        </main>
      </div>

      {savingTrack ? (
        <AddToPlaylistModal
          track={savingTrack}
          onClose={() => setSavingTrack(null)}
          onUnauthorized={onUnauthorized}
          onAdded={() => setPlaylistsVersion((v) => v + 1)}
        />
      ) : null}
      {sessionExpired ? <SessionExpiredOverlay /> : null}
    </>
  );
}

function NoGuildPrompt({ hasGuilds, onOpenSidebar }: { hasGuilds: boolean; onOpenSidebar: () => void }) {
  return (
    <div className="grid h-full place-items-center px-6 text-center">
      <div className="fade-in">
        <div className="mx-auto mb-4 grid h-14 w-14 place-items-center rounded-2xl bg-[var(--track-bg)] text-[var(--text-dim)]">
          <Icons.Server className="h-7 w-7" />
        </div>
        <h2 className="text-lg font-semibold">{hasGuilds ? 'サーバーを選択' : '共通のサーバーがありません'}</h2>
        <p className="mt-1 max-w-xs text-sm text-[var(--text-dim)]">
          {hasGuilds ? 'サイドバーから操作するサーバーを選んでください。' : 'Bot が参加しているサーバーにあなたも参加している必要があります。'}
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

function MiniPlayer({ session, onExpand }: { session: GuildSession; onExpand: () => void }) {
  const { snapshot, sendCommand } = session;
  const current = snapshot?.current;
  if (!current) return null;
  const playing = snapshot?.status === 'playing';
  const canControl = snapshot?.viewer.canControl ?? false;

  return (
    <div className="glass-strong m-3 flex items-center gap-3 rounded-2xl p-2.5" style={{ boxShadow: '0 12px 40px var(--shadow)' }}>
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
      <IconButton icon={Icons.Prev} label="前へ" size="sm" disabled={!canControl} onClick={() => void sendCommand({ type: 'previous' })} />
      <button
        onClick={() => void sendCommand({ type: 'togglePlayPause' })}
        disabled={!canControl}
        aria-label={playing ? '一時停止' : '再生'}
        className="grid h-10 w-10 flex-none place-items-center rounded-full accent-bg text-white transition active:scale-90 disabled:opacity-40"
      >
        {playing ? <Icons.Pause className="h-5 w-5" /> : <Icons.Play className="ml-0.5 h-5 w-5" />}
      </button>
      <IconButton icon={Icons.Next} label="スキップ" size="sm" disabled={!canControl} onClick={() => void sendCommand({ type: 'skip' })} />
    </div>
  );
}

function SessionExpiredOverlay() {
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/50 p-6 fade-in">
      <div className="glass-strong w-full max-w-sm rounded-3xl p-8 text-center" style={{ boxShadow: '0 30px 80px var(--shadow)' }}>
        <h2 className="text-lg font-semibold">セッションが切れました</h2>
        <p className="mt-2 text-sm text-[var(--text-dim)]">もう一度 Discord でログインしてください。</p>
        <a href="/auth/login" className="mt-5 inline-block rounded-2xl accent-bg px-5 py-2.5 font-medium text-white transition hover:brightness-110">
          再ログイン
        </a>
      </div>
    </div>
  );
}

function PlayerLayout({ session, onSaveTrack }: { session: ReturnType<typeof useGuildSession>; onSaveTrack: (t: QueueItemDTO) => void }) {
  if (session.loading) {
    return (
      <div className="grid h-full place-items-center text-[var(--text-dim)]">
        <Spinner className="h-7 w-7" />
      </div>
    );
  }
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

export function Sidebar({
  me,
  guilds,
  selectedGuildId,
  view,
  theme,
  open,
  onSelectGuild,
  onView,
  onCloseSidebar,
  onCycleTheme,
  onLogout,
}: {
  me: Me;
  guilds: GuildSummary[];
  selectedGuildId: string | null;
  view: View;
  theme: Theme;
  open: boolean;
  onSelectGuild: (id: string) => void;
  onView: (v: View) => void;
  onCloseSidebar: () => void;
  onCycleTheme: () => void;
  onLogout: () => void;
}) {
  const [railHover, setRailHover] = useState(false);
  const [isDesktop, setIsDesktop] = useState(() => window.matchMedia('(min-width: 768px)').matches);
  useEffect(() => {
    const mq = window.matchMedia('(min-width: 768px)');
    const onChange = () => setIsDesktop(mq.matches);
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);
  const ThemeIcon = theme === 'light' ? Icons.Sun : theme === 'dark' ? Icons.Moon : Icons.Spatial;
  return (
    <aside
      onMouseEnter={() => setRailHover(true)}
      onMouseLeave={() => setRailHover(false)}
      className={cx(
        // Mobile: full-width drawer toggled by `open`. Desktop: a 4.5rem icon
        // rail that expands to full width while hovered. Width is set INLINE on
        // desktop (highest priority, avoids a class-cascade override) and falls
        // back to the CSS drawer width on mobile. Labels/centering key off the
        // `is-open` class via .rail-* rules (see index.css).
        'glass-strong side-rail fixed left-0 top-0 z-40 flex h-full flex-col overflow-hidden p-3 transition-[width,transform] duration-300 ease-out',
        open ? 'translate-x-0' : '-translate-x-full',
        'md:translate-x-0',
        railHover ? 'is-open' : '',
      )}
      style={{ boxShadow: '0 0 40px var(--shadow)', ...(isDesktop ? { width: railHover ? '15.5rem' : '4.5rem' } : {}) }}
    >
      <header className="rail-center mb-3 flex h-9 items-center gap-2.5">
        <div className="grid h-9 w-9 flex-none place-items-center rounded-xl accent-bg text-white">
          <Icons.Headphones className="h-5 w-5" />
        </div>
        <span className="rail-label whitespace-nowrap text-lg font-semibold tracking-tight">PepeAudio</span>
        <button onClick={onCloseSidebar} aria-label="閉じる" className="ml-auto grid h-8 w-8 flex-none place-items-center rounded-full text-[var(--text-dim)] hover:bg-[var(--track-bg)] md:hidden">
          <Icons.Close className="h-4 w-4" />
        </button>
      </header>

      <nav className="mb-3 flex flex-col gap-1">
        <NavRow icon={Icons.Play} label="プレイヤー" active={view === 'player'} onClick={() => onView('player')} />
        <NavRow icon={Icons.Playlist} label="プレイリスト" active={view === 'playlists'} onClick={() => onView('playlists')} />
      </nav>

      <div className="rail-label mb-1 flex items-center justify-between px-2">
        <span className="whitespace-nowrap text-xs font-semibold uppercase tracking-wide text-[var(--text-faint)]">サーバー</span>
        <span className="text-xs text-[var(--text-faint)]">{guilds.length}</span>
      </div>
      {/* thin divider shown only on the collapsed desktop rail */}
      <div className="rail-collapsed-only mb-1 hidden h-px bg-[var(--hairline)] md:block" />

      <div className="min-h-0 flex-1 space-y-0.5 overflow-y-auto soft-scroll">
        {guilds.length === 0 ? (
          <p className="rail-label whitespace-nowrap px-2 py-4 text-sm text-[var(--text-faint)]">共通のサーバーがありません。</p>
        ) : (
          guilds.map((g) => {
            const selected = selectedGuildId === g.guildId;
            return (
              <button
                key={g.guildId}
                onClick={() => onSelectGuild(g.guildId)}
                title={g.name}
                style={selected ? { background: 'color-mix(in srgb, var(--accent) 20%, transparent)' } : undefined}
                className={cx('rail-center flex w-full items-center gap-2.5 rounded-xl p-1.5 text-left transition', selected ? '' : 'hover:bg-[var(--track-bg)]')}
              >
                <div className="relative flex-none">
                  {g.iconUrl ? (
                    <img src={g.iconUrl} alt="" className="h-9 w-9 rounded-xl object-cover" />
                  ) : (
                    <div className="grid h-9 w-9 place-items-center rounded-xl bg-[var(--track-bg)] text-sm font-semibold text-[var(--text-dim)]">{g.name.slice(0, 1).toUpperCase()}</div>
                  )}
                  {g.status === 'playing' ? <span className="rail-collapsed-only absolute -bottom-0.5 -right-0.5 h-2.5 w-2.5 rounded-full border-2 border-[var(--surface-strong)] accent-bg" /> : null}
                </div>
                <div className="rail-label min-w-0 flex-1">
                  <div className="truncate text-sm font-medium">{g.name}</div>
                  <div className="flex items-center gap-1 text-xs text-[var(--text-dim)]">
                    {g.status === 'playing' ? (
                      <>
                        <EqualizerBars className="h-2.5" />
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
            );
          })
        )}
      </div>

      {/* footer: avatar stays centered when collapsed; the rest reveals on expand */}
      <div className="rail-center mt-2 flex items-center gap-2 border-t border-[var(--hairline)] pt-2">
        {me.avatarUrl ? (
          <img src={me.avatarUrl} alt="" className="h-9 w-9 flex-none rounded-full object-cover" />
        ) : (
          <div className="grid h-9 w-9 flex-none place-items-center rounded-full bg-[var(--track-bg)] text-sm">{me.username.slice(0, 1).toUpperCase()}</div>
        )}
        <span className="rail-label min-w-0 flex-1 truncate text-sm">{me.username}</span>
        <div className="rail-label flex flex-none items-center gap-1">
          <button onClick={onCycleTheme} aria-label={`テーマ: ${theme}`} title={`テーマ: ${theme}`} className="grid h-8 w-8 place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)]">
            <ThemeIcon className="h-4 w-4" />
          </button>
          <button onClick={onLogout} aria-label="ログアウト" title="ログアウト" className="grid h-8 w-8 place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)] hover:accent">
            <Icons.Logout className="h-4 w-4" />
          </button>
        </div>
      </div>
    </aside>
  );
}

function NavRow({ icon: Icon, label, active, onClick }: { icon: (p: { className?: string }) => ReactNode; label: string; active: boolean; onClick: () => void }) {
  return (
    <button onClick={onClick} title={label} className="rail-center flex items-center gap-2.5 rounded-xl p-1.5 text-left transition hover:bg-[var(--track-bg)]">
      <span className={cx('grid h-9 w-9 flex-none place-items-center rounded-xl transition', active ? 'accent-bg text-white' : 'bg-[var(--track-bg)] text-[var(--text-dim)]')}>
        <Icon className="h-5 w-5" />
      </span>
      <span className={cx('rail-label whitespace-nowrap text-sm font-medium', active ? 'text-[var(--text)]' : 'text-[var(--text-dim)]')}>{label}</span>
    </button>
  );
}

function AddToPlaylistModal({ track, onClose, onUnauthorized, onAdded }: { track: QueueItemDTO; onClose: () => void; onUnauthorized: () => void; onAdded: () => void }) {
  const toast = useToast();
  const [list, setList] = useState<PlaylistSummary[] | null>(null);
  const [newName, setNewName] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .listPlaylists()
      .then(({ playlists }) => setList(playlists))
      .catch((err) => {
        if (err instanceof UnauthorizedError) onUnauthorized();
        else setList([]);
      });
  }, [onUnauthorized]);

  const addTo = async (id: string) => {
    if (busy) return;
    setBusy(true);
    try {
      await api.addPlaylistTrack(id, toPlaylistTrack(track));
      toast('プレイリストに追加しました。');
      onAdded();
      onClose();
    } catch (err) {
      toast(err instanceof Error ? err.message : '失敗しました。', 'error');
      setBusy(false);
    }
  };

  const createAndAdd = async () => {
    const name = newName.trim();
    if (!name || busy) return;
    setBusy(true);
    try {
      const { playlist } = await api.createPlaylist(name);
      await api.addPlaylistTrack(playlist.id, toPlaylistTrack(track));
      toast(`「${name}」に追加しました。`);
      onAdded();
      onClose();
    } catch (err) {
      toast(err instanceof Error ? err.message : '失敗しました。', 'error');
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/40 p-6 fade-in" onClick={onClose}>
      <div className="glass-strong w-full max-w-sm rounded-3xl p-5" onClick={(e) => e.stopPropagation()} style={{ boxShadow: '0 30px 80px var(--shadow)' }}>
        <div className="mb-3 flex items-center justify-between">
          <h3 className="text-lg font-semibold">プレイリストに保存</h3>
          <button onClick={onClose} aria-label="閉じる" className="grid h-8 w-8 place-items-center rounded-full text-[var(--text-dim)] hover:bg-[var(--track-bg)]">
            <Icons.Close className="h-4 w-4" />
          </button>
        </div>
        <p className="mb-3 truncate text-sm text-[var(--text-dim)]">{track.title}</p>

        <div className="mb-3 flex gap-2">
          <input value={newName} onChange={(e) => setNewName(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && createAndAdd()} placeholder="新規プレイリスト名" className="glass w-full rounded-xl px-3 py-2 text-sm outline-none" />
          <button onClick={createAndAdd} disabled={!newName.trim() || busy} className="rounded-xl accent-bg px-3 text-sm text-white disabled:opacity-40">作成</button>
        </div>

        <div className="max-h-64 space-y-1 overflow-y-auto soft-scroll">
          {list === null ? (
            <div className="grid place-items-center py-6"><Spinner className="h-6 w-6" /></div>
          ) : list.length === 0 ? (
            <p className="py-4 text-center text-sm text-[var(--text-faint)]">まだプレイリストがありません。</p>
          ) : (
            list.map((p) => (
              <button key={p.id} onClick={() => addTo(p.id)} disabled={busy} className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-2 text-left transition hover:bg-[var(--track-bg)] disabled:opacity-50">
                <div className="grid h-9 w-9 flex-none place-items-center rounded-lg accent-bg text-white"><Icons.Playlist className="h-4 w-4" /></div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium">{p.name}</div>
                  <div className="text-xs text-[var(--text-dim)]">{p.trackCount} 曲</div>
                </div>
                <Icons.Plus className="h-4 w-4 flex-none text-[var(--text-dim)]" />
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
