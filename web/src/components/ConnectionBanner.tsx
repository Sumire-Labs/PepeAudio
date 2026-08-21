import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { Spinner } from "@astryxdesign/core/Spinner";
import { LogIn, RefreshCw } from "lucide-react";

import type { DashboardStatus } from "../app/types";

interface ConnectionBannerProps {
  readonly status: DashboardStatus;
  readonly demo: boolean;
  readonly message: string | null;
  readonly onRetry: () => void;
  readonly onLogin: () => void;
}

export function ConnectionBanner({
  status,
  demo,
  message,
  onRetry,
  onLogin
}: ConnectionBannerProps) {
  if (status === "ready" && !demo) return null;

  if (demo) {
    return (
      <Banner
        status="info"
        container="section"
        title="デモ表示中"
        description="Discordとは同期していません。操作はこの画面内だけに反映されます。"
      />
    );
  }

  const connecting = status === "connecting";
  const reconnecting = status === "reconnecting";
  return (
    <Banner
      status={
        status === "unauthenticated"
          ? "warning"
          : connecting || reconnecting
            ? "info"
            : "error"
      }
      container="section"
      title={
        connecting
          ? "PepeAudioに接続しています"
          : reconnecting
            ? "リアルタイム接続を復旧しています"
            : statusTitle(status)
      }
      description={
        connecting
          ? "再生状態を同期しています…"
          : message ?? fallbackMessage(status)
      }
      icon={connecting ? <Spinner size="sm" aria-label="接続中" /> : undefined}
      endContent={
        status === "unauthenticated" ? (
          <Button
            label="Discordでログイン"
            variant="primary"
            icon={<Icon icon={LogIn} />}
            onClick={onLogin}
          />
        ) : status === "unavailable" || reconnecting ? (
          <Button
            label="再試行"
            variant="secondary"
            icon={<Icon icon={RefreshCw} />}
            onClick={onRetry}
          />
        ) : undefined
      }
    />
  );
}

function statusTitle(status: DashboardStatus): string {
  return status === "unauthenticated" ? "ログインが必要です" : "接続できませんでした";
}

function fallbackMessage(status: DashboardStatus): string {
  if (status === "unauthenticated") {
    return "Discordアカウントでログインしてください。";
  }
  if (status === "reconnecting") {
    return "最後に受信した再生状態を表示したまま、自動で再接続しています。";
  }
  return "時間をおいて再試行してください。";
}
