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
      <MediaSearchBar disabledMessage={null} isLoading={false} onSubmit={onSubmit} />
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

  it("explains why search is unavailable and never submits", () => {
    const onSubmit = vi.fn();
    render(
      <MediaSearchBar
        disabledMessage="このサーバーにはPepeAudioが導入されていません。"
        isLoading={false}
        onSubmit={onSubmit}
      />
    );

    expect(screen.getAllByText("このサーバーにはPepeAudioが導入されていません。"))
      .toHaveLength(2);
    expect(screen.getByRole("combobox", { name: "曲を検索またはURLを追加" })
      .getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByRole("button", { name: "キューに追加" })
      .getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "キューに追加" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("shows up to five contextual suggestions under the search field", async () => {
    render(
      <MediaSearchBar
        disabledMessage={null}
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
