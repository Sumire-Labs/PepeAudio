import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import {
  SideNav,
  SideNavHeading,
  SideNavItem,
  SideNavSection
} from "@astryxdesign/core/SideNav";
import { HStack } from "@astryxdesign/core/Stack";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { Text } from "@astryxdesign/core/Text";
import { ServerOff } from "lucide-react";
import { useCallback } from "react";

import type { DashboardAccount, DashboardStatus, GuildSummary } from "../app/types";
import { AccountPanel } from "./AccountPanel";

interface GuildSidebarProps {
  readonly guilds: readonly GuildSummary[];
  readonly selectedGuildId: string;
  readonly commandPending?: boolean;
  readonly account: DashboardAccount | null;
  readonly status: DashboardStatus;
  readonly onSelect: (guildId: string) => void;
  readonly onLogout: (() => void) | null;
  readonly loggingOut: boolean;
}

export function GuildSidebar({
  guilds,
  selectedGuildId,
  commandPending = false,
  account,
  status,
  onSelect,
  onLogout,
  loggingOut
}: GuildSidebarProps) {
  const currentPlayer = guilds.filter((guild) => guild.active && guild.connected);
  const available = guilds.filter((guild) => guild.active && !guild.connected);
  const unavailable = guilds.filter((guild) => !guild.active);
  const hideNavigationScrollbar = useCallback((section: HTMLDivElement | null) => {
    if (section?.parentElement) {
      section.parentElement.style.scrollbarWidth = "none";
    }
    let candidate = section?.parentElement ?? null;
    while (candidate !== null) {
      const overflow = getComputedStyle(candidate).overflowY;
      if (overflow === "auto" || overflow === "scroll") {
        candidate.style.scrollbarWidth = "none";
        break;
      }
      candidate = candidate.parentElement;
    }
  }, []);

  return (
    <SideNav
      collapsible={{
        defaultIsCollapsed: true,
        buttonLabel: "サーバー一覧を開閉"
      }}
      resizable={{
        defaultWidth: 280,
        minWidth: 240,
        maxWidth: 360,
        autoSaveId: "pepeaudio-guild-sidebar"
      }}
      header={
        <SideNavHeading
          icon={
            <Avatar
              src="/branding/bot-icon.png"
              name="PepeAudio"
              size="sm"
              tooltip={false}
            />
          }
          heading="PepeAudio"
          subheading="音楽ダッシュボード"
        />
      }
      footer={
        <AccountPanel
          account={account}
          status={status}
          onLogout={onLogout}
          loggingOut={loggingOut}
        />
      }
    >
      {guilds.length === 0 ? (
        <SideNavSection ref={hideNavigationScrollbar} title="Discordサーバー">
          <EmptyState
            headingLevel={3}
            isCompact
            title="利用できるサーバーがありません"
            description="DiscordでBotを追加すると、ここから再生状態を確認できます。"
            icon={<Icon icon={ServerOff} />}
          />
        </SideNavSection>
      ) : (
        <>
          <GuildGroup
            title="現在のプレイヤー"
            guilds={currentPlayer}
            selectedGuildId={selectedGuildId}
            commandPending={commandPending}
            onSelect={onSelect}
            sectionRef={hideNavigationScrollbar}
          />
          <GuildGroup
            title="Bot導入済み"
            guilds={available}
            selectedGuildId={selectedGuildId}
            commandPending={commandPending}
            onSelect={onSelect}
          />
          <GuildGroup
            title="その他のサーバー"
            guilds={unavailable}
            selectedGuildId={selectedGuildId}
            commandPending={commandPending}
            onSelect={onSelect}
          />
        </>
      )}
    </SideNav>
  );
}

interface GuildGroupProps {
  readonly title: string;
  readonly guilds: readonly GuildSummary[];
  readonly selectedGuildId: string;
  readonly commandPending: boolean;
  readonly onSelect: (guildId: string) => void;
  readonly sectionRef?: (section: HTMLDivElement | null) => void;
}

function GuildGroup({
  title,
  guilds,
  selectedGuildId,
  commandPending,
  onSelect,
  sectionRef
}: GuildGroupProps) {
  if (guilds.length === 0) return null;
  return (
    <SideNavSection
      {...(sectionRef ? { ref: sectionRef } : {})}
      title={title}
      endContent={<Badge variant="neutral" label={String(guilds.length)} />}
    >
      {guilds.map((guild) => {
        const guildStatus = describeGuildStatus(guild);
        return (
          <SideNavItem
            key={guild.id}
            label={guild.name}
            icon={
              <Avatar
                {...(guild.iconUrl === null ? {} : { src: guild.iconUrl })}
                name={initialsName(guild.initials)}
                size="sm"
                tooltip={false}
              />
            }
            endContent={
              <HStack gap={1} vAlign="center">
                <StatusDot
                  variant={guildStatusVariant(guild)}
                  label={guildStatus}
                  isPulsing={guild.connected}
                />
                <Text type="supporting" color="secondary" aria-hidden="true">
                  {shortGuildStatus(guild)}
                </Text>
              </HStack>
            }
            isSelected={guild.id === selectedGuildId}
            isDisabled={!guild.active || commandPending}
            onClick={() => onSelect(guild.id)}
          />
        );
      })}
    </SideNavSection>
  );
}

function shortGuildStatus(guild: GuildSummary): string {
  if (!guild.active) return "未導入";
  if (!guild.connected) return "待機中";
  return guild.listenerCount === null ? "使用中" : `${guild.listenerCount}人`;
}

function describeGuildStatus(guild: GuildSummary): string {
  if (!guild.active) return "PepeAudio未導入";
  if (!guild.connected) return "Bot導入済み・待機中";
  return guild.listenerCount === null
    ? "現在のプレイヤー"
    : `${guild.listenerCount}人が参加中`;
}

function guildStatusVariant(guild: GuildSummary): "success" | "warning" | "neutral" {
  if (!guild.active) return "neutral";
  return guild.connected ? "success" : "warning";
}

function initialsName(initials: string): string {
  return Array.from(initials).join(" ");
}
