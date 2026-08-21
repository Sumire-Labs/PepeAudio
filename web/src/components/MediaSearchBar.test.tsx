import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MediaSearchBar } from "./MediaSearchBar";

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

describe("MediaSearchBar", () => {
  it("submits a trimmed song search and clears the field", async () => {
    const onSubmit = vi.fn(async () => undefined);
    render(
      <MediaSearchBar isDisabled={false} isLoading={false} onSubmit={onSubmit} />
    );
    const input = screen.getByRole("combobox", { name: "曲を検索またはURLを追加" });

    fireEvent.change(input, { target: { value: "  Alan Walker Faded  " } });
    fireEvent.click(screen.getByRole("button", { name: "キューに追加" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith("Alan Walker Faded"));
    await waitFor(() => expect(
      (screen.getByRole("combobox", {
        name: "曲を検索またはURLを追加"
      }) as HTMLInputElement).value
    ).toBe(""));
  });

  it("does not submit without an active synchronized player", () => {
    const onSubmit = vi.fn();
    render(
      <MediaSearchBar isDisabled isLoading={false} onSubmit={onSubmit} />
    );

    expect(screen.getByRole("button", { name: "キューに追加" })
      .getAttribute("aria-disabled")).toBe("true");
  });

  it("shows up to five contextual suggestions under the search field", async () => {
    render(
      <MediaSearchBar
        isDisabled={false}
        isLoading={false}
        suggestions={[
          { id: "1", title: "Faded", artist: "Alan Walker" },
          { id: "2", title: "Faded Live", artist: "Alan Walker" }
        ]}
        onSubmit={vi.fn()}
      />
    );
    const input = screen.getByRole("combobox", { name: "曲を検索またはURLを追加" });
    fireEvent.change(input, { target: { value: "Faded" } });

    await waitFor(() => expect(screen.getByText("「Faded」を検索")).toBeTruthy());
    expect(screen.getByText("Faded Live")).toBeTruthy();
  });
});
