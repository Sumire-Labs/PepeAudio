import { useEffect, useRef, useState, type ReactNode } from 'react';
import type { GuildSnapshot, QueueItemDTO, WebCommand } from './api.ts';
import type { GuildSession } from './useGuildSession.ts';
import { useTicker } from './useGuildSession.ts';
import { cx, EqualizerBars, formatMs, Icons, IconButton } from './ui.tsx';
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
  const fraction = duration && duration > 0 ? Math.min(1, elapsed / duration) : 0;

  return (
    <div className="flex h-full flex-col items-center justify-center px-6 py-8 fade-in">
      {snapshot && !canControl && snapshot.viewer.denyReason ? (
        <div className="glass mb-5 max-w-md rounded-2xl px-4 py-2.5 text-center text-sm text-[var(--text-dim)]">
          {snapshot.viewer.denyReason}
        </div>
      ) : null}

      <Artwork track={current} playing={playing} />

      <div className="mt-7 w-full max-w-md text-center">
        <h2 className="truncate text-2xl font-semibold tracking-tight">{current?.title ?? '再生していません'}</h2>
        <p className="mt-1 truncate text-[var(--text-dim)]">{current?.artist ?? 'キューに曲を追加して再生を始めましょう'}</p>
      </div>

      {/* progress (display only — the bot has no seek) */}
      <div className="mt-6 w-full max-w-md">
        <div className="h-1.5 w-full overflow-hidden rounded-full" style={{ background: 'var(--track-bg)' }}>
          <div className="h-full rounded-full accent-bg transition-[width] duration-200 ease-linear" style={{ width: `${fraction * 100}%` }} />
        </div>
        <div className="mt-1.5 flex justify-between text-xs tabular-nums text-[var(--text-dim)]">
          <span>{formatMs(current ? elapsed : 0)}</span>
          <span>{duration ? formatMs(duration) : current ? 'LIVE' : '--:--'}</span>
        </div>
      </div>

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

      {/* secondary: volume + radio + 24/7 + save */}
      <div className="mt-6 flex w-full max-w-md items-center gap-3">
        <VolumeControl value={snapshot?.volume ?? 70} disabled={!canControl} onChange={(percent) => run({ type: 'setVolume', percent })} />
        <IconButton icon={Icons.Radio} label="オートプレイ (ラジオ)" active={snapshot?.autoplay} disabled={!canControl} onClick={() => run({ type: 'setAutoplay', enabled: !snapshot?.autoplay })} />
        <IconButton icon={Icons.Pin} label="24時間モード" active={snapshot?.stay247} disabled={!canControl} onClick={() => run({ type: 'setStay247', enabled: !snapshot?.stay247 })} />
        <IconButton icon={Icons.Plus} label="プレイリストに保存" disabled={!current} onClick={() => current && onSaveTrack(current)} />
        <IconButton icon={Icons.Stop} label="停止" disabled={!canControl || !snapshot?.current} onClick={() => run({ type: 'stop' })} />
      </div>

      {snapshot?.auraEnabled ? <AuraControls snapshot={snapshot} disabled={!canControl} run={run} /> : null}
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
  // Keep in sync with server pushes unless the user is mid-drag.
  const dragging = useRef(false);
  useEffect(() => {
    if (!dragging.current) setLocal(value);
  }, [value]);

  const commit = (v: number) => {
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => onChange(v), 250);
  };

  const Ico = local === 0 ? Icons.VolumeMute : Icons.Volume;
  return (
    <div className="flex flex-1 items-center gap-2.5">
      <Ico className="h-5 w-5 flex-none text-[var(--text-dim)]" />
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

function AuraControls({ snapshot, disabled, run }: { snapshot: GuildSnapshot; disabled: boolean; run: (c: WebCommand) => void }) {
  return (
    <div className="mt-6 flex w-full max-w-md flex-wrap items-center justify-center gap-2">
      <Chip icon={Icons.Spatial} label="360° Sound" active={snapshot.aura360Mode === 'on'} disabled={disabled} onClick={() => run({ type: 'setAura360', mode: snapshot.aura360Mode === 'on' ? 'off' : 'on' })} />
      <Chip icon={Icons.Headphones} label="Aura HRIR" active={snapshot.hrirMode === 'on'} disabled={disabled} onClick={() => run({ type: 'setHrir', mode: snapshot.hrirMode === 'on' ? 'off' : 'on' })} />
      {snapshot.hrirMode === 'on' && snapshot.auraPresets.length > 0 ? (
        <select
          value={snapshot.hrirProfile ?? ''}
          disabled={disabled}
          onChange={(e) => run({ type: 'setAuraPreset', id: e.target.value })}
          className="glass rounded-full px-3 py-2 text-sm text-[var(--text)] outline-none disabled:opacity-40"
        >
          {snapshot.auraPresets.map((p) => (
            <option key={p.id} value={p.id} className="text-black">
              {p.label}
            </option>
          ))}
        </select>
      ) : null}
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
