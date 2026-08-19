import { AppShell } from "@astryxdesign/core/AppShell";
import {
  Layout,
  LayoutContent,
  LayoutFooter,
  LayoutPanel
} from "@astryxdesign/core/Layout";
import { Section } from "@astryxdesign/core/Section";
import { VStack } from "@astryxdesign/core/Stack";
import { useToast } from "@astryxdesign/core/Toast";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { useEffect } from "react";

import { ConnectionBanner } from "../components/ConnectionBanner";
import { DashboardInspector } from "../components/DashboardInspector";
import { GuildSidebar } from "../components/GuildSidebar";
import { NowPlaying } from "../components/NowPlaying";
import { PlayerBar } from "../components/PlayerBar";
import { useDashboard } from "./use-dashboard";

export function App() {
  const session = useDashboard();
  const showToast = useToast();
  const { model } = session;
  const usesSideInspector = useMediaQuery("(min-width: 1025px)");
  const selectedGuild = model.guilds.find(
    (guild) => guild.id === model.selectedGuildId
  );
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
    />
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
            <LayoutPanel
              width={380}
              padding={0}
              hasDivider
              role="complementary"
              label="キューと360° Audioの設定"
            >
              {inspector}
            </LayoutPanel>
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
