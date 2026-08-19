import type {
  Announcements,
  ScreenReaderInstructions,
  UniqueIdentifier
} from "@dnd-kit/core";

import type { QueueItem } from "../app/types";

export const queueScreenReaderInstructions: ScreenReaderInstructions = {
  draggable:
    "並べ替えるにはEnterまたはSpaceを押します。上下矢印で位置を動かし、もう一度EnterまたはSpaceで確定します。Escapeで取り消します。"
};

export function queueAnnouncements(queue: readonly QueueItem[]): Announcements {
  const describe = (id: UniqueIdentifier): string => {
    const index = queue.findIndex((item) => item.id === String(id));
    if (index < 0) return "不明な曲";
    return `「${queue[index]?.title ?? "名称未設定"}」、${queue.length}曲中${index + 1}番目`;
  };

  return {
    onDragStart: ({ active }) => `${describe(active.id)}を選択しました。`,
    onDragOver: ({ active, over }) => over === null
      ? `${describe(active.id)}はキューの外にあります。`
      : `${describe(active.id)}を${describe(over.id)}の位置へ移動します。`,
    onDragEnd: ({ active, over }) => over === null
      ? `${describe(active.id)}の並べ替えを取り消しました。`
      : active.id === over.id
        ? `${describe(active.id)}を元の位置に戻しました。`
        : `${describe(active.id)}を${describe(over.id)}の位置にドロップしました。反映を待っています。`,
    onDragCancel: ({ active }) => `${describe(active.id)}の並べ替えを取り消しました。`
  };
}
