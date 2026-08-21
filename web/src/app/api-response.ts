const MAX_JSON_RESPONSE_BYTES = 1_048_576;

interface KnownError {
  readonly statuses: readonly number[];
  readonly message: string;
}

const KNOWN_ERRORS: Readonly<Record<string, KnownError>> = {
  authentication_required: { statuses: [401], message: "Discordでログインし直してください。" },
  csrf_rejected: {
    statuses: [403],
    message: "セッションの有効期限が切れました。Discordでログインし直してください。"
  },
  invalid_oauth_callback: {
    statuses: [400],
    message: "Discordログインを完了できませんでした。もう一度お試しください。"
  },
  authentication_unavailable: {
    statuses: [503],
    message: "Discordログインを現在利用できません。しばらくしてからお試しください。"
  },
  forbidden: {
    statuses: [403],
    message: "このサーバーではPepeAudioを利用できません。Botの導入状況を確認してください。"
  },
  invalid_request: {
    statuses: [400],
    message: "送信した内容を処理できませんでした。画面を再読み込みしてお試しください。"
  },
  player_not_found: { statuses: [404], message: "選択したサーバーのプレイヤーが見つかりません。" },
  command_result_not_found: {
    statuses: [404],
    message: "操作結果の保持期限が切れました。最新の状態を確認してください。"
  },
  revision_conflict: {
    statuses: [409],
    message: "プレイヤーの状態が変わりました。最新の状態でやり直してください。"
  },
  idempotency_conflict: {
    statuses: [409],
    message: "同じ操作がすでに処理されています。最新の状態を確認してください。"
  },
  invalid_player_command: {
    statuses: [422],
    message: "現在の再生状態ではこの操作を実行できません。"
  },
  service_unavailable: {
    statuses: [503],
    message: "PepeAudioを一時的に利用できません。しばらくしてからお試しください。"
  },
  internal_error: {
    statuses: [500],
    message: "操作を完了できませんでした。しばらくしてからお試しください。"
  }
};

export class ApiResponseError extends Error {
  public override readonly name = "ApiResponseError";

  public constructor(
    public readonly status: number,
    message: string,
    public readonly code: string | null = null
  ) {
    super(message);
  }
}

export class UserFacingError extends Error {
  public override readonly name = "UserFacingError";
}

export async function readJsonResponse(response: Response): Promise<unknown> {
  const declaredLength = Number(response.headers.get("content-length"));
  if (Number.isFinite(declaredLength) && declaredLength > MAX_JSON_RESPONSE_BYTES) {
    throw invalidResponse();
  }

  const text = await response.text();
  if (new TextEncoder().encode(text).byteLength > MAX_JSON_RESPONSE_BYTES) {
    throw invalidResponse();
  }

  let body: unknown = null;
  if (text.length > 0) {
    try {
      body = JSON.parse(text) as unknown;
    } catch {
      if (response.ok) throw invalidResponse();
    }
  }

  if (!response.ok) {
    const code = errorCode(body);
    const known = code === null ? undefined : KNOWN_ERRORS[code];
    throw new ApiResponseError(
      response.status,
      known?.statuses.includes(response.status) === true
        ? known.message
        : statusMessage(response.status),
      code
    );
  }
  return body;
}

export function publicErrorMessage(error: unknown, fallback: string): string {
  return error instanceof ApiResponseError || error instanceof UserFacingError
    ? error.message
    : fallback;
}

function errorCode(body: unknown): string | null {
  if (!isRecord(body)) return null;
  if (typeof body.error === "string") return boundedCode(body.error);
  if (!isRecord(body.error)) return null;
  if (
    typeof body.error.code !== "string"
    || typeof body.error.message !== "string"
    || body.error.message.length > 512
  ) {
    return null;
  }
  const revision = body.error.current_revision;
  if (revision !== undefined && (!Number.isSafeInteger(revision) || (revision as number) < 0)) {
    return null;
  }
  return boundedCode(body.error.code);
}

function boundedCode(value: string): string | null {
  return /^[a-z][a-z0-9_]{0,63}$/u.test(value) ? value : null;
}

function statusMessage(status: number): string {
  switch (status) {
    case 400:
      return "送信した内容を処理できませんでした。画面を再読み込みしてお試しください。";
    case 401:
      return "Discordでログインし直してください。";
    case 403:
      return "この操作を行う権限がないか、セッションの有効期限が切れています。";
    case 404:
      return "要求した情報が見つかりませんでした。";
    case 409:
      return "プレイヤーの状態が変わりました。最新の状態でやり直してください。";
    case 422:
      return "現在の状態ではこの操作を実行できません。";
    case 429:
      return "操作が集中しています。少し待ってからお試しください。";
    case 502:
    case 503:
    case 504:
      return "PepeAudioを一時的に利用できません。しばらくしてからお試しください。";
    default:
      return "通信中にエラーが発生しました。しばらくしてからお試しください。";
  }
}

function invalidResponse(): ApiResponseError {
  return new ApiResponseError(
    502,
    "サーバーから正しい応答を受け取れませんでした。画面を再読み込みしてください。",
    "invalid_response"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
