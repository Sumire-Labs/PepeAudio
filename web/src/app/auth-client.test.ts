import { describe, expect, it, vi } from "vitest";

import { fetchAuthBootstrap, logoutSession } from "./auth-client";

const CSRF_TOKEN = "A".repeat(43);

describe("auth client", () => {
  it("parses session and guild responses before returning bootstrap data", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce(jsonResponse({
        userId: "2",
        username: "pepe-listener",
        displayName: "Pepe Listener",
        avatar: "a_safe_hash",
        csrfToken: CSRF_TOKEN,
        createdAtMs: 1,
        expiresAtMs: 2
      }))
      .mockResolvedValueOnce(jsonResponse({
        guilds: [{
          id: "3",
          name: "Room",
          icon: null,
          owner: true,
          permissions: "8",
          botPresent: true
        }]
      }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(fetchAuthBootstrap()).resolves.toMatchObject({
      csrfToken: CSRF_TOKEN,
      guilds: [{ id: "3", name: "Room" }],
      account: {
        source: "discord",
        userId: "2",
        username: "pepe-listener",
        displayName: "Pepe Listener",
        avatarUrl: "https://cdn.discordapp.com/avatars/2/a_safe_hash.webp?size=64"
      },
      logoutAvailable: true
    });
  });

  it("does not request the guild list after the session check rejects authentication", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({
      error: "authentication_required"
    }), {
      status: 401,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(fetchAuthBootstrap()).rejects.toMatchObject({ status: 401 });

    expect(fetchMock).toHaveBeenCalledOnce();
    expect(fetchMock).toHaveBeenCalledWith("/auth/session", {
      credentials: "same-origin",
      signal: null
    });
  });

  it("posts logout with the synchronizer token and same-origin credentials", async () => {
    const fetchMock = vi.fn(async () => new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await logoutSession(CSRF_TOKEN);

    expect(fetchMock).toHaveBeenCalledWith("/auth/logout", {
      method: "POST",
      credentials: "same-origin",
      headers: { "x-csrf-token": CSRF_TOKEN }
    });
  });
});

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" }
  });
}
