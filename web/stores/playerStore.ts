// SPDX-License-Identifier: Apache-2.0
import { create } from "zustand";
import type { PlayerState } from "@/lib/types";

interface PlayerStore {
  state: PlayerState | null;
  setState: (s: PlayerState) => void;
  reset: () => void;
}

// Live player state, fed by SignalR pushes.
export const usePlayerStore = create<PlayerStore>((set) => ({
  state: null,
  setState: (s) => set({ state: s }),
  reset: () => set({ state: null }),
}));
