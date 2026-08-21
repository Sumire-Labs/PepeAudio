import { Card } from "@astryxdesign/core/Card";
import { Grid, GridSpan } from "@astryxdesign/core/Grid";
import { Section } from "@astryxdesign/core/Section";
import { StackItem, VStack } from "@astryxdesign/core/Stack";

import type {
  HrirCatalogStatus,
  HrirPreset,
  PlayerSnapshot,
  QueueItem
} from "../app/types";
import { dashboardInspectorStyles } from "./dashboard-inspector.styles";
import { MediaSearchBar, type MediaSearchSeed } from "./MediaSearchBar";
import { QueuePanel } from "./QueuePanel";
import { SpatialPanel } from "./SpatialPanel";

interface DashboardInspectorProps {
  readonly presentation: "panel" | "content";
  readonly queue: readonly QueueItem[];
  readonly presets: readonly HrirPreset[];
  readonly catalogStatus: HrirCatalogStatus;
  readonly selectedPresetId: string | null;
  readonly snapshot: PlayerSnapshot;
  readonly connected: boolean;
  readonly commandPending: boolean;
  readonly searchSuggestions: readonly MediaSearchSeed[];
  readonly onEnqueue: (input: string) => Promise<void> | void;
  readonly onRemove: (trackId: string) => void;
  readonly onMove: (trackId: string, beforeTrackId: string | null) => void;
  readonly onPresetChange: (presetId: string) => void;
  readonly onSpatialToggle: () => void;
  readonly onCollapse?: (() => void) | undefined;
}

export function DashboardInspector({
  presentation,
  queue,
  presets,
  catalogStatus,
  selectedPresetId,
  snapshot,
  connected,
  commandPending,
  searchSuggestions,
  onEnqueue,
  onRemove,
  onMove,
  onPresetChange,
  onSpatialToggle,
  onCollapse
}: DashboardInspectorProps) {
  const mediaSearch = (
    <MediaSearchBar
      isDisabled={!connected}
      isLoading={commandPending}
      suggestions={searchSuggestions}
      onSubmit={onEnqueue}
    />
  );
  const queuePanel = (
    <QueuePanel
      queue={queue}
      commandPending={commandPending}
      onRemove={onRemove}
      onMove={onMove}
      onCollapse={presentation === "panel" ? onCollapse : undefined}
    />
  );
  const spatialPanel = (
    <SpatialPanel
      presets={presets}
      catalogStatus={catalogStatus}
      selectedPresetId={selectedPresetId}
      snapshot={snapshot}
      connected={connected}
      commandPending={commandPending}
      onPresetChange={onPresetChange}
      onToggle={onSpatialToggle}
    />
  );

  if (presentation === "content") {
    return (
      <Grid columns={{ minWidth: 260, max: 2, repeat: "fill" }} gap={4} align="start">
        <GridSpan columns="full">
          <Card padding={4} xstyle={dashboardInspectorStyles.card}>
            {mediaSearch}
          </Card>
        </GridSpan>
        <GridSpan columns="full">
          <Card minHeight={320} padding={4} xstyle={dashboardInspectorStyles.card}>
            {queuePanel}
          </Card>
        </GridSpan>
        <GridSpan columns="full">
          <Card variant="muted" padding={4} xstyle={dashboardInspectorStyles.card}>
            {spatialPanel}
          </Card>
        </GridSpan>
      </Grid>
    );
  }

  return (
    <VStack gap={0} height="100%">
      <StackItem size="static">
        <Section variant="transparent" padding={4} dividers={["bottom"]}>
          {mediaSearch}
        </Section>
      </StackItem>
      <StackItem
        size="fill"
        isScrollable
        role="region"
        aria-label="次に再生キュー"
      >
        <Section
          variant="transparent"
          padding={4}
          height="100%"
          dividers={["bottom"]}
        >
          {queuePanel}
        </Section>
      </StackItem>
      <StackItem size="static">
        <Section variant="transparent" padding={4}>
          {spatialPanel}
        </Section>
      </StackItem>
    </VStack>
  );
}
