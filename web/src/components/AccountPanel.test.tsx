import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AccountPanel } from "./AccountPanel";

afterEach(cleanup);

describe("AccountPanel", () => {
  it("shows the projected Discord account without exposing its snowflake", () => {
    render(
      <AccountPanel
        account={{
          source: "discord",
          userId: "18446744073709551615",
          username: "pepe-listener",
          displayName: "Pepe Listener",
          avatarUrl: "https://cdn.discordapp.com/avatars/1/safe.webp?size=64"
        }}
        status="ready"
        onLogout={null}
        loggingOut={false}
      />
    );

    expect(screen.getByText("Pepe Listener")).toBeTruthy();
    expect(screen.getByText("@pepe-listener")).toBeTruthy();
    expect(screen.queryByText("18446744073709551615")).toBeNull();
    expect(screen.queryByText("リアルタイム同期")).toBeNull();
  });

  it("routes the single account action through the existing logout callback", () => {
    const onLogout = vi.fn();
    render(
      <AccountPanel
        account={{
          source: "discord",
          userId: "1",
          username: null,
          displayName: "Discordアカウント",
          avatarUrl: null
        }}
        status="ready"
        onLogout={onLogout}
        loggingOut={false}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "PepeAudioからログアウト" }));
    expect(onLogout).toHaveBeenCalledOnce();
  });

  it("uses an honest pending state before authentication completes", () => {
    render(
      <AccountPanel
        account={null}
        status="connecting"
        onLogout={null}
        loggingOut={false}
      />
    );
    expect(screen.getByText("確認しています…")).toBeTruthy();
  });
});
