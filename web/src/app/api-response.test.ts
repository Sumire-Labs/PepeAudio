import { describe, expect, it } from "vitest";

import { ApiResponseError, publicErrorMessage, readJsonResponse } from "./api-response";

describe("API response boundary", () => {
  it("maps known API codes without exposing the backend message", async () => {
    const response = jsonResponse({
      error: {
        code: "revision_conflict",
        message: "database host and password detail"
      }
    }, 409);

    await expect(readJsonResponse(response)).rejects.toMatchObject({
      status: 409,
      code: "revision_conflict",
      message: "プレイヤーの状態が変わりました。最新の状態でやり直してください。"
    });
  });

  it("understands the compact auth error envelope", async () => {
    await expect(readJsonResponse(jsonResponse({ error: "csrf_rejected" }, 403)))
      .rejects.toMatchObject({
        status: 403,
        code: "csrf_rejected",
        message: "セッションの有効期限が切れました。Discordでログインし直してください。"
      });
  });

  it("falls back to the HTTP status when an envelope is malformed or inconsistent", async () => {
    await expect(readJsonResponse(jsonResponse({
      error: { code: "authentication_required", message: 42 }
    }, 500))).rejects.toMatchObject({
      status: 500,
      message: "通信中にエラーが発生しました。しばらくしてからお試しください。"
    });
    await expect(readJsonResponse(jsonResponse({
      error: { code: "authentication_required", message: "ignored" }
    }, 403))).rejects.toMatchObject({
      status: 403,
      message: "この操作を行う権限がないか、セッションの有効期限が切れています。"
    });
  });

  it("rejects malformed success responses with a safe client message", async () => {
    const response = new Response("not JSON", { status: 200 });
    await expect(readJsonResponse(response)).rejects.toMatchObject({
      status: 502,
      code: "invalid_response"
    });
  });

  it("does not surface arbitrary parser errors", () => {
    expect(publicErrorMessage(new Error("private adapter detail"), "操作に失敗しました。"))
      .toBe("操作に失敗しました。");
    expect(publicErrorMessage(
      new ApiResponseError(503, "一時的に利用できません。"),
      "操作に失敗しました。"
    )).toBe("一時的に利用できません。");
  });
});

function jsonResponse(body: unknown, status: number): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" }
  });
}
