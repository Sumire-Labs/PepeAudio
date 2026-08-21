import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MediaSearchBar } from "./MediaSearchBar";

afterEach(cleanup);

describe("MediaSearchBar", () => {
  it("submits a trimmed song search and clears the field", async () => {
    const onSubmit = vi.fn(async () => undefined);
    render(
      <MediaSearchBar isDisabled={false} isLoading={false} onSubmit={onSubmit} />
    );
    const input = screen.getByRole("textbox", { name: "曲を検索またはURLを追加" });

    fireEvent.change(input, { target: { value: "  Alan Walker Faded  " } });
    fireEvent.click(screen.getByRole("button", { name: "キューに追加" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith("Alan Walker Faded"));
    await waitFor(() => expect((input as HTMLInputElement).value).toBe(""));
  });

  it("does not submit without an active synchronized player", () => {
    const onSubmit = vi.fn();
    render(
      <MediaSearchBar isDisabled isLoading={false} onSubmit={onSubmit} />
    );

    expect(
      (screen.getByRole("button", { name: "キューに追加" }) as HTMLButtonElement)
        .disabled
    ).toBe(true);
  });
});
