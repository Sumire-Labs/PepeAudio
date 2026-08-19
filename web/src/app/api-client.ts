import type { PlayerCommand, PlayerSnapshotWire } from "./wire-types";
import type { HrirPreset } from "./types";
import { runtimeConfig } from "./runtime-config";
import { interpretPlayerSseFrame, parsePlayerSnapshotWire } from "./player-sse";
import { parseHrirPresetCatalog } from "./hrir-catalog";
import {
  parseCommandReceipt,
  parseCommandResult,
  type CommandReceiptWire,
  type CommandResultWire,
  type TerminalCommandResultWire
} from "./command-result";
import { ApiResponseError, readJsonResponse } from "./api-response";

export { interpretPlayerSseFrame, parseSseFrame } from "./player-sse";
export { parseHrirPresetCatalog };
export { commandFailureMessage, parseCommandReceipt, parseCommandResult } from "./command-result";
export { ApiResponseError, UserFacingError, publicErrorMessage } from "./api-response";
export { fetchAuthBootstrap, logoutSession } from "./auth-client";
export { discordGuildIconUrl, discordUserAvatarUrl } from "./auth-wire";
export type { AuthBootstrap } from "./auth-client";
export type { AuthGuild } from "./auth-wire";

const JSON_HEADERS = { "content-type": "application/json" } as const;
const UTF8_ENCODER = new TextEncoder();
const MAX_PLAYER_EVENT_FRAME_BYTES = 1_048_576;

export async function fetchSnapshot(
  guildId: string,
  signal?: AbortSignal
): Promise<PlayerSnapshotWire> {
  const response = await fetch(playerUrl(guildId), {
    headers: authHeaders(),
    credentials: "same-origin",
    signal: signal ?? null
  });
  return parsePlayerSnapshotWire(await readJsonResponse(response), guildId);
}

export async function fetchHrirPresets(
  guildId: string,
  signal?: AbortSignal
): Promise<readonly HrirPreset[]> {
  const response = await fetch(
    `${runtimeConfig.apiBaseUrl}/guilds/${encodeURIComponent(guildId)}/hrir-presets`,
    {
      headers: authHeaders(),
      credentials: "same-origin",
      signal: signal ?? null
    }
  );
  return parseHrirPresetCatalog(await readJsonResponse(response), guildId);
}

export async function sendPlayerCommand(
  guildId: string,
  expectedRevision: number,
  command: PlayerCommand,
  csrfToken: string
): Promise<CommandReceiptWire> {
  const response = await fetch(`${playerUrl(guildId)}/commands`, {
    method: "POST",
    credentials: "same-origin",
    headers: {
      ...JSON_HEADERS,
      ...authHeaders(),
      "x-csrf-token": csrfToken,
      "idempotency-key": crypto.randomUUID()
    },
    body: JSON.stringify({
      expected_revision: expectedRevision,
      command
    })
  });
  return parseCommandReceipt(await readJsonResponse(response));
}

export async function waitForSnapshotRevision(
  guildId: string,
  targetRevision: number,
  timeoutMs = 5_000,
  dependencies: SnapshotWaitDependencies = {}
): Promise<PlayerSnapshotWire> {
  const fetchCurrent = dependencies.fetchCurrent ?? fetchSnapshot;
  const wait = dependencies.wait ?? delay;
  const now = dependencies.now ?? Date.now;
  const deadline = now() + timeoutMs;
  let delayMs = 80;
  while (now() < deadline) {
    const snapshot = await fetchCurrent(guildId);
    if (snapshot.revision >= targetRevision) return snapshot;
    await wait(delayMs);
    delayMs = Math.min(400, Math.ceil(delayMs * 1.6));
  }
  throw new Error("操作は受理されましたが、担当Botによる反映を確認できませんでした。状態を再読み込みしてください。");
}

export interface SnapshotWaitDependencies {
  readonly fetchCurrent?: typeof fetchSnapshot;
  readonly wait?: (milliseconds: number) => Promise<void>;
  readonly now?: () => number;
}

