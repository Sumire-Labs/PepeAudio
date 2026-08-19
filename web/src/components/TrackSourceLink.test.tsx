import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { TrackProvenance } from "../app/types";
import { TrackSourceLinks } from "./TrackSourceLink";

describe("TrackSourceLinks", () => {
  it("shows both catalog origin and playback with safe external attributes", () => {
    render(<TrackSourceLinks provenance={spotifyToYouTube()} />);

    const origin = screen.getByRole("link", { name: /Spotifyで開く/u });
    const playback = screen.getByRole("link", { name: /YouTubeで再生/u });
    expect(origin.getAttribute("href")).toBe(
      "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"
    );
    expect(playback.getAttribute("href")).toBe(
      "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    );
    for (const link of [origin, playback]) {
      expect(link.getAttribute("target")).toBe("_blank");
      expect(link.getAttribute("rel")).toContain("noopener");
      expect(link.getAttribute("rel")).toContain("noreferrer");
    }
  });

  it("falls back to the playback page when there is no catalog origin", () => {
    render(
      <TrackSourceLinks
        provenance={{
          origin: null,
          playback: {
            provider: "soundcloud",
            url: "https://soundcloud.com/example/example-track"
          }
        }}
      />
    );

    expect(screen.getByRole("link", { name: /SoundCloudで再生/u }).getAttribute("href"))
      .toBe("https://soundcloud.com/example/example-track");
  });

  it("deduplicates an identical origin and playback page", () => {
    const youtube = {
      provider: "youtube" as const,
      url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    };
    const { container } = render(
      <TrackSourceLinks provenance={{ origin: youtube, playback: youtube }} />
    );

    expect(container.querySelectorAll("a")).toHaveLength(1);
    expect(container.textContent).toContain("YouTubeで開く");
  });

  it("does not render a signed CDN locator", () => {
    const { container } = render(
      <TrackSourceLinks
        provenance={{
          origin: null,
          playback: {
            provider: "youtube",
            url: "https://rr1---sn.example.googlevideo.com/videoplayback?sig=secret"
          }
        }}
      />
    );

    expect(container.childElementCount).toBe(0);
  });

  it("renders nothing without validated provenance", () => {
    const { container } = render(<TrackSourceLinks provenance={null} />);

    expect(container.childElementCount).toBe(0);
  });
});

function spotifyToYouTube(): TrackProvenance {
  return {
    origin: {
      provider: "spotify",
      url: "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"
    },
    playback: {
      provider: "youtube",
      url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    }
  };
}
