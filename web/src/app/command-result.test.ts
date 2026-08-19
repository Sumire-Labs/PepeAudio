import { describe, expect, it } from "vitest";

import {
  commandFailureMessage,
  parseCommandReceipt,
  parseCommandResult
} from "./command-result";

const COMMAND_ID = "00000000-0000-0000-0000-000000000001";
const IDEMPOTENCY_KEY = "00000000-0000-0000-0000-000000000002";
const GUILD_ID = "18446744073709551615";

describe("command result wire parsing", () => {
  it("keeps the receipt command ID needed for correlation", () => {
    expect(parseCommandReceipt({
      command_id: COMMAND_ID,
      idempotency_key: IDEMPOTENCY_KEY,
      resulting_revision: null,
      replayed: false
    })).toEqual({
      command_id: COMMAND_ID,
      idempotency_key: IDEMPOTENCY_KEY,
      resulting_revision: null,
      replayed: false
    });
  });

  it("accepts pending and terminal results only for the expected guild and command", () => {
    expect(parseCommandResult({
      command_id: COMMAND_ID,
      guild_id: GUILD_ID,
      status: "pending"
    }, GUILD_ID, COMMAND_ID).status).toBe("pending");

    expect(parseCommandResult({
      command_id: COMMAND_ID,
      guild_id: GUILD_ID,
      status: "applied",
      resulting_revision: 12
    }, GUILD_ID, COMMAND_ID)).toMatchObject({
      status: "applied",
      resulting_revision: 12
    });

    expect(() => parseCommandResult({
      command_id: IDEMPOTENCY_KEY,
      guild_id: GUILD_ID,
      status: "pending"
    }, GUILD_ID, COMMAND_ID)).toThrow("command ID mismatch");
    expect(() => parseCommandResult({
      command_id: COMMAND_ID,
      guild_id: "1",
      status: "pending"
    }, GUILD_ID, COMMAND_ID)).toThrow("guild mismatch");
  });

  it("rejects unknown backend codes instead of displaying backend text", () => {
    expect(() => parseCommandResult({
      command_id: COMMAND_ID,
      guild_id: GUILD_ID,
      status: "rejected",
      code: "database_password_was_wrong",
      message: "private adapter detail"
    }, GUILD_ID, COMMAND_ID)).toThrow("code is invalid");

    const denied = parseCommandResult({
      command_id: COMMAND_ID,
      guild_id: GUILD_ID,
      status: "denied",
      code: "not_authorized"
    }, GUILD_ID, COMMAND_ID);
    if (denied.status === "pending") throw new Error("expected terminal result");
    expect(commandFailureMessage(denied)).toBe(
      "このサーバーのプレイヤーを操作する権限がありません。"
    );
  });
});