export async function fetchCommandResult(
  guildId: string,
  commandId: string
): Promise<CommandResultWire> {
  const response = await fetch(
    `${playerUrl(guildId)}/commands/${encodeURIComponent(commandId)}`,
    {
      headers: authHeaders(),
      credentials: "same-origin"
    }
  );
  return parseCommandResult(await readJsonResponse(response), guildId, commandId);
}

export async function waitForCommandResult(
  guildId: string,
  commandId: string,
  timeoutMs = 10_000,
  dependencies: CommandResultWaitDependencies = {}
): Promise<TerminalCommandResultWire> {
  const fetchResult = dependencies.fetchResult ?? fetchCommandResult;
  const wait = dependencies.wait ?? delay;
  const now = dependencies.now ?? Date.now;
  const deadline = now() + timeoutMs;
  let delayMs = 80;
  while (now() < deadline) {
    let result: CommandResultWire;
    try {
      result = await fetchResult(guildId, commandId);
    } catch (error) {
      if (error instanceof ApiResponseError && error.status === 404) {
        throw new Error("操作結果の保持期限が切れたか、結果を確認できません。成功したものとして扱わず、最新の状態を確認してください。");
      }
      throw error;
    }
    if (result.status !== "pending") return result;
    await wait(delayMs);
    delayMs = Math.min(400, Math.ceil(delayMs * 1.6));
  }
  throw new Error("操作結果を確認できませんでした。成功したものとして扱わず、最新の状態を確認してください。");
}

export interface CommandResultWaitDependencies {
  readonly fetchResult?: typeof fetchCommandResult;
  readonly wait?: (milliseconds: number) => Promise<void>;
  readonly now?: () => number;
}

export async function streamPlayerEvents(
  guildId: string,
  afterRevision: number,
  signal: AbortSignal,
  onSnapshot: (snapshot: PlayerSnapshotWire) => void
): Promise<void> {
  const response = await fetch(
    `${runtimeConfig.apiBaseUrl}/guilds/${encodeURIComponent(guildId)}/events`,
    {
      headers: { ...authHeaders(), "last-event-id": afterRevision.toString() },
      credentials: "same-origin",
      signal
    }
  );
  if (!response.ok) {
    await readJsonResponse(response);
  }
  if (response.body === null) {
    throw new Error(`Player event stream returned HTTP ${response.status}`);
  }

  const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
  let pending = "";
  let pendingBytes = 0;
  let currentRevision = afterRevision;
  try {
    while (!signal.aborted) {
      const { done, value } = await reader.read();
      if (done) return;
      pending += value;
      pendingBytes += UTF8_ENCODER.encode(value).byteLength;
      let boundary = findSseBoundary(pending);
      while (boundary !== null) {
        const frame = pending.slice(0, boundary.index);
        const frameBytes = UTF8_ENCODER.encode(frame).byteLength;
        if (frameBytes > MAX_PLAYER_EVENT_FRAME_BYTES) {
          throw new Error("Player event frame exceeded the client limit");
        }
        const action = interpretPlayerSseFrame(
          frame.replaceAll("\r\n", "\n"),
          guildId,
          currentRevision
        );
        pending = pending.slice(boundary.index + boundary.delimiterBytes);
        pendingBytes -= frameBytes + boundary.delimiterBytes;
        if (action.kind === "resync") {
          throw new Error("Player event stream requires a snapshot resync");
        }
        if (action.kind === "snapshot") {
          currentRevision = action.snapshot.revision;
          onSnapshot(action.snapshot);
        }
        boundary = findSseBoundary(pending);
      }
      if (pendingBytes > MAX_PLAYER_EVENT_FRAME_BYTES) {
        throw new Error("Player event frame exceeded the client limit");
      }
    }
  } finally {
    await reader.cancel().catch(() => undefined);
    reader.releaseLock();
  }
}

