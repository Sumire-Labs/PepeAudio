import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { ListItem } from "@astryxdesign/core/List";
import { MoreMenu } from "@astryxdesign/core/MoreMenu";
import { HStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Clock3, GripVertical } from "lucide-react";

import { formatDuration } from "../app/progress";
import type { QueueItem } from "../app/types";
import { queuePanelStyles } from "./queue-panel.styles";
import { TrackSourceLinks } from "./TrackSourceLink";

interface SortableQueueItemProps {
  readonly item: QueueItem;
  readonly index: number;
  readonly queueLength: number;
  readonly previousTrackId: string | null;
  readonly afterNextTrackId: string | null;
  readonly commandPending: boolean;
  readonly onRemove: (trackId: string) => void;
  readonly onMove: (trackId: string, beforeTrackId: string | null) => void;
}

export function SortableQueueItem({
  item,
  index,
  queueLength,
  previousTrackId,
  afterNextTrackId,
  commandPending,
  onRemove,
  onMove
}: SortableQueueItemProps) {
  const sortingDisabled = commandPending || queueLength < 2;
  const {
    attributes,
    isDragging,
    listeners,
    setActivatorNodeRef,
    setNodeRef,
    transform,
    transition
  } = useSortable({ id: item.id, disabled: sortingDisabled });
  const details = queueDetails(item);

  return (
    <ListItem
      ref={setNodeRef}
      data-queue-track-id={item.id}
      label={item.title}
      description={details.length > 0 ? details.join(" · ") : undefined}
      startContent={
        <IconButton
          ref={setActivatorNodeRef}
          icon={<Icon icon={GripVertical} size="sm" />}
          label={`「${item.title}」を並べ替え`}
          tooltip="ドラッグ、またはキーボードで並べ替え"
          variant="ghost"
          size="sm"
          isDisabled={sortingDisabled}
          xstyle={queuePanelStyles.dragHandle}
          {...attributes}
          {...listeners}
          aria-roledescription="並べ替え可能な曲"
        />
      }
      endContent={
        <HStack gap={2} vAlign="center">
          <TrackSourceLinks provenance={item.provenance} />
          <HStack gap={1} vAlign="center">
            <Icon icon={Clock3} size="xsm" color="secondary" />
            <Text type="code" color="secondary" hasTabularNumbers>
              {formatDuration(item.durationMs)}
            </Text>
          </HStack>
          <MoreMenu
            label={`「${item.title}」のキュー操作`}
            size="sm"
            isDisabled={commandPending}
            items={[
              {
                label: "1つ上へ移動",
                isDisabled: previousTrackId === null,
                onClick: () => previousTrackId && onMove(item.id, previousTrackId)
              },
              {
                label: "1つ下へ移動",
                isDisabled: index === queueLength - 1,
                onClick: () => onMove(item.id, afterNextTrackId)
              },
              { type: "divider" },
              {
                label: "キューから削除",
                onClick: () => onRemove(item.id)
              }
            ]}
          />
        </HStack>
      }
      style={{
        transform: CSS.Transform.toString(transform),
        transition
      }}
      xstyle={[
        queuePanelStyles.sortableItem,
        isDragging && queuePanelStyles.draggingItem
      ]}
    />
  );
}

function queueDetails(item: QueueItem): readonly string[] {
  return [
    item.artist,
    item.requestedBy ? `リクエスト: ${item.requestedBy}` : null
  ].filter((value): value is string => Boolean(value));
}
