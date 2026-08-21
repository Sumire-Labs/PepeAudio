import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConnectionBanner } from "./ConnectionBanner";

afterEach(cleanup);

describe("ConnectionBanner actions", () => {
  it("stays out of the layout when the live connection is ready", () => {
    const { container } = render(
      <ConnectionBanner
        status="ready"
        demo={false}
        message={null}
        onRetry={vi.fn()}
        onLogin={vi.fn()}
      />
    );

    expect(container.childElementCount).toBe(0);
  });

  it("offers the action that matches the current failure state", () => {
    const onLogin = vi.fn();
    const onRetry = vi.fn();
    const { rerender } = render(
      <ConnectionBanner
        status="unauthenticated"
        demo={false}
        message={null}
        onRetry={onRetry}
        onLogin={onLogin}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Discordでログイン" }));
    expect(onLogin).toHaveBeenCalledOnce();

    rerender(
      <ConnectionBanner
        status="unavailable"
        demo={false}
        message="接続を確認してください。"
        onRetry={onRetry}
        onLogin={onLogin}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: "再試行" }));
    expect(onRetry).toHaveBeenCalledOnce();
    expect(screen.getByText("接続を確認してください。")).toBeTruthy();
  });

  it("keeps a recovered snapshot visible while the live stream reconnects", () => {
    const onRetry = vi.fn();
    render(
      <ConnectionBanner
        status="reconnecting"
        demo={false}
        message="リアルタイム接続を再接続しています（3秒以内）。"
        onRetry={onRetry}
        onLogin={vi.fn()}
      />
    );

    expect(screen.getByText("リアルタイム接続を復旧しています")).toBeTruthy();
    expect(screen.queryByText("接続できませんでした")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "再試行" }));
    expect(onRetry).toHaveBeenCalledOnce();
  });
});