function findSseBoundary(value: string): { index: number; delimiterBytes: 2 | 4 } | null {
  const lineFeed = value.indexOf("\n\n");
  const carriageReturn = value.indexOf("\r\n\r\n");
  if (lineFeed < 0 && carriageReturn < 0) return null;
  if (carriageReturn >= 0 && (lineFeed < 0 || carriageReturn < lineFeed)) {
    return { index: carriageReturn, delimiterBytes: 4 };
  }
  return { index: lineFeed, delimiterBytes: 2 };
}

export interface PlayerEventRecoveryDependencies {
  readonly fetchCurrent?: typeof fetchSnapshot;
  readonly openStream?: typeof streamPlayerEvents;
  readonly random?: () => number;
  readonly wait?: (signal: AbortSignal, milliseconds: number) => Promise<boolean>;
}

export async function maintainPlayerEventStream(
  guildId: string,
  signal: AbortSignal,
  onSnapshot: (snapshot: PlayerSnapshotWire) => void,
  onRetry: (error: unknown, delayMs: number) => void,
  dependencies: PlayerEventRecoveryDependencies = {}
): Promise<void> {
  const fetchCurrent = dependencies.fetchCurrent ?? fetchSnapshot;
  const openStream = dependencies.openStream ?? streamPlayerEvents;
  const random = dependencies.random ?? Math.random;
  const wait = dependencies.wait ?? waitForReconnect;
  let lastRevision: number | null = null;
  let consecutiveFailures = 0;

  while (!signal.aborted) {
    try {
      const fresh = await fetchCurrent(guildId, signal);
      if (signal.aborted) return;
      if (lastRevision !== null && fresh.revision < lastRevision) {
        throw new Error("Player snapshot revision moved backwards");
      }
      if (lastRevision === null || fresh.revision > lastRevision) consecutiveFailures = 0;
      lastRevision = fresh.revision;
      onSnapshot(fresh);
      await openStream(guildId, fresh.revision, signal, (next) => {
        if (next.revision > (lastRevision ?? -1)) consecutiveFailures = 0;
        lastRevision = next.revision;
        onSnapshot(next);
      });
      if (signal.aborted) return;
    } catch (error) {
      if (signal.aborted) return;
      const delayMs = sseReconnectDelayMs(consecutiveFailures, random());
      consecutiveFailures += 1;
      onRetry(error, delayMs);
      if (!(await wait(signal, delayMs))) return;
      continue;
    }

    const delayMs = sseReconnectDelayMs(consecutiveFailures, random());
    consecutiveFailures += 1;
    onRetry(new Error("Player event stream ended"), delayMs);
    if (!(await wait(signal, delayMs))) return;
  }
}

export function sseReconnectDelayMs(attempt: number, randomValue: number): number {
  const boundedAttempt = Number.isFinite(attempt)
    ? Math.min(16, Math.max(0, Math.trunc(attempt)))
    : 0;
  const unit = Number.isFinite(randomValue)
    ? Math.min(1, Math.max(0, randomValue))
    : 0.5;
  const ceiling = Math.min(10_000, 250 * (2 ** boundedAttempt));
  return Math.max(1, Math.floor(ceiling * (0.5 + unit * 0.5)));
}

function playerUrl(guildId: string): string {
  return `${runtimeConfig.apiBaseUrl}/guilds/${encodeURIComponent(guildId)}/player`;
}

function authHeaders(): Record<string, string> {
  return runtimeConfig.devUserId === null
    ? {}
    : { "x-pepeaudio-dev-user-id": runtimeConfig.devUserId };
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

function waitForReconnect(signal: AbortSignal, milliseconds: number): Promise<boolean> {
  if (signal.aborted) return Promise.resolve(false);
  return new Promise((resolve) => {
    const timer = window.setTimeout(() => {
      signal.removeEventListener("abort", cancel);
      resolve(true);
    }, milliseconds);
    const cancel = () => {
      window.clearTimeout(timer);
      resolve(false);
    };
    signal.addEventListener("abort", cancel, { once: true });
  });
}
