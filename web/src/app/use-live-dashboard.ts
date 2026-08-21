import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  ApiResponseError,
  fetchAuthBootstrap,
  fetchHrirPresets,
  logoutSession,
  maintainPlayerEventStream,
  publicErrorMessage,
  type AuthBootstrap
} from "./api-client";
import { buildLiveDashboardModel, selectInitialGuild } from "./live-dashboard-model";
import type {
  DashboardFeedback,
  DashboardSession,
  HrirCatalogStatus,
  HrirPreset,
  PlayerSnapshot
} from "./types";
import { usePlayerCommand } from "./use-player-command";
import { toPlayerSnapshot } from "./wire-types";

export function useLiveDashboard(enabled: boolean): DashboardSession {
  return useLiveDashboardWithDependencies(enabled, defaultDependencies);
}

interface LiveDashboardDependencies {
  readonly fetchBootstrap: typeof fetchAuthBootstrap;
  readonly fetchPresets: typeof fetchHrirPresets;
  readonly maintainEvents: typeof maintainPlayerEventStream;
  readonly logout: typeof logoutSession;
}

const defaultDependencies: LiveDashboardDependencies = {
  fetchBootstrap: fetchAuthBootstrap,
  fetchPresets: fetchHrirPresets,
  maintainEvents: maintainPlayerEventStream,
  logout: logoutSession
};

