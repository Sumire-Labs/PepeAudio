// SPDX-License-Identifier: Apache-2.0
"use client";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { HubConnection } from "@microsoft/signalr";
import { createHub } from "@/lib/signalr";
import type { LoopMode, PlayerSnapshot } from "@/lib/types";

export interface PlayerCommands {
  playPause(): void;
  skip(): void;
  previous(): void;
  stop(): void;
  toggleShuffle(): void;
  toggleAura(): void;
  clearQueue(): void;
  setLoop(mode: LoopMode): void;
  setPreset(name: string): void;
  setVolume(percent: number): void;
  seek(positionMs: number): void;
  moveTrack(id: string, toIndex: number): void;
  removeTrack(id: string): void;
  jumpTo(id: string): void;
  setAutoplay(enabled: boolean): void;
  play(query: string): Promise<void>;
}

export interface PlayerSession {
  snapshot: PlayerSnapshot | null;
  /** Date.now() when the current snapshot arrived — for smooth progress extrapolation. */
  receivedAt: number;
  connected: boolean;
  cmd: PlayerCommands;
}

// Connects to the player hub, subscribes to a guild, streams PlayerSnapshot, and exposes
// the strongly-typed command surface (each method maps to a hub invocation).
export function usePlayerHub(guildId: string | null): PlayerSession {
  const [snapshot, setSnapshot] = useState<PlayerSnapshot | null>(null);
  const [receivedAt, setReceivedAt] = useState(0);
  const [connected, setConnected] = useState(false);
  const hubRef = useRef<HubConnection | null>(null);

  useEffect(() => {
    setSnapshot(null);
    setConnected(false);
    if (!guildId) return;

    const hub = createHub();
    hubRef.current = hub;
    let active = true;

    hub.on("PlayerState", (s: PlayerSnapshot) => {
      if (!active) return;
      setSnapshot(s);
      setReceivedAt(Date.now());
    });
    hub.onreconnecting(() => active && setConnected(false));
    hub.onreconnected(() => {
      if (!active) return;
      setConnected(true);
      hub.invoke("Subscribe", guildId).catch(() => {});
    });
    hub.onclose(() => active && setConnected(false));

    hub
      .start()
      .then(() => hub.invoke("Subscribe", guildId))
      .then(() => active && setConnected(true))
      .catch((e) => console.error("hub error", e));

    return () => {
      active = false;
      hubRef.current = null;
      hub.stop().catch(() => {});
    };
  }, [guildId]);

  const invoke = useCallback(
    (method: string, ...args: unknown[]) => {
      hubRef.current?.invoke(method, guildId, ...args).catch((e) => console.error(method, e));
    },
    [guildId],
  );

  const cmd: PlayerCommands = useMemo(() => ({
    playPause: () => invoke("Control", "PlayPause"),
    skip: () => invoke("Control", "Skip"),
    previous: () => invoke("Control", "Previous"),
    stop: () => invoke("Control", "Stop"),
    toggleShuffle: () => invoke("Control", "Shuffle"),
    toggleAura: () => invoke("Control", "ToggleAura"),
    clearQueue: () => invoke("Control", "ClearQueue"),
    setLoop: (mode) => invoke("SetLoop", mode),
    setPreset: (name) => invoke("SetPreset", name),
    setVolume: (percent) => invoke("SetVolume", Math.round(percent)),
    seek: (positionMs) => invoke("Seek", Math.round(positionMs)),
    moveTrack: (id, toIndex) => invoke("MoveTrack", id, toIndex),
    removeTrack: (id) => invoke("RemoveTrack", id),
    jumpTo: (id) => invoke("JumpTo", id),
    setAutoplay: (enabled) => invoke("SetAutoplay", enabled),
    play: (query) => hubRef.current?.invoke("Play", guildId, query) ?? Promise.resolve(),
  }), [invoke, guildId]);

  return { snapshot, receivedAt, connected, cmd };
}
