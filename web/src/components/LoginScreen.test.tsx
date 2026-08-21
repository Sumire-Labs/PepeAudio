import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { LoginScreen } from "./LoginScreen";

afterEach(cleanup);

describe("LoginScreen", () => {
  it("offers Discord login without rendering dashboard navigation", () => {
    const onLogin = vi.fn();
    render(
      <LoginScreen
        status="unauthenticated"
        message={null}
        onLogin={onLogin}
        onRetry={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Discordでログイン" }));
    expect(onLogin).toHaveBeenCalledOnce();
    expect(screen.queryByRole("navigation")).toBeNull();
  });

  it("shows a dedicated connection check before authentication completes", () => {
    render(
      <LoginScreen
        status="connecting"
        message={null}
        onLogin={vi.fn()}
        onRetry={vi.fn()}
      />
    );

    expect(screen.getByText("ログイン状態を確認しています…")).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
