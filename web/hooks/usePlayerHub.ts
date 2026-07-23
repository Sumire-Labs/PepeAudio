// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef } from "react";
import type { HubConnection } from "@microsoft/signalr";
import { createHub } from "@/lib/signalr";
import { usePlayerStore } from "@/stores/playerStore";
import type { Control, PlayerState } from "@/lib/types";

// Connects to the player hub, subscribes to a guild, and streams PlayerState.
export function usePlayerHub(guildId: string | null) {
  const setState = usePlayerStore((s) => s.setState);
  const reset = usePlayerStore((s) => s.reset);
  const hubRef = useRef<HubConnection | null>(null);

  useEffect(() => {
    if (!guildId) return;
    reset();
    const hub = createHub();
    hubRef.current = hub;
    hub.on("PlayerState", (s: PlayerState) => setState(s));
    hub.start()
      .then(() => hub.invoke("Subscribe", guildId))
      .catch((e) => console.error("hub error", e));
    return () => {
      hub.stop().catch(() => {});
      hubRef.current = null;
    };
  }, [guildId, setState, reset]);

  const control = useCallback(
    (action: Control) => hubRef.current?.invoke("Control", guildId, action).catch(console.error),
    [guildId],
  );
  const play = useCallback(
    (url: string) => hubRef.current?.invoke("Play", guildId, url).catch(console.error),
    [guildId],
  );
  const reorder = useCallback(
    (from: number, to: number) => hubRef.current?.invoke("ReorderQueue", guildId, from, to).catch(console.error),
    [guildId],
  );
  const remove = useCallback(
    (index: number) => hubRef.current?.invoke("RemoveTrack", guildId, index).catch(console.error),
    [guildId],
  );

  return { control, play, reorder, remove };
}
