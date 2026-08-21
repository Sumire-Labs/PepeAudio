import { Badge } from "@astryxdesign/core/Badge";
import { Center } from "@astryxdesign/core/Center";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { List } from "@astryxdesign/core/List";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading } from "@astryxdesign/core/Text";
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy
} from "@dnd-kit/sortable";
import { ListMusic, PanelRightClose } from "lucide-react";
import { memo, useMemo } from "react";

import type { QueueItem } from "../app/types";
import {
  queueAnnouncements,
  queueScreenReaderInstructions
} from "./queue-accessibility";
import { queueMoveForDrop } from "./queue-reorder";
import { SortableQueueItem } from "./SortableQueueItem";

interface QueuePanelProps {
  readonly queue: readonly QueueItem[];
  readonly commandPending: boolean;
  readonly onRemove: (trackId: string) => void;
  readonly onMove: (trackId: string, beforeTrackId: string | null) => void;
  readonly onCollapse?: (() => void) | undefined;
}

export const QueuePanel = memo(function QueuePanel({
  queue,
  commandPending,
  onRemove,
  onMove,
  onCollapse
}: QueuePanelProps) {
  const header = <QueueHeader count={queue.length} onCollapse={onCollapse} />;
  const orderedTrackIds = useMemo(() => queue.map((item) => item.id), [queue]);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );
  if (queue.length === 0) {
    return (
      <VStack gap={3}>
        {header}
        <Center minHeight={240}>
          <EmptyState
            headingLevel={3}
            isCompact
            title="キューは空です"
            description="上の検索欄またはDiscordの /play から追加できます。"
            icon={<Icon icon={ListMusic} />}
          />
        </Center>
      </VStack>
    );
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      accessibility={{
        announcements: queueAnnouncements(queue),
        screenReaderInstructions: queueScreenReaderInstructions
      }}
      onDragEnd={handleDragEnd}
    >
      <SortableContext
        items={orderedTrackIds}
        strategy={verticalListSortingStrategy}
      >
        <List header={header} density="balanced" hasDividers listStyle="decimal">
          {queue.map((item, index) => (
            <SortableQueueItem
              key={item.id}
              item={item}
              index={index}
              queueLength={queue.length}
              previousTrackId={queue[index - 1]?.id ?? null}
              afterNextTrackId={queue[index + 2]?.id ?? null}
              commandPending={commandPending}
              onRemove={onRemove}
              onMove={onMove}
            />
          ))}
        </List>
      </SortableContext>
    </DndContext>
  );

  function handleDragEnd({ active, over }: DragEndEvent) {
    if (commandPending || over === null) return;
    const move = queueMoveForDrop(
      orderedTrackIds,
      String(active.id),
      String(over.id)
    );
    if (move !== null) onMove(move.trackId, move.beforeTrackId);
  }
});

function QueueHeader({
  count,
  onCollapse
}: {
  readonly count: number;
  readonly onCollapse?: (() => void) | undefined;
}) {
  return (
    <HStack gap={3} vAlign="center" hAlign="between">
      <HStack gap={2} vAlign="center">
        <Icon icon={ListMusic} color="accent" />
        <Heading level={2}>次に再生</Heading>
      </HStack>
      <HStack gap={2} vAlign="center">
        <Badge variant="neutral" label={`${count}曲`} />
        {onCollapse ? (
          <IconButton
            label="キューパネルを閉じる"
            tooltip="キューを閉じる"
            icon={<Icon icon={PanelRightClose} />}
            variant="ghost"
            size="sm"
            onClick={onCollapse}
          />
        ) : null}
      </HStack>
    </HStack>
  );
}
