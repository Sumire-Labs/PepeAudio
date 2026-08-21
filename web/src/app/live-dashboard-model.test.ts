import { describe, expect, it, vi } from "vitest";

import type { AuthGuild } from "./auth-wire";
import { createDemoSnapshot } from "./demo-data";
import { buildLiveDashboardModel, selectInitialGuild } from "./live-dashboard-model";

const GUILD_ID = "120000000000000001";
const guild: AuthGuild = {
  id: GUILD_ID,
  name: "Listening Room",
  icon: null,
  owner: true,
  permissions: "8",
  botPresent: true
};

describe("buildLiveDashboardModel", () => {
  it("routes queue moves with stable source and destination identities", () => {
    const run = vi.fn(async () => undefined);
    const model = buildLiveDashboardModel({
      guilds: [guild],
      selectedGuildId: GUILD_ID,
      snapshot: createDemoSnapshot(GUILD_ID),
      presets: [],
      catalogStatus: "ready",
      commandPending: false,
      run,
      selectGuild: vi.fn()
    });

    model.moveQueued("queue-3", "queue-1");
    expect(run).toHaveBeenCalledWith({
      type: "move_queued",
      track_id: "queue-3",
      before_track_id: "queue-1"
    });

    model.moveQueued("queue-1", null);
    expect(run).toHaveBeenLastCalledWith({
      type: "move_queued",
      track_id: "queue-1",
      before_track_id: null
    });
  });
});

describe("selectInitialGuild", () => {
  const inactiveGuild: AuthGuild = {
    ...guild,
    id: "120000000000000002",
    name: "Bot not installed",
    botPresent: false
  };

  it("keeps the current guild only while the bot is present", () => {
    expect(selectInitialGuild(GUILD_ID, [inactiveGuild, guild])).toBe(GUILD_ID);
    expect(selectInitialGuild(inactiveGuild.id, [inactiveGuild, guild])).toBe(GUILD_ID);
  });

  it("does not open a player connection when the bot is absent from every guild", () => {
    expect(selectInitialGuild(inactiveGuild.id, [inactiveGuild])).toBe("");
  });
});
