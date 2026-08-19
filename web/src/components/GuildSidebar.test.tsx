import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { demoGuilds } from "../app/demo-data";
import { GuildSidebar } from "./GuildSidebar";

afterEach(cleanup);

describe("GuildSidebar accessible names", () => {
  it("renders a guided empty state when the account has no available guilds", () => {
    render(
      <GuildSidebar
        guilds={[]}
        selectedGuildId=""
        account={null}
        status="unauthenticated"
        onSelect={vi.fn()}
        onLogout={null}
        loggingOut={false}
      />
    );

    expect(screen.getByRole("heading", {
      name: "利用できるサーバーがありません"
    })).toBeTruthy();
    expect(
      screen.getByText("DiscordでBotを追加すると、ここから再生状態を確認できます。")
    ).toBeTruthy();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(screen.queryByText("リアルタイム同期")).toBeNull();
  });

  it("keeps each guild name and status on its button when visible copy is collapsed", () => {
    const guild = demoGuilds[0];
    if (!guild) throw new Error("demo guild fixture is missing");

    render(
      <GuildSidebar
        guilds={[guild]}
        selectedGuildId={guild.id}
        account={null}
        status="ready"
        onSelect={vi.fn()}
        onLogout={null}
        loggingOut={false}
      />
    );

    const button = screen.getByRole("button", {
      name: new RegExp(`^${guild.name}.*${guild.listenerCount}人が参加中$`, "u")
    });
    expect(button).toBeTruthy();
  });

  it("routes selection through the SideNav item and blocks it while a command is pending", () => {
    const guild = demoGuilds[0];
    if (!guild) throw new Error("demo guild fixture is missing");
    const onSelect = vi.fn();
    const { rerender } = render(
      <GuildSidebar
        guilds={[guild]}
        selectedGuildId="another-guild"
        account={null}
        status="ready"
        onSelect={onSelect}
        onLogout={null}
        loggingOut={false}
      />
    );

    const guildButton = screen.getByRole("button", { name: /Sumire Listening Room/u });
    fireEvent.click(guildButton);
    expect(onSelect).toHaveBeenCalledWith(guild.id);

    onSelect.mockClear();
    rerender(
      <GuildSidebar
        guilds={[guild]}
        selectedGuildId="another-guild"
        account={null}
        status="ready"
        commandPending
        onSelect={onSelect}
        onLogout={null}
        loggingOut={false}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: /Sumire Listening Room/u }));
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("shows a validated Discord icon, falls back to initials, and offers logout", () => {
    const onLogout = vi.fn();
    const fixture = demoGuilds[0];
    if (!fixture) throw new Error("demo guild fixture is missing");
    const guild = {
      ...fixture,
      iconUrl: "https://cdn.discordapp.com/icons/1/safe_hash.webp?size=64"
    };
    const { container } = render(
      <GuildSidebar
        guilds={[guild]}
        selectedGuildId={guild.id}
        account={{
          source: "discord",
          userId: "123",
          username: "listener",
          displayName: "Listener",
          avatarUrl: null
        }}
        status="ready"
        onSelect={vi.fn()}
        onLogout={onLogout}
        loggingOut={false}
      />
    );

    const image = container.querySelector(
      `img[src*="cdn.discordapp.com/icons/1/safe_hash"]`
    );
    if (!(image instanceof HTMLImageElement)) throw new Error("guild icon is missing");
    expect(image.src).toContain("cdn.discordapp.com/icons/1/safe_hash.webp?size=64");
    fireEvent.error(image);
    expect(screen.getByText(guild.initials)).toBeTruthy();

    screen.getByRole("button", { name: "PepeAudioからログアウト" }).click();
    expect(onLogout).toHaveBeenCalledOnce();
  });
});
