import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from 'react';
import type { QueueItemDTO, WebCommand } from './api.ts';
import type { GuildSession } from './useGuildSession.ts';
import { useTicker } from './useGuildSession.ts';
import { cx, Dropdown, EqualizerBars, formatMs, Icons, IconButton, Menu } from './ui.tsx';
import { useToast } from './toast.tsx';

export function Player({
  session,
  onSaveTrack,
}: {
  session: GuildSession;
  onSaveTrack: (track: QueueItemDTO) => void;
}) {
  const { snapshot, receivedAt, sendCommand } = session;
  const toast = useToast();

  const run = async (command: WebCommand) => {
    const result = await sendCommand(command);
    if (!result.ok && result.error) toast(result.error, 'error');
  };

  const current = snapshot?.current ?? null;
  const canControl = snapshot?.viewer.canControl ?? false;
  const playing = snapshot?.status === 'playing';
  const now = useTicker(playing);
  const elapsed = current
    ? Math.min(current.durationMs ?? Number.MAX_SAFE_INTEGER, (snapshot?.elapsedMs ?? 0) + (playing ? now - receivedAt : 0))
    : 0;
  const duration = current?.durationMs ?? null;

  return (
    <div className="relative flex h-full flex-col items-center justify-center px-6 py-8 fade-in">
      {/* top-right: save + overflow (stop). Keeps the main control area uncluttered. */}
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
              label: '停止して退出',
              icon: Icons.Stop,
              danger: true,
              disabled: !canControl || !snapshot?.current,
              onClick: () => run({ type: 'stop' }),
            },
          ]}
        />
      </div>

      {snapshot && !canControl && snapshot.viewer.denyReason ? (
        <div className="glass mb-5 max-w-md rounded-2xl px-4 py-2.5 text-center text-sm text-[var(--text-dim)]">
          {snapshot.viewer.denyReason}
        </div>
      ) : null}

      <Artwork track={current} playing={playing} />

      <div className="mt-7 w-full max-w-md text-center">
        <h2 className="truncate text-2xl font-semibold tracking-tight">{current?.title ?? '再生していません'}</h2>
        <p className="mt-1 truncate text-[var(--text-dim)]">{current?.artist ?? 'キューに曲を追加して再生を始めましょう'}</p>
        {current?.requesterName ? (
          <div className="mt-2 inline-flex items-center gap-1.5 text-xs text-[var(--text-faint)]">
            {current.requesterAvatarUrl ? <img src={current.requesterAvatarUrl} alt="" className="h-4 w-4 rounded-full" /> : null}
            <span>{current.requesterName} がリクエスト</span>
          </div>
        ) : null}
      </div>

      <SeekBar
        elapsed={current ? elapsed : 0}
        duration={duration}
        canSeek={canControl && !!current}
        onSeek={(ms) => run({ type: 'seek', positionMs: ms })}
      />

      {/* transport */}
      <div className="mt-6 flex items-center gap-4">
        <IconButton icon={Icons.Shuffle} label="シャッフル" size="md" active={snapshot?.shuffleEnabled} disabled={!canControl} onClick={() => run({ type: 'toggleShuffle' })} />
        <IconButton icon={Icons.Prev} label="前へ" size="md" disabled={!canControl} onClick={() => run({ type: 'previous' })} />
        <button
          type="button"
          aria-label={playing ? '一時停止' : '再生'}
          disabled={!canControl || !current}
          onClick={() => run({ type: 'togglePlayPause' })}
          className={cx(
            'grid h-16 w-16 place-items-center rounded-full accent-bg text-white shadow-xl transition-transform duration-150 active:scale-90',
            !canControl || !current ? 'opacity-40' : 'hover:brightness-110',
          )}
          style={{ boxShadow: '0 10px 30px color-mix(in srgb, var(--accent) 45%, transparent)' }}
        >
          {playing ? <Icons.Pause className="h-7 w-7" /> : <Icons.Play className="ml-0.5 h-7 w-7" />}
        </button>
        <IconButton icon={Icons.Next} label="スキップ" size="md" disabled={!canControl} onClick={() => run({ type: 'skip' })} />
        <LoopButton mode={snapshot?.loopMode ?? 'off'} disabled={!canControl} onCycle={(mode) => run({ type: 'setLoopMode', mode })} />
      </div>

      {/* volume */}
      <div className="mt-6 w-full max-w-md">
        <VolumeControl value={snapshot?.volume ?? 70} disabled={!canControl} onChange={(percent) => run({ type: 'setVolume', percent })} />
      </div>

      {/* playback modes — centered row */}
      <div className="mt-6 flex w-full max-w-md flex-wrap items-center justify-center gap-2">
        <Chip icon={Icons.Radio} label="オートプレイ" active={Boolean(snapshot?.autoplay)} disabled={!canControl} onClick={() => run({ type: 'setAutoplay', enabled: !snapshot?.autoplay })} />
        <Chip icon={Icons.Pin} label="24時間" active={Boolean(snapshot?.stay247)} disabled={!canControl} onClick={() => run({ type: 'setStay247', enabled: !snapshot?.stay247 })} />
      </div>

      {/* sound (Aura) — separate centered row */}
      {snapshot?.auraEnabled ? (
        <div className="mt-3 flex w-full max-w-md flex-wrap items-center justify-center gap-2">
          <Chip icon={Icons.Spatial} label="360°" active={snapshot.aura360Mode === 'on'} disabled={!canControl} onClick={() => run({ type: 'setAura360', mode: snapshot.aura360Mode === 'on' ? 'off' : 'on' })} />
          <Chip icon={Icons.Headphones} label="Aura" active={snapshot.hrirMode === 'on'} disabled={!canControl} onClick={() => run({ type: 'setHrir', mode: snapshot.hrirMode === 'on' ? 'off' : 'on' })} />
          {snapshot.hrirMode === 'on' && snapshot.auraPresets.length > 0 ? (
            <Dropdown
              value={snapshot.hrirProfile ?? ''}
              options={snapshot.auraPresets.map((p) => ({ value: p.id, label: p.label }))}
              disabled={!canControl}
              icon={Icons.Headphones}
              onChange={(id) => run({ type: 'setAuraPreset', id })}
            />
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

function SeekBar({ elapsed, duration, canSeek, onSeek }: { elapsed: number; duration: number | null; canSeek: boolean; onSeek: (ms: number) => void }) {
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
        className={cx('group relative flex h-4 items-center touch-none', seekable ? 'cursor-pointer' : '')}
      >
        <div className="h-1.5 w-full overflow-hidden rounded-full" style={{ background: 'var(--track-bg)' }}>
          <div className={cx('h-full rounded-full accent-bg', scrub === null ? 'transition-[width] duration-200 ease-linear' : '')} style={{ width: `${fraction * 100}%` }} />
        </div>
        {seekable ? (
          <div className="pointer-events-none absolute h-3 w-3 -translate-x-1/2 rounded-full bg-white opacity-0 shadow transition group-hover:opacity-100" style={{ left: `${fraction * 100}%` }} />
        ) : null}
      </div>
      <div className="mt-1.5 flex justify-between text-xs tabular-nums text-[var(--text-dim)]">
        <span>{formatMs(value)}</span>
        <span>{duration ? formatMs(duration) : elapsed ? 'LIVE' : '--:--'}</span>
      </div>
    </div>
  );
}

function Artwork({ track, playing }: { track: QueueItemDTO | null; playing: boolean }) {
  return (
    <div
      className="relative aspect-square w-full max-w-[min(58vh,22rem)] overflow-hidden rounded-[28px]"
      style={{ boxShadow: '0 30px 70px var(--shadow)' }}
    >
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

function LoopButton({ mode, disabled, onCycle }: { mode: 'off' | 'track' | 'queue'; disabled: boolean; onCycle: (m: 'off' | 'track' | 'queue') => void }) {
  const next = mode === 'off' ? 'track' : mode === 'track' ? 'queue' : 'off';
  const Icon = mode === 'track' ? Icons.RepeatOne : Icons.Repeat;
  return <IconButton icon={Icon} label={`リピート: ${mode}`} active={mode !== 'off'} disabled={disabled} onClick={() => onCycle(next)} />;
}

function VolumeControl({ value, disabled, onChange }: { value: number; disabled: boolean; onChange: (v: number) => void }) {
  const [local, setLocal] = useState(value);
  const timer = useRef<number | null>(null);
  const prevVolume = useRef(value || 70);
  // Keep in sync with server pushes unless the user is mid-drag.
  const dragging = useRef(false);
  useEffect(() => {
    if (!dragging.current) setLocal(value);
  }, [value]);

  const commit = (v: number) => {
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => onChange(v), 250);
  };

  const toggleMute = () => {
    if (disabled) return;
    if (local > 0) {
      prevVolume.current = local;
      setLocal(0);
      onChange(0);
    } else {
      const restore = prevVolume.current || 50;
      setLocal(restore);
      onChange(restore);
    }
  };

  const Ico = local === 0 ? Icons.VolumeMute : Icons.Volume;
  return (
    <div className="flex flex-1 items-center gap-2.5">
      <button type="button" onClick={toggleMute} disabled={disabled} aria-label={local === 0 ? 'ミュート解除' : 'ミュート'} title={local === 0 ? 'ミュート解除' : 'ミュート'} className="grid h-8 w-8 flex-none place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--track-bg)] disabled:opacity-40">
        <Ico className="h-5 w-5" />
      </button>
      <input
        type="range"
        min={0}
        max={100}
        step={5}
        value={local}
        disabled={disabled}
        onPointerDown={() => (dragging.current = true)}
        onPointerUp={() => (dragging.current = false)}
        onChange={(e) => {
          const v = Number(e.target.value);
          setLocal(v);
          commit(v);
        }}
        className="h-1.5 w-full cursor-pointer appearance-none rounded-full disabled:opacity-40"
        style={{
          accentColor: 'var(--accent)',
          background: `linear-gradient(to right, var(--accent) ${local}%, var(--track-bg) ${local}%)`,
          borderRadius: 999,
        }}
      />
      <span className="w-8 flex-none text-right text-xs tabular-nums text-[var(--text-dim)]">{local}</span>
    </div>
  );
}

function Chip({ icon: Icon, label, active, disabled, onClick }: { icon: (p: { className?: string }) => ReactNode; label: string; active: boolean; disabled: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cx(
        'flex items-center gap-2 rounded-full px-3.5 py-2 text-sm font-medium transition-all duration-200 active:scale-95 disabled:opacity-40',
        active ? 'accent-bg text-white' : 'glass text-[var(--text-dim)]',
      )}
    >
      <Icon className="h-4 w-4" />
      {label}
    </button>
  );
}
