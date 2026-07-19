import { useState, type FormEvent, type ReactNode } from 'react';
import { api, UnauthorizedError, type QueueItemDTO, type SearchCandidate, type WebCommand } from './api.ts';
import type { GuildSession } from './useGuildSession.ts';
import { cx, formatMs, Icons, Spinner } from './ui.tsx';
import { useToast } from './toast.tsx';

function looksLikeUrl(q: string): boolean {
  return /^https?:\/\//i.test(q) || /\b(youtube\.com|youtu\.be|spotify\.com|soundcloud\.com|music\.apple\.com)\b/i.test(q);
}

export function Queue({ session, onSaveTrack }: { session: GuildSession; onSaveTrack: (t: QueueItemDTO) => void }) {
  const { snapshot, sendCommand } = session;
  const toast = useToast();
  const [query, setQuery] = useState('');
  const [adding, setAdding] = useState(false);
  const [searching, setSearching] = useState(false);
  const [candidates, setCandidates] = useState<SearchCandidate[] | null>(null);
  const [draggedId, setDraggedId] = useState<string | null>(null);

  const canControl = snapshot?.viewer.canControl ?? false;
  const queue = snapshot?.queue ?? [];
  const history = snapshot?.history ?? [];

  const run = async (command: WebCommand): Promise<boolean> => {
    const result = await sendCommand(command);
    if (!result.ok && result.error) toast(result.error, 'error');
    return result.ok;
  };

  const addDirect = async (q: string) => {
    setAdding(true);
    const ok = await run({ type: 'addTrack', query: q });
    setAdding(false);
    if (ok) {
      setQuery('');
      setCandidates(null);
      toast('キューに追加しました。');
    }
  };

  const doSearch = async (q: string) => {
    setSearching(true);
    setCandidates(null);
    try {
      const { candidates } = await api.search(q);
      setCandidates(candidates);
      if (candidates.length === 0) toast('検索結果が見つかりませんでした。', 'error');
    } catch (err) {
      if (err instanceof UnauthorizedError) window.location.reload();
      else toast(err instanceof Error ? err.message : '検索に失敗しました。', 'error');
    } finally {
      setSearching(false);
    }
  };

  const submitAdd = (e: FormEvent) => {
    e.preventDefault();
    const q = query.trim();
    if (!q || adding || searching) return;
    if (looksLikeUrl(q)) void addDirect(q);
    else void doSearch(q);
  };

  const onDrop = (targetIndex: number) => {
    if (!draggedId) return;
    const from = queue.findIndex((t) => t.id === draggedId);
    setDraggedId(null);
    if (from === -1 || from === targetIndex) return;
    void run({ type: 'moveQueueItem', id: draggedId, toIndex: targetIndex });
  };

  return (
    <div className="flex h-full flex-col">
      <form onSubmit={submitAdd} className="glass mb-2 flex items-center gap-2 rounded-2xl px-3.5 py-2.5">
        <Icons.Search className="h-5 w-5 flex-none text-[var(--text-dim)]" />
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="URL・プレイリストURL・検索語を入力…"
          className="w-full bg-transparent text-sm outline-none placeholder:text-[var(--text-faint)]"
        />
        {query ? (
          <button type="button" aria-label="クリア" onClick={() => { setQuery(''); setCandidates(null); }} className="grid h-6 w-6 flex-none place-items-center rounded-full text-[var(--text-faint)] hover:text-[var(--text)]">
            <Icons.Close className="h-3.5 w-3.5" />
          </button>
        ) : null}
        <button
          type="submit"
          disabled={adding || searching || !query.trim()}
          className="grid h-8 w-8 flex-none place-items-center rounded-full accent-bg text-white transition active:scale-90 disabled:opacity-40"
          aria-label={query.trim() && !looksLikeUrl(query.trim()) ? '検索' : '追加'}
        >
          {adding || searching ? <Spinner className="h-4 w-4" /> : looksLikeUrl(query.trim()) ? <Icons.Plus className="h-4 w-4" /> : <Icons.Search className="h-4 w-4" />}
        </button>
      </form>

      {candidates ? (
        <div className="glass mb-2 max-h-72 overflow-y-auto soft-scroll rounded-2xl p-1.5 fade-in">
          <div className="flex items-center justify-between px-2 py-1">
            <span className="text-xs font-medium text-[var(--text-dim)]">検索結果</span>
            <button onClick={() => setCandidates(null)} className="text-xs text-[var(--text-faint)] hover:accent">閉じる</button>
          </div>
          {candidates.map((c) => (
            <button
              key={c.url}
              onClick={() => void addDirect(c.url)}
              disabled={adding}
              className="flex w-full items-center gap-3 rounded-xl px-2 py-1.5 text-left transition hover:bg-[var(--track-bg)] disabled:opacity-50"
            >
              <img src={c.thumbnailUrl} alt="" className="h-9 w-12 flex-none rounded-md object-cover" />
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium">{c.title}</div>
                <div className="truncate text-xs text-[var(--text-dim)]">{c.author}</div>
              </div>
              <Icons.Plus className="h-4 w-4 flex-none text-[var(--text-dim)]" />
            </button>
          ))}
        </div>
      ) : null}

      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto soft-scroll pr-1">
        <section>
          <div className="mb-2 flex items-center justify-between px-1">
            <h3 className="flex items-center gap-2 text-sm font-semibold text-[var(--text-dim)]">
              <Icons.Queue className="h-4 w-4" /> 次の曲 · {queue.length}
            </h3>
            {queue.length > 0 && canControl ? (
              <button onClick={() => run({ type: 'clearQueue' })} className="text-xs text-[var(--text-faint)] transition hover:accent">
                すべてクリア
              </button>
            ) : null}
          </div>
          {queue.length === 0 ? (
            <p className="px-1 py-4 text-sm text-[var(--text-faint)]">キューは空です。上の入力欄から曲を追加できます。</p>
          ) : (
            <ul className="space-y-1">
              {queue.map((track, i) => (
                <TrackRow
                  key={track.id}
                  track={track}
                  draggable={canControl}
                  dragging={draggedId === track.id}
                  onDragStart={() => setDraggedId(track.id)}
                  onDragEnd={() => setDraggedId(null)}
                  onDropHere={() => onDrop(i)}
                  onJump={canControl ? () => run({ type: 'jumpTo', id: track.id }) : undefined}
                  onRemove={canControl ? () => run({ type: 'removeQueueItem', id: track.id }) : undefined}
                  onSave={() => onSaveTrack(track)}
                />
              ))}
            </ul>
          )}
        </section>

        {history.length > 0 ? (
          <section>
            <h3 className="mb-2 flex items-center gap-2 px-1 text-sm font-semibold text-[var(--text-dim)]">
              <Icons.Clock className="h-4 w-4" /> 履歴
            </h3>
            <ul className="space-y-1 opacity-70">
              {[...history].reverse().map((track) => (
                <TrackRow
                  key={`${track.id}-h`}
                  track={track}
                  onRequeue={canControl ? () => run({ type: 'addTrack', query: track.sourceUrl }) : undefined}
                  onSave={() => onSaveTrack(track)}
                />
              ))}
            </ul>
          </section>
        ) : null}
      </div>
    </div>
  );
}

