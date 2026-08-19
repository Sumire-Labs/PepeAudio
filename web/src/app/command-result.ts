export interface CommandReceiptWire {
  readonly command_id: string;
  readonly idempotency_key: string;
  readonly resulting_revision: number | null;
  readonly replayed: boolean;
}

export type CommandResultWire =
  | {
      readonly command_id: string;
      readonly guild_id: string;
      readonly status: "pending";
    }
  | {
      readonly command_id: string;
      readonly guild_id: string;
      readonly status: "applied";
      readonly resulting_revision: number;
    }
  | {
      readonly command_id: string;
      readonly guild_id: string;
      readonly status: "denied";
      readonly code: CommandResultCode;
    }
  | {
      readonly command_id: string;
      readonly guild_id: string;
      readonly status: "rejected";
      readonly code: CommandResultCode;
      readonly current_revision?: number;
    };

export type TerminalCommandResultWire = Exclude<CommandResultWire, { readonly status: "pending" }>;

export type CommandResultCode =
  | "not_authorized"
  | "revision_conflict"
  | "deadline_expired"
  | "invalid_player_state"
  | "no_current_track"
  | "no_previous_track"
  | "queued_track_not_found"
  | "track_not_seekable"
  | "seek_past_end"
  | "not_connected"
  | "voice_channel_mismatch"
  | "queue_full"
  | "duplicate_track"
  | "state_exhausted"
  | "idempotency_replayed"
  | "result_expired";

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu;
const NIL_UUID = "00000000-0000-0000-0000-000000000000";
const SNOWFLAKE_PATTERN = /^(?:[1-9][0-9]{0,19})$/u;
const RESULT_CODES = new Set<CommandResultCode>([
  "not_authorized",
  "revision_conflict",
  "deadline_expired",
  "invalid_player_state",
  "no_current_track",
  "no_previous_track",
  "queued_track_not_found",
  "track_not_seekable",
  "seek_past_end",
  "not_connected",
  "voice_channel_mismatch",
  "queue_full",
  "duplicate_track",
  "state_exhausted",
  "idempotency_replayed",
  "result_expired"
]);

export function parseCommandReceipt(input: unknown): CommandReceiptWire {
  const value = record(input, "command receipt");
  return {
    command_id: uuid(value.command_id, "command ID"),
    idempotency_key: uuid(value.idempotency_key, "idempotency key"),
    resulting_revision:
      value.resulting_revision === null
        ? null
        : revision(value.resulting_revision, "resulting revision"),
    replayed: boolean(value.replayed, "replayed flag")
  };
}

export function parseCommandResult(
  input: unknown,
  expectedGuildId: string,
  expectedCommandId: string
): CommandResultWire {
  const value = record(input, "command result");
  const commandId = uuid(value.command_id, "command ID");
  const guildId = snowflake(value.guild_id, "guild ID");
  if (commandId.toLowerCase() !== expectedCommandId.toLowerCase()) {
    throw new Error("Command result command ID mismatch");
  }
  if (guildId !== expectedGuildId) throw new Error("Command result guild mismatch");

  switch (value.status) {
    case "pending":
      return { command_id: commandId, guild_id: guildId, status: "pending" };
    case "applied":
      return {
        command_id: commandId,
        guild_id: guildId,
        status: "applied",
        resulting_revision: revision(value.resulting_revision, "resulting revision")
      };
    case "denied":
      return {
        command_id: commandId,
        guild_id: guildId,
        status: "denied",
        code: resultCode(value.code)
      };
    case "rejected": {
      const currentRevision = value.current_revision;
      return {
        command_id: commandId,
        guild_id: guildId,
        status: "rejected",
        code: resultCode(value.code),
        ...(currentRevision === undefined
          ? {}
          : { current_revision: revision(currentRevision, "current revision") })
      };
    }
    default:
      throw new Error("Command result status is invalid");
  }
}

export function commandFailureMessage(result: TerminalCommandResultWire): string {
  if (result.status === "applied") return "";
  const messages: Readonly<Record<CommandResultCode, string>> = {
    not_authorized: "このサーバーのプレイヤーを操作する権限がありません。",
    revision_conflict: "プレイヤーの状態が変わりました。最新の状態でやり直してください。",
    deadline_expired: "操作の有効期限が切れました。もう一度お試しください。",
    invalid_player_state: "現在の再生状態ではこの操作を実行できません。",
    no_current_track: "現在再生中の曲がありません。",
    no_previous_track: "前の曲はありません。",
    queued_track_not_found: "対象の曲はキューにありません。",
    track_not_seekable: "この曲はシークに対応していません。",
    seek_past_end: "曲の長さを超える位置には移動できません。",
    not_connected: "Botはボイスチャンネルに接続していません。",
    voice_channel_mismatch: "Botは別のボイスチャンネルに接続しています。",
    queue_full: "キューが上限に達しています。",
    duplicate_track: "同じ曲はすでに再生中またはキューにあります。",
    state_exhausted: "プレイヤーを安全に更新できません。再接続してください。",
    idempotency_replayed: "同じ操作はすでに処理されています。最新の状態を確認してください。",
    result_expired: "操作結果の保持期限が切れました。最新の状態を確認してください。"
  };
  return messages[result.code];
}

function record(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${label} is invalid`);
  }
  return value as Record<string, unknown>;
}

function uuid(value: unknown, label: string): string {
  if (
    typeof value !== "string"
    || !UUID_PATTERN.test(value)
    || value.toLowerCase() === NIL_UUID
  ) {
    throw new Error(`${label} is invalid`);
  }
  return value;
}

function snowflake(value: unknown, label: string): string {
  if (typeof value !== "string" || !SNOWFLAKE_PATTERN.test(value)) {
    throw new Error(`${label} is invalid`);
  }
  return value;
}

function revision(value: unknown, label: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    throw new Error(`${label} is invalid`);
  }
  return value as number;
}

function boolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") throw new Error(`${label} is invalid`);
  return value;
}

function resultCode(value: unknown): CommandResultCode {
  if (typeof value !== "string" || !RESULT_CODES.has(value as CommandResultCode)) {
    throw new Error("Command result code is invalid");
  }
  return value as CommandResultCode;
}
