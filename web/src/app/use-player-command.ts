import { useCallback, useRef, useState } from "react";

import {
  ApiResponseError,
  UserFacingError,
  commandFailureMessage,
  publicErrorMessage,
  sendPlayerCommand,
  waitForCommandResult,
  waitForSnapshotRevision,
  type AuthBootstrap
} from "./api-client";
import type { PlayerSnapshot } from "./types";
import { toPlayerSnapshot, type PlayerCommand } from "./wire-types";

interface PlayerCommandContext {
  readonly auth: AuthBootstrap | null;
  readonly selectedGuildId: string;
  readonly snapshot: PlayerSnapshot | null;
  readonly onSnapshot: (snapshot: PlayerSnapshot) => void;
  readonly onMessage: (message: string) => void;
  readonly onUnauthorized: () => void;
}

interface PlayerCommandDependencies {
  readonly send: typeof sendPlayerCommand;
  readonly waitForResult: typeof waitForCommandResult;
  readonly waitForRevision: typeof waitForSnapshotRevision;
}

const defaultDependencies: PlayerCommandDependencies = {
  send: sendPlayerCommand,
  waitForResult: waitForCommandResult,
  waitForRevision: waitForSnapshotRevision
};

export function usePlayerCommand(
  context: PlayerCommandContext,
  dependencies: PlayerCommandDependencies = defaultDependencies
) {
  const [pending, setPending] = useState(false);
  const pendingRef = useRef(false);
  const generationRef = useRef(0);

  const invalidate = useCallback(() => {
    generationRef.current += 1;
    pendingRef.current = false;
    setPending(false);
  }, []);

  const run = useCallback(async (command: PlayerCommand) => {
    const { auth, selectedGuildId, snapshot } = context;
    if (snapshot === null || auth === null || pendingRef.current) return;

    const generation = generationRef.current;
    const isCurrent = () => generationRef.current === generation;
    pendingRef.current = true;
    setPending(true);
    try {
      const receipt = await dependencies.send(
        selectedGuildId,
        snapshot.revision,
        command,
        auth.csrfToken
      );
      if (!isCurrent()) return;

      const result = await dependencies.waitForResult(
        selectedGuildId,
        receipt.command_id,
        command.type === "enqueue_media" ? 330_000 : 10_000
      );
      if (!isCurrent()) return;
      if (result.status !== "applied") {
        throw new UserFacingError(commandFailureMessage(result));
      }

      const confirmed = await dependencies.waitForRevision(
        selectedGuildId,
        result.resulting_revision
      );
      if (isCurrent()) context.onSnapshot(toPlayerSnapshot(confirmed));
    } catch (error) {
      if (!isCurrent()) return;
      if (error instanceof ApiResponseError && error.status === 401) {
        context.onUnauthorized();
      }
      context.onMessage(publicErrorMessage(error, "操作に失敗しました。"));
    } finally {
      if (isCurrent()) {
        pendingRef.current = false;
        setPending(false);
      }
    }
  }, [context, dependencies]);

  return { pending, run, invalidate } as const;
}
