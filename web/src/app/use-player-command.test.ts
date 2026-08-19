import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { createDemoSnapshot } from "./demo-data";
import { usePlayerCommand } from "./use-player-command";

const COMMAND_ID = "00000000-0000-0000-0000-000000000001";
const IDEMPOTENCY_KEY = "00000000-0000-0000-0000-000000000002";

describe("usePlayerCommand", () => {
  it("ignores a delayed command receipt after the session is invalidated", async () => {
    const receipt = deferred<{
      command_id: string;
      idempotency_key: string;
      resulting_revision: null;
      replayed: false;
    }>();
    const onSnapshot = vi.fn();
    const waitForResult = vi.fn();
    const waitForRevision = vi.fn();
    const { result } = renderHook(() => usePlayerCommand({
      auth: {
        csrfToken: "token",
        guilds: [],
        account: testAccount,
        logoutAvailable: true
      },
      selectedGuildId: "1",
      snapshot: createDemoSnapshot("1"),
      onSnapshot,
      onMessage: vi.fn(),
      onUnauthorized: vi.fn()
    }, {
      send: vi.fn(() => receipt.promise),
      waitForResult,
      waitForRevision
    }));

    let operation: Promise<void> | undefined;
    act(() => {
      operation = result.current.run({ type: "pause" });
    });
    expect(result.current.pending).toBe(true);

    act(() => result.current.invalidate());
    receipt.resolve({
      command_id: COMMAND_ID,
      idempotency_key: IDEMPOTENCY_KEY,
      resulting_revision: null,
      replayed: false
    });
    await act(async () => operation);

    expect(result.current.pending).toBe(false);
    expect(waitForResult).not.toHaveBeenCalled();
    expect(waitForRevision).not.toHaveBeenCalled();
    expect(onSnapshot).not.toHaveBeenCalled();
  });

  it("blocks a second command before React has committed the pending state", async () => {
    const receipt = deferred<never>();
    const send = vi.fn(() => receipt.promise);
    const { result } = renderHook(() => usePlayerCommand({
      auth: {
        csrfToken: "token",
        guilds: [],
        account: testAccount,
        logoutAvailable: true
      },
      selectedGuildId: "1",
      snapshot: createDemoSnapshot("1"),
      onSnapshot: vi.fn(),
      onMessage: vi.fn(),
      onUnauthorized: vi.fn()
    }, {
      send,
      waitForResult: vi.fn(),
      waitForRevision: vi.fn()
    }));

    let firstOperation: Promise<void> | undefined;
    act(() => {
      firstOperation = result.current.run({ type: "pause" });
      void result.current.run({ type: "skip" });
    });

    expect(send).toHaveBeenCalledOnce();
    act(() => result.current.invalidate());
    receipt.reject(new Error("cancelled by test"));
    await act(async () => firstOperation);
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const testAccount = {
  source: "discord" as const,
  userId: "1",
  username: "listener",
  displayName: "Listener",
  avatarUrl: null
};
