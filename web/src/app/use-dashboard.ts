import type { DashboardSession } from "./types";
import { runtimeConfig } from "./runtime-config";
import { useDemoDashboard } from "./use-demo-dashboard";
import { useLiveDashboard } from "./use-live-dashboard";

export function useDashboard(): DashboardSession {
  const demo = useDemoDashboard();
  const live = useLiveDashboard(!runtimeConfig.demoMode);
  return runtimeConfig.demoMode
      ? {
        status: "ready",
        model: demo,
        account: {
          source: "demo",
          userId: null,
          username: null,
          displayName: "デモアカウント",
          avatarUrl: null
        },
        usingDemoData: true,
        message: null,
        feedback: null,
        retry: () => undefined,
        login: () => undefined,
        logout: null,
        loggingOut: false
      }
    : live;
}
