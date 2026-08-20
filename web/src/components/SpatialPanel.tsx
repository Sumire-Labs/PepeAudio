import { Icon } from "@astryxdesign/core/Icon";
import { Item } from "@astryxdesign/core/Item";
import { Link } from "@astryxdesign/core/Link";
import { Selector, SelectorOption } from "@astryxdesign/core/Selector";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Switch } from "@astryxdesign/core/Switch";
import { Heading, Text } from "@astryxdesign/core/Text";
import { Orbit, Sparkles } from "lucide-react";

import { orbitDegreesAt } from "../app/progress";
import type { HrirCatalogStatus, HrirPreset, PlayerSnapshot } from "../app/types";
import { useClock } from "../app/use-clock";

interface SpatialPanelProps {
  readonly presets: readonly HrirPreset[];
  readonly catalogStatus: HrirCatalogStatus;
  readonly selectedPresetId: string | null;
  readonly snapshot: PlayerSnapshot;
  readonly connected: boolean;
  readonly commandPending: boolean;
  readonly onPresetChange: (presetId: string) => void;
  readonly onToggle: () => void;
}

export function SpatialPanel({
  presets,
  catalogStatus,
  selectedPresetId,
  snapshot,
  connected,
  commandPending,
  onPresetChange,
  onToggle
}: SpatialPanelProps) {
  const enabled = snapshot.spatialEnabled;
  const selected = presets.find((preset) => preset.id === selectedPresetId);
  const selectionValue = selected?.id ?? "";
  const catalogMessage = describeCatalog(catalogStatus, presets.length, selected);
  const unavailable = catalogStatus !== "ready" || presets.length === 0;
  const controlsUnavailable = !connected || commandPending;
  const controlMessage = connected
    ? "別のプレイヤー操作を反映しています。"
    : "プレイヤー状態の同期後に操作できます。";

  return (
    <VStack gap={3}>
      <HStack gap={3} vAlign="center" hAlign="between">
        <HStack gap={2} vAlign="center">
          <Icon icon={Orbit} color="secondary" />
          <Heading level={2}>360° Audio</Heading>
        </HStack>
        <Switch
          label="360° Audio"
          value={enabled}
          isLabelHidden
          isDisabled={controlsUnavailable}
          isLoading={commandPending}
          disabledMessage={controlMessage}
          onChange={() => onToggle()}
        />
      </HStack>

      <Item
        density="compact"
        startContent={<Icon icon={Orbit} color="secondary" />}
        label={enabled ? "水平音場を適用中" : "空間処理はオフ"}
        description="HeSuViの水平7方向をサーバー全体へ適用"
        endContent={<OrbitReadout snapshot={snapshot} />}
      />

      <Selector
        label="HRIRプリセット"
        options={presets.map((preset) => ({
          value: preset.id,
          label: preset.name,
          icon: Sparkles
        }))}
        value={selectionValue}
        placeholder={catalogOption(catalogStatus, presets.length)}
        hasSearch={presets.length > 8}
        searchPlaceholder="HRIRプリセットを検索…"
        renderOption={(option) => (
          <SelectorOption
            icon={option.icon}
            label={option.label ?? option.value}
            description={presets.find((preset) => preset.id === option.value)?.description}
          />
        )}
        width="100%"
        isDisabled={controlsUnavailable || unavailable}
        disabledMessage={selectorDisabledMessage(
          connected,
          commandPending,
          catalogStatus,
          presets.length
        )}
        onChange={(presetId) => {
          if (presetId) onPresetChange(presetId);
        }}
      />

      {selected?.description ? (
        <Text type="body" color="primary">
          {selected.description}
        </Text>
      ) : null}
      {catalogMessage ? (
        <Text type="supporting" color="secondary">
          {catalogMessage}
        </Text>
      ) : null}
      {selected?.source.sourceUrl ? (
        <Link
          href={selected.source.sourceUrl}
          isExternalLink
          isStandalone
          newTabLabel="（新しいタブで開きます）"
        >
          プリセットの出典を見る
        </Link>
      ) : null}
    </VStack>
  );
}

function OrbitReadout({ snapshot }: { readonly snapshot: PlayerSnapshot }) {
  const nowUnixMs = useClock();
  return (
    <Text
      type="code"
      color="secondary"
      hasTabularNumbers
      aria-label="現在の水平音源方向"
    >
      {Math.round(orbitDegreesAt(snapshot, nowUnixMs))}°
    </Text>
  );
}

function catalogOption(status: HrirCatalogStatus, count: number): string {
  if (status === "loading") return "読み込み中…";
  if (status === "unavailable") return "カタログを取得できません";
  return count === 0 ? "利用可能なプリセットなし" : "プリセットを選択";
}

function selectorDisabledMessage(
  connected: boolean,
  commandPending: boolean,
  status: HrirCatalogStatus,
  count: number
): string {
  if (!connected) return "プレイヤー状態の同期後に選択できます。";
  if (commandPending) return "別のプレイヤー操作を反映しています。";
  if (status === "loading") return "読み込み中のため選択できません。";
  if (status === "unavailable") return "カタログ取得失敗のため選択できません。";
  if (count === 0) return "利用可能なプリセットがないため選択できません。";
  return "現在は選択できません。";
}

function describeCatalog(
  status: HrirCatalogStatus,
  count: number,
  selected: HrirPreset | undefined
): string | null {
  if (status === "loading") return "HRIRプリセットを読み込んでいます。";
  if (status === "unavailable") return "HRIRカタログを取得できませんでした。";
  if (count === 0) return "このサーバーで利用可能なプリセットはありません。";
  if (!selected) return "現在のプリセットはカタログにありません。";
  const details = [selected.source.attribution, selected.source.licenseName].filter(Boolean);
  return details.length > 0 ? details.join(" · ") : null;
}
