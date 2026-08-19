import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiResponseError, type AuthBootstrap } from "./api-client";
import type { HrirPreset } from "./types";
import { useLiveDashboardWithDependencies } from "./use-live-dashboard";

const SESSION: AuthBootstrap = {
  csrfToken: "A".repeat(43),
  account: {
    source: "discord",
    userId: "111",
    username: "pepe-listener",
    displayName: "Pepe Listener",
    avatarUrl: null
  },
  guilds: [{
    id: "123",
    name: "Listening Room",
    icon: null,
    owner: true,
    permissions: "8",
    botPresent: true
  }],
  logoutAvailable: true
};

afterEach(() => vi.restoreAllMocks());

describe("useLiveDashboard session boundaries", () => {
  it("treats an authenticated account with no guilds as a ready empty workspace", async () => {
    const maintainEvents = vi.fn(async () => undefined);
    const dependencies = {
      fetchBootstrap: vi.fn(async () => ({ ...SESSION, guilds: [] })),
      fetchPresets: vi.fn(async (): Promise<readonly HrirPreset[]> => []),
      maintainEvents,
      logout: vi.fn(async () => undefined)
    };
    const { result } = renderHook(() =>
      useLiveDashboardWithDependencies(true, dependencies)
    );

    await waitFor(() => expect(result.current.status).toBe("ready"));

    expect(result.current.message).toBeNull();
    expect(result.current.model.guilds).toHaveLength(0);
    expect(result.current.model.selectedGuildId).toBe("");
    expect(result.current.account?.displayName).toBe("Pepe Listener");
    expect(maintainEvents).not.toHaveBeenCalled();
  });

  it("expires the whole dashboard session when the HRIR catalog returns 401", async () => {
    const dependencies = {
      fetchBootstrap: vi.fn(async () => SESSION),
      fetchPresets: vi.fn(async () => {
        throw new ApiResponseError(401, "Discordでログインし直してください。");
      }),
      maintainEvents: vi.fn(waitUntilAborted),
      logout: vi.fn(async () => undefined)
    };
    const { result } = renderHook(() =>
      useLiveDashboardWithDependencies(true, dependencies)
    );

    await waitFor(() => expect(result.current.status).toBe("unauthenticated"));

    expect(result.current.model.guilds).toHaveLength(0);
    expect(result.current.model.selectedGuildId).toBe("");
    expect(result.current.account).toBeNull();
    expect(result.current.message).toBe("Discordでログインし直してください。");
  });

  it("keeps the logout callback stable across unrelated dashboard renders", async () => {
    const dependencies = {
      fetchBootstrap: vi.fn(async () => SESSION),
      fetchPresets: vi.fn(async (): Promise<readonly HrirPreset[]> => []),
      maintainEvents: vi.fn(waitUntilAborted),
      logout: vi.fn(async () => undefined)
    };
    const { result } = renderHook(() =>
      useLiveDashboardWithDependencies(true, dependencies)
    );
    await waitFor(() => expect(result.current.logout).not.toBeNull());
    const initial = result.current.logout;

    act(() => result.current.retry());
    await waitFor(() => expect(result.current.logout).not.toBeNull());

    expect(result.current.logout).toBe(initial);
  });
});

function waitUntilAborted(
  _guildId: string,
  signal: AbortSignal
): Promise<void> {
  return new Promise((resolve) => {
    if (signal.aborted) {
      resolve();
      return;
    }
    signal.addEventListener("abort", () => resolve(), { once: true });
  });
}
