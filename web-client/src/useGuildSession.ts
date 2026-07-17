import { useCallback, useEffect, useRef, useState } from 'react';
import { api, subscribeToGuild, UnauthorizedError } from './api.ts';
import type { CommandResult, GuildSnapshot, ViewerCapabilities, WebCommand } from './api.ts';

/** The shard IPC push uses this exact placeholder (see server DISPLAY_ONLY_VIEWER). */
function isDisplayOnlyViewer(v: ViewerCapabilities): boolean {
  return !v.canControl && v.denyReason === null && !v.inBotVoiceChannel;
}

export interface GuildSession {
  snapshot: GuildSnapshot | null;
  /** Client Date.now() when the current snapshot arrived, for progress extrapolation. */
  receivedAt: number;
  loading: boolean;
  sendCommand: (command: WebCommand) => Promise<CommandResult>;
  refresh: () => void;
}

/**
 * Subscribes to one guild's live state over SSE, keeps the last accurate
 * per-viewer capabilities across display-only pushes, and exposes a command
 * sender that folds the command's returned snapshot back into state.
 */
export function useGuildSession(guildId: string | null, onUnauthorized: () => void): GuildSession {
  const [snapshot, setSnapshot] = useState<GuildSnapshot | null>(null);
  const [receivedAt, setReceivedAt] = useState(0);
  const [loading, setLoading] = useState(true);
  const viewerRef = useRef<ViewerCapabilities | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  const apply = useCallback((snap: GuildSnapshot | null) => {
    if (snap) {
      // Display-only pushes (sharded mode) omit real caps — keep the last known.
      if (isDisplayOnlyViewer(snap.viewer) && viewerRef.current) {
        snap = { ...snap, viewer: viewerRef.current };
      } else {
        viewerRef.current = snap.viewer;
      }
    }
    setSnapshot(snap);
    setReceivedAt(Date.now());
  }, []);

  useEffect(() => {
    if (!guildId) {
      setSnapshot(null);
      setLoading(false);
      return;
    }
    let active = true;
    setLoading(true);
    viewerRef.current = null;

    api
      .getSnapshot(guildId)
      .then(({ snapshot: snap }) => {
        if (!active) return;
        apply(snap);
        setLoading(false);
      })
      .catch((err) => {
        if (err instanceof UnauthorizedError) onUnauthorized();
        if (active) setLoading(false);
      });

    const close = subscribeToGuild(guildId, (snap) => {
      if (active) apply(snap);
    });

    // Refresh accurate per-viewer caps when the tab regains focus (the user may
    // have joined/left the voice channel while away).
    const onFocus = () => {
      api
        .getSnapshot(guildId)
        .then(({ snapshot: snap }) => {
          if (active) apply(snap);
        })
        .catch(() => {});
    };
    window.addEventListener('focus', onFocus);

    return () => {
      active = false;
      close();
      window.removeEventListener('focus', onFocus);
    };
  }, [guildId, refreshKey, apply, onUnauthorized]);

  const sendCommand = useCallback(
    async (command: WebCommand): Promise<CommandResult> => {
      if (!guildId) return { ok: false, error: 'no guild' };
      try {
        const result = await api.sendCommand(guildId, command);
        if (result.snapshot !== undefined) apply(result.snapshot);
        return result;
      } catch (err) {
        if (err instanceof UnauthorizedError) {
          onUnauthorized();
          return { ok: false, error: 'セッションが切れました。' };
        }
        return { ok: false, error: err instanceof Error ? err.message : '操作に失敗しました。' };
      }
    },
    [guildId, apply, onUnauthorized],
  );

  const refresh = useCallback(() => setRefreshKey((k) => k + 1), []);

  return { snapshot, receivedAt, loading, sendCommand, refresh };
}

/** Returns a value that changes ~4×/sec, to drive smooth local progress-bar updates. */
export function useTicker(active: boolean): number {
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!active) return;
    const id = setInterval(() => setTick((t) => t + 1), 250);
    return () => clearInterval(id);
  }, [active]);
  return Date.now();
}
