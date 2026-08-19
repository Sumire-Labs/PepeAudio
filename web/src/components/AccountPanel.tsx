import { Avatar } from "@astryxdesign/core/Avatar";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Item } from "@astryxdesign/core/Item";
import { LogOut } from "lucide-react";

import type { DashboardAccount, DashboardStatus } from "../app/types";

interface AccountPanelProps {
  readonly account: DashboardAccount | null;
  readonly status: DashboardStatus;
  readonly onLogout: (() => void) | null;
  readonly loggingOut: boolean;
}

export function AccountPanel({
  account,
  status,
  onLogout,
  loggingOut
}: AccountPanelProps) {
  const copy = accountCopy(account, status);
  return (
    <Item
      density="balanced"
      label={copy.label}
      description={copy.description}
      labelLines={1}
      descriptionLines={1}
      startContent={
        <Avatar
          {...(account?.avatarUrl ? { src: account.avatarUrl } : {})}
          name={copy.label}
          size="md"
          tooltip={false}
        />
      }
      endContent={
        onLogout === null ? undefined : (
          <IconButton
            label="PepeAudioからログアウト"
            tooltip="ログアウト"
            icon={<Icon icon={LogOut} />}
            variant="ghost"
            size="sm"
            isLoading={loggingOut}
            isDisabled={loggingOut}
            onClick={onLogout}
          />
        )
      }
    />
  );
}

function accountCopy(
  account: DashboardAccount | null,
  status: DashboardStatus
): { readonly label: string; readonly description: string } {
  if (account !== null) {
    const source = account.source === "demo"
      ? "ローカルプレビュー"
      : account.source === "development"
        ? "開発環境"
        : "Discordでログイン中";
    return {
      label: account.displayName,
      description: account.username ? `@${account.username}` : source
    };
  }
  if (status === "connecting") {
    return { label: "Discordアカウント", description: "確認しています…" };
  }
  if (status === "unauthenticated") {
    return { label: "Discordアカウント", description: "未ログイン" };
  }
  return { label: "Discordアカウント", description: "アカウントを確認できません" };
}