function TrackRow({
  track,
  draggable,
  dragging,
  onDragStart,
  onDragEnd,
  onDropHere,
  onJump,
  onRemove,
  onRequeue,
  onSave,
}: {
  track: QueueItemDTO;
  draggable?: boolean;
  dragging?: boolean;
  onDragStart?: () => void;
  onDragEnd?: () => void;
  onDropHere?: () => void;
  onJump?: () => void;
  onRemove?: () => void;
  onRequeue?: () => void;
  onSave?: () => void;
}) {
  return (
    <li
      draggable={draggable}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onDragOver={onDropHere ? (e) => e.preventDefault() : undefined}
      onDrop={onDropHere}
      onClick={onJump}
      title={onJump ? 'クリックでこの曲へ' : undefined}
      className={cx(
        'group flex items-center gap-3 rounded-xl px-2 py-1.5 transition hover:bg-[var(--track-bg)]',
        dragging ? 'opacity-40' : '',
        draggable ? 'cursor-grab active:cursor-grabbing' : onJump ? 'cursor-pointer' : '',
      )}
    >
      {draggable ? <Icons.Grip className="h-4 w-4 flex-none text-[var(--text-faint)] opacity-0 transition group-hover:opacity-100" /> : null}
      <div className="relative h-10 w-10 flex-none">
        {track.thumbnailUrl ? (
          <img src={track.thumbnailUrl} alt="" className="h-10 w-10 rounded-md object-cover" />
        ) : (
          <div className="grid h-10 w-10 place-items-center rounded-md bg-[var(--track-bg)]">
            <Icons.Headphones className="h-5 w-5 text-[var(--text-faint)]" />
          </div>
        )}
        {onJump ? (
          <div className="absolute inset-0 grid place-items-center rounded-md bg-black/45 opacity-0 transition group-hover:opacity-100">
            <Icons.Play className="h-4 w-4 text-white" />
          </div>
        ) : null}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{track.title}</div>
        <div className="truncate text-xs text-[var(--text-dim)]">{track.artist}</div>
      </div>
      {track.requesterAvatarUrl ? (
        <img src={track.requesterAvatarUrl} alt="" title={track.requesterName ?? undefined} className="h-4 w-4 flex-none rounded-full opacity-70" />
      ) : null}
      <span className="flex-none text-xs tabular-nums text-[var(--text-faint)]">{formatMs(track.durationMs)}</span>
      <div className="flex flex-none items-center gap-0.5 opacity-0 transition group-hover:opacity-100">
        {onSave ? <RowBtn icon={Icons.Playlist} label="プレイリストに保存" onClick={onSave} /> : null}
        {onRequeue ? <RowBtn icon={Icons.Plus} label="もう一度キューへ" onClick={onRequeue} /> : null}
        {onRemove ? <RowBtn icon={Icons.Trash} label="削除" onClick={onRemove} /> : null}
      </div>
    </li>
  );
}

function RowBtn({ icon: Icon, label, onClick }: { icon: (p: { className?: string }) => ReactNode; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={(e) => {
        e.stopPropagation(); // don't trigger the row's jump-to-track click
        onClick();
      }}
      className="grid h-7 w-7 place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--hairline-strong)] hover:text-[var(--text)]"
    >
      <Icon className="h-4 w-4" />
    </button>
  );
}
