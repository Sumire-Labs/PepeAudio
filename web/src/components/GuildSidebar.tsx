import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { NavIcon } from "@astryxdesign/core/NavIcon";
import {
  SideNav,
  SideNavHeading,
  SideNavItem,
  SideNavSection
} from "@astryxdesign/core/SideNav";
import { HStack } from "@astryxdesign/core/Stack";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { Text } from "@astryxdesign/core/Text";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Headphones, ServerOff } from "lucide-react";
import { useCallback, useState } from "react";

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
  const [isCollapsed, setIsCollapsed] = useState(false);
  const usesInlineSidebar = useMediaQuery("(min-width: 769px)");
  const hideNavigationScrollbar = useCallback((section: HTMLDivElement | null) => {
    const scrollRegion = section?.parentElement;
    if (scrollRegion !== null && scrollRegion !== undefined) {
      scrollRegion.style.scrollbarWidth = "none";
    }
  }, []);

  return (
    <SideNav
      collapsible={{
        isCollapsed,
        onCollapsedChange: setIsCollapsed
      }}
      {...(usesInlineSidebar && !isCollapsed ? { style: { width: 320 } } : {})}
      header={
        <SideNavHeading
          icon={
            <NavIcon icon={<Icon icon={Headphones} color="inherit" />} />
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
      <SideNavSection
        ref={hideNavigationScrollbar}
        title="Discordサーバー"
        endContent={<Badge variant="neutral" label={String(guilds.length)} />}
      >
        {guilds.length === 0 ? (
          <EmptyState
            headingLevel={3}
            isCompact
            title="利用できるサーバーがありません"
            description="DiscordでBotを追加すると、ここから再生状態を確認できます。"
            icon={<Icon icon={ServerOff} />}
          />
        ) : guilds.map((guild) => {
          const selected = guild.id === selectedGuildId;
          const status = describeGuildStatus(guild);
          return (
            <SideNavItem
              key={guild.id}
              label={guild.name}
              icon={
                <HStack aria-hidden="true">
                  <Avatar
                    {...(guild.iconUrl === null ? {} : { src: guild.iconUrl })}
                    name={initialsName(guild.initials)}
                    size="sm"
                    tooltip={false}
                  />
                </HStack>
              }
              endContent={
                <HStack gap={1} vAlign="center">
                  <StatusDot
                    variant={guildStatusVariant(guild)}
                    label={status}
                    isPulsing={guild.connected}
                  />
                  <Text type="supporting" color="secondary" aria-hidden="true">
                    {shortGuildStatus(guild)}
                  </Text>
                </HStack>
              }
              isSelected={selected}
              isDisabled={!guild.active || commandPending}
              onClick={() => onSelect(guild.id)}
            />
          );
        })}
      </SideNavSection>
    </SideNav>
  );
}

function shortGuildStatus(guild: GuildSummary): string {
  if (!guild.active) return "未参加";
  if (!guild.connected) return "未接続";
  return guild.listenerCount === null ? "接続中" : `${guild.listenerCount}人`;
}

function describeGuildStatus(guild: GuildSummary): string {
  if (!guild.active) return "PepeAudio未参加";
  if (!guild.connected) return "ボイスチャンネル未接続";
  return guild.listenerCount === null
    ? "ボイスチャンネル接続中"
    : `${guild.listenerCount}人が参加中`;
}

function guildStatusVariant(guild: GuildSummary): "success" | "warning" | "neutral" {
  if (!guild.active) return "neutral";
  return guild.connected ? "success" : "warning";
}

function initialsName(initials: string): string {
  return Array.from(initials).join(" ");
}
