import { AppShell } from "@astryxdesign/core/AppShell";
import { Center } from "@astryxdesign/core/Center";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Layout, LayoutContent, LayoutPanel } from "@astryxdesign/core/Layout";
import { ResizeHandle, useResizable } from "@astryxdesign/core/Resizable";
import { Section } from "@astryxdesign/core/Section";
import { StackItem, VStack } from "@astryxdesign/core/Stack";
import { useToast } from "@astryxdesign/core/Toast";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { PanelRightOpen } from "lucide-react";
import { useEffect } from "react";

import { ConnectionBanner } from "../components/ConnectionBanner";
import { DashboardInspector } from "../components/DashboardInspector";
import { GuildSidebar } from "../components/GuildSidebar";
import { LoginScreen } from "../components/LoginScreen";
import { NowPlaying } from "../components/NowPlaying";
import { PlayerBar } from "../components/PlayerBar";
import {
  INSPECTOR_RAIL_WIDTH,
  inspectorPanelSizing
} from "./inspector-panel-sizing";
import { useDashboard } from "./use-dashboard";

export function App() {
  const session = useDashboard();
  const showToast = useToast();
  const { model } = session;
  const usesSideInspector = useMediaQuery("(min-width: 769px)");
  const inspectorSize = useResizable({
    defaultSize: 440,
    minSizePx: 360,
    maxSizePx: 600,
    collapsible: true,
    collapsedSize: INSPECTOR_RAIL_WIDTH,
    snaps: [400, 440, 520],
    autoSaveId: "pepeaudio-queue-inspector"
  });
  const selectedGuild = model.guilds.find(
    (guild) => guild.id === model.selectedGuildId
  );
  const searchSuggestions = [
    ...(model.snapshot.track ? [model.snapshot.track] : []),
    ...model.snapshot.queue
  ].map((track) => ({
    id: track.id,
    title: track.title,
    artist: track.artist ?? null
  }));
  const inspectorPanel = inspectorPanelSizing(
    inspectorSize.isCollapsed,
    inspectorSize.props
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
      connected={selectedGuild?.active === true && model.connected}
      commandPending={model.commandPending}
      searchSuggestions={searchSuggestions}
      onEnqueue={model.enqueueMedia}
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
        content={
          <LayoutContent padding={0} isScrollable={!usesSideInspector}>
            <VStack gap={0} {...(usesSideInspector ? { height: "100%" } : {})}>
              {usesSideInspector ? (
                <StackItem size="fill" isScrollable>
                  <NowPlaying guild={selectedGuild} snapshot={model.snapshot} />
                </StackItem>
              ) : (
                <NowPlaying guild={selectedGuild} snapshot={model.snapshot} />
              )}
              <StackItem size="static">
                <Section variant="transparent" padding={6} dividers={["top"]}>
                  <PlayerBar model={model} />
                </Section>
              </StackItem>
              {!usesSideInspector ? (
                <Section
                  variant="transparent"
                  padding={4}
                  dividers={["top"]}
                  role="complementary"
                  aria-label="検索、キューと360° Audioの設定"
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
              {!inspectorSize.isCollapsed ? (
                <ResizeHandle
                  resizable={inspectorSize.props}
                  isReversed
                  hasDivider
                  isAlwaysVisible={false}
                  label="キューパネルの幅を調整"
                />
              ) : null}
              <LayoutPanel
                {...inspectorPanel}
                padding={0}
                isScrollable={false}
                role="complementary"
                label="検索、キューと360° Audioの設定"
              >
                {inspectorSize.isCollapsed ? (
                  <Center width="100%" height="100%">
                    <IconButton
                      label="キューパネルを開く"
                      tooltip="キューを開く"
                      icon={<Icon icon={PanelRightOpen} />}
                      variant="ghost"
                      onClick={inspectorSize.expand}
                    />
                  </Center>
                ) : inspector}
              </LayoutPanel>
            </>
          ) : undefined
        }
      />
    </AppShell>
  );
}
