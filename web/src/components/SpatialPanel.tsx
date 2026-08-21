import { Icon } from "@astryxdesign/core/Icon";
import { Item } from "@astryxdesign/core/Item";
import { Link } from "@astryxdesign/core/Link";
import { Selector, SelectorOption } from "@astryxdesign/core/Selector";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Switch } from "@astryxdesign/core/Switch";
import { Heading, Text } from "@astryxdesign/core/Text";
import { Orbit, Sparkles } from "lucide-react";

import type { HrirCatalogStatus, HrirPreset, PlayerSnapshot } from "../app/types";

const SPATIAL_OFF_VALUE = "__pepeaudio_spatial_off__";

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
  const selectionValue = enabled ? (selected?.id ?? SPATIAL_OFF_VALUE) : SPATIAL_OFF_VALUE;
  const catalogMessage = describeCatalog(
    catalogStatus,
    presets.length,
    enabled ? selected : undefined
  );
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
        label={enabled ? "前方固定でHRIRを適用中" : "空間処理はオフ"}
        description="音源を移動させず、HeSuViの前方ステレオ音場を適用"
        endContent={
          <Text type="code" color="secondary">
            {enabled ? "前方固定" : "オフ"}
          </Text>
        }
      />

      <Selector
        label="HRIRプリセット"
        options={[
          { value: SPATIAL_OFF_VALUE, label: "オフ", icon: Orbit },
          ...presets.map((preset) => ({
            value: preset.id,
            label: preset.name,
            icon: Sparkles
          }))
        ]}
        value={selectionValue}
        placeholder={catalogOption(catalogStatus, presets.length)}
        hasSearch={presets.length > 8}
        searchPlaceholder="HRIRプリセットを検索…"
        renderOption={(option) => (
          <SelectorOption
            icon={option.icon}
            label={option.label ?? option.value}
            description={
              option.value === SPATIAL_OFF_VALUE
                ? "HRIR空間処理を無効にします"
                : presets.find((preset) => preset.id === option.value)?.description
            }
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
          if (presetId === SPATIAL_OFF_VALUE) {
            if (enabled) onToggle();
          } else if (presetId) {
            onPresetChange(presetId);
          }
        }}
      />

      {enabled && selected?.description ? (
        <Text type="body" color="primary">
          {selected.description}
        </Text>
      ) : null}
      {catalogMessage ? (
        <Text type="supporting" color="secondary">
          {catalogMessage}
        </Text>
      ) : null}
      {enabled && selected?.source.sourceUrl ? (
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
