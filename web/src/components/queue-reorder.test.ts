import { describe, expect, it } from "vitest";

import { queueMoveForDrop } from "./queue-reorder";

const queue = ["first", "second", "third", "fourth"] as const;

describe("queueMoveForDrop", () => {
  it("moves an item upward before the item at the drop position", () => {
    expect(queueMoveForDrop(queue, "fourth", "second")).toEqual({
      trackId: "fourth",
      beforeTrackId: "second"
    });
  });

  it("uses the item after the drop position for a downward move", () => {
    expect(queueMoveForDrop(queue, "first", "third")).toEqual({
      trackId: "first",
      beforeTrackId: "fourth"
    });
  });

  it("uses null when the dropped item becomes the queue tail", () => {
    expect(queueMoveForDrop(queue, "second", "fourth")).toEqual({
      trackId: "second",
      beforeTrackId: null
    });
  });

  it("does not submit a same-position or stale-identity move", () => {
    expect(queueMoveForDrop(queue, "second", "second")).toBeNull();
    expect(queueMoveForDrop(queue, "missing", "second")).toBeNull();
    expect(queueMoveForDrop(queue, "second", "missing")).toBeNull();
  });
});
