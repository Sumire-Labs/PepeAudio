// SPDX-License-Identifier: Apache-2.0
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";

export function useMe() {
  return useQuery({ queryKey: ["me"], queryFn: api.me, retry: false });
}

export function useGuilds() {
  // Poll so the sidebar's per-guild play status stays live.
  return useQuery({ queryKey: ["guilds"], queryFn: api.guilds, refetchInterval: 15_000 });
}
