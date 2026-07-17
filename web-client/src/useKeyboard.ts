import { useEffect, useRef } from 'react';
import type { GuildSession } from './useGuildSession.ts';

/**
 * Global playback keyboard shortcuts (Space = play/pause, ←/→ = prev/skip,
 * ↑/↓ = volume). Ignored while typing in a field or without control permission.
 * Reads the session through a ref so the listener binds once.
 */
export function useKeyboardShortcuts(session: GuildSession, enabled: boolean): void {
  const ref = useRef(session);
  ref.current = session;

  useEffect(() => {
    if (!enabled) return;
    const handler = (e: KeyboardEvent): void => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable)) return;

      const { snapshot, sendCommand } = ref.current;
      if (!snapshot?.viewer.canControl) return;

      switch (e.key) {
        case ' ':
          e.preventDefault();
          void sendCommand({ type: 'togglePlayPause' });
          break;
        case 'ArrowRight':
          void sendCommand({ type: 'skip' });
          break;
        case 'ArrowLeft':
          void sendCommand({ type: 'previous' });
          break;
        case 'ArrowUp':
          e.preventDefault();
          void sendCommand({ type: 'setVolume', percent: Math.min(100, snapshot.volume + 5) });
          break;
        case 'ArrowDown':
          e.preventDefault();
          void sendCommand({ type: 'setVolume', percent: Math.max(0, snapshot.volume - 5) });
          break;
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [enabled]);
}
