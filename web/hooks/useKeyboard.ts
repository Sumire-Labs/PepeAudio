// SPDX-License-Identifier: Apache-2.0
"use client";
import { useEffect, useRef } from "react";
import type { PlayerSession } from "./usePlayerHub";

// Session is read via a ref so the keydown listener binds once (deps: [enabled]).
export function useKeyboardShortcuts(session: PlayerSession, enabled: boolean): void {
  const ref = useRef(session);
  ref.current = session;

  useEffect(() => {
    if (!enabled) return;
    const handler = (e: KeyboardEvent): void => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable)) return;

      const { snapshot, cmd } = ref.current;
      if (!snapshot) return;

      switch (e.key) {
        case " ":
          e.preventDefault();
          cmd.playPause();
          break;
        case "ArrowRight":
          cmd.skip();
          break;
        case "ArrowLeft":
          cmd.previous();
          break;
        case "ArrowUp":
          e.preventDefault();
          cmd.setVolume(Math.min(200, snapshot.volume + 5));
          break;
        case "ArrowDown":
          e.preventDefault();
          cmd.setVolume(Math.max(0, snapshot.volume - 5));
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [enabled]);
}
