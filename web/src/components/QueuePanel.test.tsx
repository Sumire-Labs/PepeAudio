import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { QueueItem } from "../app/types";
import { QueuePanel } from "./QueuePanel";

afterEach(cleanup);

const queue: readonly QueueItem[] = [
  {
    id: "84cb4cf6-7e0a-4c5e-b44b-cb8d8df5d37d",
    title: "Signals After Rain",
    artist: "Aster Vale",
    requestedBy: "Nox",
    durationMs: 221_000
  },
  {
    id: "f6371e8d-48f8-4a03-af5d-68c6db889770",
    title: "Soft Relay",
    artist: "Cassette Flora",
    requestedBy: "miso",
    durationMs: 194_000
  }
];

describe("QueuePanel queue actions", () => {
  it("gives the primary empty queue enough room to remain visually dominant", () => {
    const { container } = render(
      <QueuePanel
        queue={[]}
        commandPending={false}
        onRemove={vi.fn()}
        onMove={vi.fn()}
      />
    );

    expect(screen.getByRole("heading", { name: "次に再生" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "キューは空です" })).toBeTruthy();
    expect(container.querySelector(".astryx-center")).toBeTruthy();
  });

  it("submits the selected upcoming track identity", async () => {
    const onRemove = vi.fn();
    render(
      <QueuePanel
        queue={queue}
        commandPending={false}
        onRemove={onRemove}
        onMove={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "「Soft Relay」のキュー操作" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "キューから削除" }));

    expect(onRemove).toHaveBeenCalledOnce();
    expect(onRemove).toHaveBeenCalledWith(queue[1]?.id);
    expect(screen.getByRole("list").getAttribute("aria-labelledby")).toBeTruthy();
    expect(screen.queryByRole("toolbar")).toBeNull();
  });

  it("disables every queue action while another command is pending", () => {
    const onRemove = vi.fn();
    render(
      <QueuePanel
        queue={queue}
        commandPending
        onRemove={onRemove}
        onMove={vi.fn()}
      />
    );

    for (const button of screen.getAllByRole("button", { name: /のキュー操作$/u })) {
      expect(button.getAttribute("aria-disabled")).toBe("true");
      fireEvent.click(button);
    }
    for (const button of screen.getAllByRole("button", { name: /を並べ替え$/u })) {
      expect(button.getAttribute("aria-disabled")).toBe("true");
    }
    expect(onRemove).not.toHaveBeenCalled();
  });

  it("omits metadata that is not available in the player snapshot", () => {
    render(
      <QueuePanel
        queue={[{ id: "track", title: "Known title", durationMs: null }]}
        commandPending={false}
        onRemove={vi.fn()}
        onMove={vi.fn()}
      />
    );

    expect(screen.getByText("Known title")).toBeTruthy();
    expect(screen.getByText("1曲")).toBeTruthy();
    expect(screen.queryByText("Discord audio")).toBeNull();
    expect(screen.queryByText("System")).toBeNull();
  });

  it("links the queued title to its safe playback page", () => {
    render(
      <QueuePanel
        queue={[{
          id: "track",
          title: "Known title",
          durationMs: null,
          provenance: {
            origin: null,
            playback: {
              provider: "soundcloud",
              url: "https://soundcloud.com/example/example-track"
            }
          }
        }]}
        commandPending={false}
        onRemove={vi.fn()}
        onMove={vi.fn()}
      />
    );

    expect(
      screen.getByRole("link", { name: /Known title/u }).getAttribute("target")
    ).toBe("_blank");
    expect(screen.queryByText(/SoundCloudで再生/u)).toBeNull();
  });

  it("moves tracks using stable before-track identities", async () => {
    const third: QueueItem = {
      id: "baa02968-c45e-4f79-82a7-02c958d68346",
      title: "Neon Orchard",
      durationMs: 182_000
    };
    const onMove = vi.fn();
    render(
      <QueuePanel
        queue={[...queue, third]}
        commandPending={false}
        onRemove={vi.fn()}
        onMove={onMove}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "「Soft Relay」のキュー操作" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "1つ上へ移動" }));
    expect(onMove).toHaveBeenLastCalledWith(queue[1]?.id, queue[0]?.id);

    fireEvent.click(screen.getByRole("button", { name: "「Signals After Rain」のキュー操作" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "1つ下へ移動" }));
    expect(onMove).toHaveBeenLastCalledWith(queue[0]?.id, third.id);

    fireEvent.click(screen.getByRole("button", { name: "「Neon Orchard」のキュー操作" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "1つ上へ移動" }));
    expect(onMove).toHaveBeenLastCalledWith(third.id, queue[1]?.id);
  });

  it("provides a focusable keyboard drag handle for every queue item", () => {
    render(
      <QueuePanel
        queue={queue}
        commandPending={false}
        onRemove={vi.fn()}
        onMove={vi.fn()}
      />
    );

    const handles = screen.getAllByRole("button", { name: /を並べ替え$/u });
    expect(handles).toHaveLength(2);
    expect(handles[0]?.getAttribute("aria-roledescription")).toBe("並べ替え可能な曲");
    expect(handles[0]?.getAttribute("aria-describedby")).toBeTruthy();
  });

  it("reorders with Space and the arrow keys through the same stable-ID action", async () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement
    ) {
      const trackId = this.dataset.queueTrackId;
      const top = trackId === queue[0]?.id ? 0 : trackId === queue[1]?.id ? 80 : 0;
      return rectangle(top);
    });
    const onMove = vi.fn();
    render(
      <QueuePanel
        queue={queue}
        commandPending={false}
        onRemove={vi.fn()}
        onMove={onMove}
      />
    );

    const handle = screen.getByRole("button", { name: "「Soft Relay」を並べ替え" });
    handle.focus();
    fireEvent.keyDown(handle, { key: " ", code: "Space" });
    await new Promise((resolve) => window.setTimeout(resolve, 0));
    fireEvent.keyDown(document, { key: "ArrowUp", code: "ArrowUp" });
    fireEvent.keyDown(document, { key: " ", code: "Space" });

    expect(onMove).toHaveBeenCalledOnce();
    expect(onMove).toHaveBeenCalledWith(queue[1]?.id, queue[0]?.id);
  });

  it("disables impossible boundary moves and the trigger while pending", async () => {
    const { rerender } = render(
      <QueuePanel
        queue={queue}
        commandPending={false}
        onRemove={vi.fn()}
        onMove={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "「Signals After Rain」のキュー操作" }));
    expect((await screen.findByRole("menuitem", { name: "1つ上へ移動" }))
      .getAttribute("aria-disabled")).toBe("true");
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

    fireEvent.click(screen.getByRole("button", { name: "「Soft Relay」のキュー操作" }));
    expect((await screen.findByRole("menuitem", { name: "1つ下へ移動" }))
      .getAttribute("aria-disabled")).toBe("true");
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

    rerender(
      <QueuePanel
        queue={queue}
        commandPending
        onRemove={vi.fn()}
        onMove={vi.fn()}
      />
    );
    for (const button of screen.getAllByRole("button", { name: /のキュー操作$/u })) {
      expect(button.getAttribute("aria-disabled")).toBe("true");
    }
  });
});

function rectangle(top: number): DOMRect {
  return {
    x: 0,
    y: top,
    top,
    left: 0,
    right: 400,
    bottom: top + 64,
    width: 400,
    height: 64,
    toJSON: () => ({})
  } as DOMRect;
}