export function useLiveDashboardWithDependencies(
  enabled: boolean,
  dependencies: LiveDashboardDependencies
): DashboardSession {
  const [auth, setAuth] = useState<AuthBootstrap | null>(null);
  const [selectedGuildId, setSelectedGuildId] = useState("");
  const [snapshot, setSnapshot] = useState<PlayerSnapshot | null>(null);
  const [presets, setPresets] = useState<readonly HrirPreset[]>([]);
  const [catalogStatus, setCatalogStatus] = useState<HrirCatalogStatus>("loading");
  const [message, setMessage] = useState<string | null>(null);
  const [reconnecting, setReconnecting] = useState(false);
  const [feedback, setFeedback] = useState<DashboardFeedback | null>(null);
  const [unauthenticated, setUnauthenticated] = useState(false);
  const [generation, setGeneration] = useState(0);
  const [loggingOut, setLoggingOut] = useState(false);
  const sessionGenerationRef = useRef(0);
  const csrfToken = auth?.csrfToken ?? null;
  const logoutAvailable = auth?.logoutAvailable ?? false;

  const clearPlayerState = useCallback(() => {
    setSelectedGuildId("");
    setSnapshot(null);
    setPresets([]);
    setCatalogStatus("loading");
    setReconnecting(false);
  }, []);

  const markUnauthorized = useCallback(() => {
    sessionGenerationRef.current += 1;
    setAuth(null);
    setFeedback(null);
    clearPlayerState();
    setUnauthenticated(true);
  }, [clearPlayerState]);

  const commandContext = useMemo(() => ({
    auth,
    selectedGuildId,
    snapshot,
    onSnapshot: setSnapshot,
    onMessage: (next: string) => setFeedback((current) => ({
      id: (current?.id ?? 0) + 1,
      message: next,
      type: "error"
    })),
    onUnauthorized: markUnauthorized
  }), [auth, markUnauthorized, selectedGuildId, snapshot]);
  const {
    pending: commandPending,
    run: runCommand,
    invalidate: invalidateCommands
  } = usePlayerCommand(commandContext);

  useEffect(() => {
    if (!enabled) return undefined;
    const controller = new AbortController();
    const bootstrap = async () => {
      try {
        const next = await dependencies.fetchBootstrap(controller.signal);
        if (controller.signal.aborted) return;
        setAuth(next);
        setSelectedGuildId((current) => selectInitialGuild(current, next.guilds));
        setUnauthenticated(false);
        setMessage(null);
        setReconnecting(false);
      } catch (error) {
        if (controller.signal.aborted) return;
        if (error instanceof ApiResponseError && error.status === 401) {
          markUnauthorized();
          setMessage("Discord OAuthでログインしてください。");
        } else {
          setMessage(publicErrorMessage(error, "認証状態を取得できませんでした。"));
        }
      }
    };
    void bootstrap();
    return () => controller.abort();
  }, [dependencies, enabled, generation, markUnauthorized]);

  useEffect(() => {
    if (!enabled || auth === null || !selectedGuildId) return undefined;
    const controller = new AbortController();
    const sessionGeneration = sessionGenerationRef.current;
    const isCurrent = () => sessionGenerationRef.current === sessionGeneration;
    setSnapshot(null);
    setReconnecting(false);
    void dependencies.maintainEvents(
      selectedGuildId,
      controller.signal,
      (wire) => {
        if (!controller.signal.aborted && isCurrent()) {
          setSnapshot(toPlayerSnapshot(wire));
          setMessage(null);
          setReconnecting(false);
        }
      },
      (error, delayMs) => {
        if (controller.signal.aborted || !isCurrent()) return;
        if (error instanceof ApiResponseError && error.status === 401) {
          invalidateCommands();
          markUnauthorized();
          setMessage(error.message);
          controller.abort();
          return;
        }
        if (error instanceof ApiResponseError && error.status === 403) {
          invalidateCommands();
          setSelectedGuildId("");
          setSnapshot(null);
          setReconnecting(false);
          setMessage(
            "このサーバーではPepeAudioを利用できません。Botの参加状態を確認して再試行してください。"
          );
          controller.abort();
          return;
        }
        const seconds = Math.max(1, Math.ceil(delayMs / 1_000));
        setReconnecting(true);
        setMessage(`リアルタイム接続を再接続しています（${seconds}秒以内）。`);
      }
    );
    return () => controller.abort();
  }, [auth, dependencies, enabled, generation, invalidateCommands, markUnauthorized, selectedGuildId]);

  useEffect(() => {
    if (!enabled || auth === null || !selectedGuildId) {
      setPresets([]);
      setCatalogStatus(auth === null ? "loading" : "ready");
      return undefined;
    }
    const controller = new AbortController();
    setPresets([]);
    setCatalogStatus("loading");
    const loadCatalog = async () => {
      try {
        const next = await dependencies.fetchPresets(selectedGuildId, controller.signal);
        if (controller.signal.aborted) return;
        setPresets(next);
        setCatalogStatus("ready");
      } catch (error) {
        if (controller.signal.aborted) return;
        if (error instanceof ApiResponseError && error.status === 401) {
          invalidateCommands();
          markUnauthorized();
          setMessage(error.message);
          return;
        }
        setPresets([]);
        setCatalogStatus("unavailable");
      }
    };
    void loadCatalog();
    return () => controller.abort();
  }, [auth, dependencies, enabled, generation, invalidateCommands, markUnauthorized, selectedGuildId]);

  const logout = useCallback(async () => {
    if (csrfToken === null || !logoutAvailable || loggingOut) return;
    setLoggingOut(true);
    try {
      await dependencies.logout(csrfToken);
      sessionGenerationRef.current += 1;
      invalidateCommands();
      setAuth(null);
      setFeedback(null);
      clearPlayerState();
      setUnauthenticated(true);
      setMessage("ログアウトしました。Discordで再度ログインできます。");
    } catch (error) {
      if (error instanceof ApiResponseError && error.status === 401) {
        invalidateCommands();
        markUnauthorized();
      }
      setMessage(publicErrorMessage(error, "ログアウトできませんでした。もう一度お試しください。"));
    } finally {
      setLoggingOut(false);
    }
  }, [
    clearPlayerState,
    csrfToken,
    dependencies,
    invalidateCommands,
    loggingOut,
    logoutAvailable,
    markUnauthorized
  ]);

  const selectGuild = useCallback((guildId: string) => {
    if (commandPending) return;
    sessionGenerationRef.current += 1;
    setSelectedGuildId(guildId);
    setSnapshot(null);
    setPresets([]);
    setCatalogStatus("loading");
    setMessage(null);
    setReconnecting(false);
  }, [commandPending]);
  const model = useMemo(() => buildLiveDashboardModel({
    guilds: auth?.guilds ?? [],
    selectedGuildId,
    snapshot,
    presets,
    catalogStatus,
    commandPending,
    run: runCommand,
    selectGuild
  }), [
    auth,
    catalogStatus,
    commandPending,
    presets,
    runCommand,
    selectedGuildId,
    selectGuild,
    snapshot
  ]);

  const retry = useCallback(() => {
    sessionGenerationRef.current += 1;
    invalidateCommands();
    setMessage(null);
    setReconnecting(false);
    setGeneration((current) => current + 1);
  }, [invalidateCommands]);

  const login = useCallback(() => {
    window.location.assign("/auth/login");
  }, []);
  const logoutAction = useMemo(
    () => logoutAvailable ? () => void logout() : null,
    [logout, logoutAvailable]
  );

  return {
    status: unauthenticated
      ? "unauthenticated"
      : reconnecting && snapshot !== null
        ? "reconnecting"
        : message
          ? "unavailable"
        : snapshot !== null || (auth !== null && selectedGuildId === "")
          ? "ready"
          : "connecting",
    model,
    account: auth?.account ?? null,
    usingDemoData: false,
    message,
    feedback,
    retry,
    login,
    logout: logoutAction,
    loggingOut
  };
}
