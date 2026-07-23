// SPDX-License-Identifier: Apache-2.0
"use client";
import { useEffect, useState } from "react";
import {
  DndContext, PointerSensor, closestCenter, useSensor, useSensors, type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext, arrayMove, useSortable, verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { PlayerState, QueueEntry } from "@/lib/types";
import { formatTime } from "@/lib/format";
import { Play, Headphones, Grip, Close, Queue as QueueIcon } from "@/components/icons";

export function Queue({ state, reorder, remove }: {
  state: PlayerState;
  reorder: (from: number, to: number) => void;
  remove: (index: number) => void;
}) {
  const [items, setItems] = useState<QueueEntry[]>(state.queue);
  useEffect(() => setItems(state.queue), [state.queue]);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));

  const onDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return;
    const from = items.findIndex((q) => String(q.position) === e.active.id);
    const to = items.findIndex((q) => String(q.position) === e.over!.id);
    if (from < 0 || to < 0) return;
    setItems((prev) => arrayMove(prev, from, to)); // optimistic; server push reconciles
    reorder(from, to);
  };

  return (
    <aside className="glass flex w-80 shrink-0 flex-col rounded-2xl p-3">
      <h2 className="mb-3 flex items-center gap-2 px-1 text-sm font-semibold text-[var(--text-dim)]">
        <QueueIcon className="h-4 w-4" /> 次の曲 · {items.length}
      </h2>
      {items.length === 0 && (
        <p className="px-1 py-4 text-sm text-[var(--text-faint)]">次に再生する曲はありません。</p>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto soft-scroll pr-1">
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
          <SortableContext items={items.map((q) => String(q.position))} strategy={verticalListSortingStrategy}>
            <ul className="space-y-1">
              {items.map((q) => <Row key={q.position} entry={q} onRemove={() => remove(q.position)} />)}
            </ul>
          </SortableContext>
        </DndContext>
      </div>
    </aside>
  );
}

function Row({ entry, onRemove }: { entry: QueueEntry; onRemove: () => void }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } =
    useSortable({ id: String(entry.position) });
  const style = { transform: CSS.Transform.toString(transform), transition, opacity: isDragging ? 0.5 : 1 };
  const { track } = entry;

  return (
    <li ref={setNodeRef} style={style}
      className="group flex items-center gap-3 rounded-xl px-2 py-1.5 transition hover:bg-[var(--track-bg)]">
      <div className="relative h-10 w-10 flex-none">
        {track.thumbnailUrl ? (
          <img src={track.thumbnailUrl} alt="" className="h-10 w-10 rounded-md object-cover" />
        ) : (
          <div className="grid h-10 w-10 place-items-center rounded-md bg-[var(--track-bg)]">
            <Headphones className="h-5 w-5 text-[var(--text-faint)]" />
          </div>
        )}
        <div className="pointer-events-none absolute inset-0 grid place-items-center rounded-md bg-black/45 opacity-0 transition group-hover:opacity-100">
          <Play className="h-4 w-4 text-white" />
        </div>
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{track.title}</div>
        <div className="truncate text-xs text-[var(--text-dim)]">{track.artist}</div>
      </div>
      <span className="flex-none text-xs tabular-nums text-[var(--text-faint)]">
        {track.durationMs > 0 ? formatTime(track.durationMs) : ""}
      </span>
      <div className="flex flex-none items-center gap-0.5 opacity-0 transition group-hover:opacity-100">
        <button
          {...attributes}
          {...listeners}
          aria-label="ドラッグ"
          className="grid h-7 w-7 cursor-grab place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--hairline-strong)] hover:text-[var(--text)] active:cursor-grabbing"
        >
          <Grip className="h-4 w-4" />
        </button>
        <button
          type="button"
          onClick={onRemove}
          aria-label="削除"
          className="grid h-7 w-7 place-items-center rounded-full text-[var(--text-dim)] transition hover:bg-[var(--hairline-strong)] hover:text-[var(--text)]"
        >
          <Close className="h-4 w-4" />
        </button>
      </div>
    </li>
  );
}
