import { useCallback, useMemo, useState } from "react";

import { createDemoSnapshot, demoGuilds, demoPresets } from "./demo-data";
import type { DashboardModel, PlayerSnapshot, RepeatMode } from "./types";

const repeatSequence: readonly RepeatMode[] = ["off", "track", "queue"];

export function useDemoDashboard(): DashboardModel {
  const [selectedGuildId, setSelectedGuildId] = useState(demoGuilds[0]?.id ?? "0");
  const [snapshots, setSnapshots] = useState<Record<string, PlayerSnapshot>>(() => ({
    [selectedGuildId]: createDemoSnapshot(selectedGuildId)
  }));

  const snapshot = useMemo(
    () => snapshots[selectedGuildId] ?? createDemoSnapshot(selectedGuildId),
    [selectedGuildId, snapshots]
  );

  const update = useCallback(
    (mutate: (current: PlayerSnapshot) => PlayerSnapshot) => {
      setSnapshots((current) => {
        const existing = current[selectedGuildId] ?? createDemoSnapshot(selectedGuildId);
        return { ...current, [selectedGuildId]: mutate(existing) };
      });
    },
    [selectedGuildId]
  );

  const selectGuild = useCallback((guildId: string) => {
    setSelectedGuildId(guildId);
    setSnapshots((current) =>
      current[guildId] === undefined
        ? { ...current, [guildId]: createDemoSnapshot(guildId) }
        : current
    );
  }, []);

  return {
    guilds: demoGuilds,
    selectedGuildId,
    snapshot,
    presets: demoPresets,
    hrirCatalogStatus: "ready",
    connected: true,
    commandPending: false,
    selectGuild,
    togglePlayback: () =>
      update((current) => ({
        ...current,
        revision: current.revision + 1,
        state: current.state === "playing" ? "paused" : "playing"
      })),
    skip: () => update(bumpRevision),
    previous: () => update(bumpRevision),
    stop: () =>
      update((current) => ({
        ...current,
        revision: current.revision + 1,
        state: "idle_connected",
        track: null,
        queue: [],
        hasPreviousTrack: false
      })),
    removeQueued: (trackId) =>
      update((current) => ({
        ...current,
        revision: current.revision + 1,
        queue: current.queue.filter((track) => track.id !== trackId)
      })),
    moveQueued: (trackId, beforeTrackId) =>
      update((current) => moveQueued(current, trackId, beforeTrackId)),
    toggleShuffle: () =>
      update((current) => ({
        ...current,
        revision: current.revision + 1,
        shuffleEnabled: !current.shuffleEnabled
      })),
    cycleRepeat: () =>
      update((current) => {
        const index = repeatSequence.indexOf(current.repeatMode);
        const mode = repeatSequence[(index + 1) % repeatSequence.length] ?? "off";
        return { ...current, revision: current.revision + 1, repeatMode: mode };
      }),
    setVolume: (volumePercent) =>
      update((current) => ({
        ...current,
        revision: current.revision + 1,
        volumePercent
      })),
    setPreset: (hrirPresetId) =>
      update((current) => ({
        ...current,
        revision: current.revision + 1,
        hrirPresetId
      })),
    toggleSpatial: () =>
      update((current) => ({
        ...current,
        revision: current.revision + 1,
        spatialEnabled: !current.spatialEnabled
      })),
    seek: (positionMs) =>
      update((current) => ({
        ...current,
        revision: current.revision + 1,
        track:
          current.track === null
            ? null
            : {
                ...current.track,
                positionMsAtAnchor: positionMs,
                anchorUnixMs: Date.now()
              }
      }))
  };
}

function bumpRevision(snapshot: PlayerSnapshot): PlayerSnapshot {
  return { ...snapshot, revision: snapshot.revision + 1 };
}

function moveQueued(
  snapshot: PlayerSnapshot,
  trackId: string,
  beforeTrackId: string | null
): PlayerSnapshot {
  const sourceIndex = snapshot.queue.findIndex((track) => track.id === trackId);
  if (sourceIndex < 0 || beforeTrackId === trackId) return snapshot;

  const queue = [...snapshot.queue];
  const [moved] = queue.splice(sourceIndex, 1);
  if (moved === undefined) return snapshot;
  const destinationIndex = beforeTrackId === null
    ? queue.length
    : queue.findIndex((track) => track.id === beforeTrackId);
  if (destinationIndex < 0) return snapshot;
  queue.splice(destinationIndex, 0, moved);
  if (queue.every((track, index) => track.id === snapshot.queue[index]?.id)) return snapshot;
  return { ...snapshot, revision: snapshot.revision + 1, queue };
}
