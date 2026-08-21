import { AppShell } from "@astryxdesign/core/AppShell";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import {
  Layout,
  LayoutContent,
  LayoutFooter,
  LayoutHeader,
  LayoutPanel
} from "@astryxdesign/core/Layout";
import { ResizeHandle, useResizable } from "@astryxdesign/core/Resizable";
import { Section } from "@astryxdesign/core/Section";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { useToast } from "@astryxdesign/core/Toast";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { PanelRightOpen } from "lucide-react";
import { useEffect } from "react";

import { ConnectionBanner } from "../components/ConnectionBanner";
import { DashboardInspector } from "../components/DashboardInspector";
import { GuildSidebar } from "../components/GuildSidebar";
import { LoginScreen } from "../components/LoginScreen";
import { MediaSearchBar } from "../components/MediaSearchBar";
import { NowPlaying } from "../components/NowPlaying";
import { PlayerBar } from "../components/PlayerBar";
import { useDashboard } from "./use-dashboard";

export function App() {
  const session = useDashboard();
  const showToast = useToast();
  const { model } = session;
  const usesSideInspector = useMediaQuery("(min-width: 1025px)");
  const inspectorSize = useResizable({
    defaultSize: 380,
    minSizePx: 300,
    maxSizePx: 520,
    collapsible: true,
    collapsedSize: 220,
    snaps: [340, 380, 440],
    autoSaveId: "pepeaudio-queue-inspector"
  });
  const selectedGuild = model.guilds.find(
    (guild) => guild.id === model.selectedGuildId
  );

  useEffect(() => {
    if (session.feedback === null) return;
    showToast({
      body: session.feedback.message,
      type: session.feedback.type,
      uniqueID: "player-command-feedback",
      collisionBehavior: "overwrite"
    });
  }, [session.feedback, showToast]);

  if (session.account === null) {
    return (
      <LoginScreen
        status={session.status}
        message={session.message}
        onLogin={session.login}
        onRetry={session.retry}
      />
    );
  }

  const inspector = (
    <DashboardInspector
      presentation={usesSideInspector ? "panel" : "content"}
      queue={model.snapshot.queue}
      presets={model.presets}
      catalogStatus={model.hrirCatalogStatus}
      selectedPresetId={model.snapshot.hrirPresetId}
      snapshot={model.snapshot}
      connected={model.connected}
      commandPending={model.commandPending}
      onRemove={model.removeQueued}
      onMove={model.moveQueued}
      onPresetChange={model.setPreset}
      onSpatialToggle={model.toggleSpatial}
      onCollapse={usesSideInspector ? inspectorSize.collapse : undefined}
    />
  );

  return (
    <AppShell
      height="fill"
      variant="section"
      contentPadding={0}
      mobileNav={{ breakpoint: "md" }}
      banner={
        <ConnectionBanner
          status={session.status}
          demo={session.usingDemoData}
          message={session.message}
          onRetry={session.retry}
          onLogin={session.login}
        />
      }
      sideNav={
        <GuildSidebar
          guilds={model.guilds}
          selectedGuildId={model.selectedGuildId}
          commandPending={model.commandPending}
          account={session.account}
          status={session.status}
          onSelect={model.selectGuild}
          onLogout={session.logout}
          loggingOut={session.loggingOut}
        />
      }
    >
      <Layout
        height="fill"
        header={
          <LayoutHeader padding={4} hasDivider label="曲を追加">
            <HStack gap={2} width="100%" vAlign="center">
              <StackItem size="fill">
                <MediaSearchBar
                  isDisabled={selectedGuild?.active !== true || !model.connected}
                  isLoading={model.commandPending}
                  onSubmit={model.enqueueMedia}
                />
              </StackItem>
              {usesSideInspector && inspectorSize.isCollapsed ? (
                <IconButton
                  label="キューパネルを開く"
                  tooltip="キューを開く"
                  icon={<Icon icon={PanelRightOpen} />}
                  variant="ghost"
                  onClick={inspectorSize.expand}
                />
              ) : null}
            </HStack>
          </LayoutHeader>
        }
        content={
          <LayoutContent padding={0} isScrollable>
            <VStack>
              <NowPlaying guild={selectedGuild} snapshot={model.snapshot} />
              {!usesSideInspector ? (
                <Section
                  variant="transparent"
                  padding={6}
                  dividers={["top"]}
                  role="complementary"
                  aria-label="キューと360° Audioの設定"
                >
                  {inspector}
                </Section>
              ) : null}
            </VStack>
          </LayoutContent>
        }
        end={
          usesSideInspector ? (
            <>
              <ResizeHandle
                resizable={inspectorSize.props}
                isReversed
                hasDivider
                isAlwaysVisible={false}
                label="キューパネルの幅を調整"
              />
              <LayoutPanel
                resizable={inspectorSize.props}
                padding={0}
                role="complementary"
                label="キューと360° Audioの設定"
              >
                {inspector}
              </LayoutPanel>
            </>
          ) : undefined
        }
        footer={
          <LayoutFooter padding={0} hasDivider role="region" label="音楽プレイヤー">
            <PlayerBar model={model} />
          </LayoutFooter>
        }
      />
    </AppShell>
  );
}
