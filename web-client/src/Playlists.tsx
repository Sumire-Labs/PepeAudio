import { useCallback, useEffect, useState } from 'react';
import { api, UnauthorizedError, type PlaylistDetail, type PlaylistSummary, type PlaylistTrackDTO } from './api.ts';
import { cx, Icons, Spinner } from './ui.tsx';
import { useToast } from './toast.tsx';

export function Playlists({
  onLoadToQueue,
  onUnauthorized,
}: {
  onLoadToQueue: ((sourceUrls: string[]) => Promise<boolean>) | null;
  onUnauthorized: () => void;
}) {
  const toast = useToast();
  const [list, setList] = useState<PlaylistSummary[] | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<PlaylistDetail | null>(null);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState('');

  const guard = useCallback(
    (err: unknown) => {
      if (err instanceof UnauthorizedError) onUnauthorized();
      else toast(err instanceof Error ? err.message : '失敗しました。', 'error');
    },
    [onUnauthorized, toast],
  );

  const reload = useCallback(() => {
    api
      .listPlaylists()
      .then(({ playlists }) => setList(playlists))
      .catch(guard);
  }, [guard]);

  useEffect(reload, [reload]);

  useEffect(() => {
    if (!selectedId) {
      setDetail(null);
      return;
    }
    api
      .getPlaylist(selectedId)
      .then(({ playlist }) => setDetail(playlist))
      .catch(guard);
  }, [selectedId, guard]);

  const create = async () => {
    const name = newName.trim();
    if (!name) return;
    try {
      const { playlist } = await api.createPlaylist(name);
      setNewName('');
      setCreating(false);
      reload();
      setSelectedId(playlist.id);
    } catch (err) {
      guard(err);
    }
  };

  const removeTrack = async (index: number) => {
    if (!detail) return;
    const tracks = detail.tracks.filter((_, i) => i !== index);
    try {
      const { playlist } = await api.replacePlaylistTracks(detail.id, tracks);
      setDetail(playlist);
      reload();
    } catch (err) {
      guard(err);
    }
  };

  const rename = async () => {
    if (!detail) return;
    const name = window.prompt('プレイリスト名', detail.name);
    if (!name) return;
    try {
      const { playlist } = await api.renamePlaylist(detail.id, name);
      setDetail(playlist);
      reload();
    } catch (err) {
      guard(err);
    }
  };

  const remove = async () => {
    if (!detail || !window.confirm(`「${detail.name}」を削除しますか？`)) return;
    try {
      await api.deletePlaylist(detail.id);
      setSelectedId(null);
      reload();
    } catch (err) {
      guard(err);
    }
  };

  const load = async () => {
    if (!detail || !onLoadToQueue) return;
    if (detail.tracks.length === 0) {
      toast('プレイリストが空です。', 'error');
      return;
    }
    const ok = await onLoadToQueue(detail.tracks.map((t) => t.sourceUrl));
    if (ok) toast(`「${detail.name}」をキューに追加しました。`);
  };

  if (list === null) {
    return (
      <div className="grid h-full place-items-center text-[var(--text-dim)]">
        <Spinner className="h-7 w-7" />
      </div>
    );
  }

  return (
    <div className="mx-auto grid h-full w-full max-w-5xl grid-cols-1 gap-4 px-6 py-8 md:grid-cols-[18rem_1fr] fade-in">
      {/* list */}
      <div className="glass flex min-h-0 flex-col rounded-3xl p-3">
        <div className="mb-2 flex items-center justify-between px-2">
          <h2 className="text-lg font-semibold">プレイリスト</h2>
          <button onClick={() => setCreating((v) => !v)} className="grid h-8 w-8 place-items-center rounded-full accent-bg text-white active:scale-90" aria-label="新規作成">
            <Icons.Plus className="h-4 w-4" />
          </button>
        </div>
        {creating ? (
          <div className="mb-2 flex gap-2 px-1">
            <input
              autoFocus
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && create()}
              placeholder="新しいプレイリスト名"
              className="glass w-full rounded-xl px-3 py-1.5 text-sm outline-none"
            />
            <button onClick={create} className="rounded-xl accent-bg px-3 text-sm text-white">作成</button>
          </div>
        ) : null}
        <div className="min-h-0 flex-1 space-y-1 overflow-y-auto soft-scroll">
          {list.length === 0 ? (
            <p className="px-2 py-4 text-sm text-[var(--text-faint)]">まだプレイリストがありません。</p>
          ) : (
            list.map((p) => (
              <button
                key={p.id}
                onClick={() => setSelectedId(p.id)}
                className={cx('flex w-full items-center gap-2 rounded-xl px-2.5 py-2 text-left transition', selectedId === p.id ? 'bg-[var(--track-bg)]' : 'hover:bg-[var(--track-bg)]')}
              >
                <div className="grid h-9 w-9 flex-none place-items-center rounded-lg accent-bg text-white">
                  <Icons.Playlist className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium">{p.name}</div>
                  <div className="text-xs text-[var(--text-dim)]">{p.trackCount} 曲</div>
                </div>
              </button>
            ))
          )}
        </div>
      </div>

      {/* detail */}
      <div className="glass flex min-h-0 flex-col rounded-3xl p-4">
        {!detail ? (
          <div className="grid h-full place-items-center text-[var(--text-faint)]">プレイリストを選択してください。</div>
        ) : (
          <>
            <div className="mb-3 flex items-center gap-2">
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-xl font-semibold">{detail.name}</h2>
                <p className="text-sm text-[var(--text-dim)]">{detail.tracks.length} 曲</p>
              </div>
              <button onClick={load} disabled={!onLoadToQueue} className="flex items-center gap-1.5 rounded-full accent-bg px-4 py-2 text-sm font-medium text-white transition active:scale-95 disabled:opacity-40" title={onLoadToQueue ? '' : 'サーバーを選択してください'}>
                <Icons.Play className="h-4 w-4" /> キューへ
              </button>
              <button onClick={rename} className="grid h-9 w-9 place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)]" aria-label="名前変更" title="名前変更">
                <Icons.Search className="hidden" />
                <span className="text-sm">✎</span>
              </button>
              <button onClick={remove} className="grid h-9 w-9 place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)] hover:accent" aria-label="削除" title="削除">
                <Icons.Trash className="h-4 w-4" />
              </button>
            </div>
            <div className="min-h-0 flex-1 space-y-1 overflow-y-auto soft-scroll">
              {detail.tracks.length === 0 ? (
                <p className="px-1 py-4 text-sm text-[var(--text-faint)]">曲がありません。プレイヤーの「＋」から追加できます。</p>
              ) : (
                detail.tracks.map((t, i) => <PlaylistRow key={`${t.sourceUrl}-${i}`} track={t} onRemove={() => removeTrack(i)} />)
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function PlaylistRow({ track, onRemove }: { track: PlaylistTrackDTO; onRemove: () => void }) {
  return (
    <div className="group flex items-center gap-3 rounded-xl px-2 py-1.5 transition hover:bg-[var(--track-bg)]">
      {track.thumbnailUrl ? (
        <img src={track.thumbnailUrl} alt="" className="h-10 w-10 flex-none rounded-md object-cover" />
      ) : (
        <div className="grid h-10 w-10 flex-none place-items-center rounded-md bg-[var(--track-bg)]">
          <Icons.Headphones className="h-5 w-5 text-[var(--text-faint)]" />
        </div>
      )}
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{track.title}</div>
        <div className="truncate text-xs text-[var(--text-dim)]">{track.artist}</div>
      </div>
      <button onClick={onRemove} aria-label="削除" className="grid h-7 w-7 flex-none place-items-center rounded-full text-[var(--text-dim)] opacity-0 transition hover:accent group-hover:opacity-100">
        <Icons.Trash className="h-4 w-4" />
      </button>
    </div>
  );
}
