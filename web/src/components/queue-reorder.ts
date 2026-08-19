import { arrayMove } from "@dnd-kit/sortable";

export interface QueueMove {
  readonly trackId: string;
  readonly beforeTrackId: string | null;
}

/**
 * Converts dnd-kit's destination index into the stable before-track contract
 * understood by the player actor.
 */
export function queueMoveForDrop(
  orderedTrackIds: readonly string[],
  activeTrackId: string,
  overTrackId: string
): QueueMove | null {
  const sourceIndex = orderedTrackIds.indexOf(activeTrackId);
  const destinationIndex = orderedTrackIds.indexOf(overTrackId);
  if (
    sourceIndex < 0
    || destinationIndex < 0
    || sourceIndex === destinationIndex
  ) {
    return null;
  }

  const reordered = arrayMove([...orderedTrackIds], sourceIndex, destinationIndex);
  return {
    trackId: activeTrackId,
    beforeTrackId: reordered[destinationIndex + 1] ?? null
  };
}
