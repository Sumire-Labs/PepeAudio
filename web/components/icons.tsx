// SPDX-License-Identifier: Apache-2.0
/** Inline-SVG icon set. Pure SVG — no "use client" needed. */
import type { ReactNode } from "react";

type IconProps = { className?: string };

const svg = (children: ReactNode, filled = false) =>
  function Icon({ className }: IconProps) {
    return (
      <svg
        viewBox="0 0 24 24"
        className={className}
        fill={filled ? "currentColor" : "none"}
        stroke={filled ? "none" : "currentColor"}
        strokeWidth={filled ? 0 : 1.8}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        {children}
      </svg>
    );
  };

export const Play = svg(<path d="M8 5.5v13l11-6.5z" />, true);

export const Pause = svg(
  <>
    <rect x="6" y="5" width="4" height="14" rx="1.2" />
    <rect x="14" y="5" width="4" height="14" rx="1.2" />
  </>,
  true,
);

export const Next = svg(<path d="M6 5l9 7-9 7zM17 5h2v14h-2z" />, true);

export const Prev = svg(<path d="M18 5l-9 7 9 7zM5 5h2v14H5z" />, true);

export const Stop = svg(<rect x="6" y="6" width="12" height="12" rx="2.2" />, true);

export const Shuffle = svg(
  <>
    <path d="M4 5h4l10 14h2" />
    <path d="M18 5h2v0" />
    <path d="M16 5h4v4" />
    <path d="M4 19h4l3-4.2" />
    <path d="M16 19h4v-4" />
    <path d="M13.2 9.2 8 5H4" />
  </>,
);

export const Repeat = svg(
  <>
    <path d="M17 3l3 3-3 3" />
    <path d="M4 12V9a3 3 0 0 1 3-3h13" />
    <path d="M7 21l-3-3 3-3" />
    <path d="M20 12v3a3 3 0 0 1-3 3H4" />
  </>,
);

export const RepeatOne = svg(
  <>
    <path d="M17 3l3 3-3 3" />
    <path d="M4 12V9a3 3 0 0 1 3-3h13" />
    <path d="M7 21l-3-3 3-3" />
    <path d="M20 12v3a3 3 0 0 1-3 3H4" />
    <path d="M11 10.5h1.5V15" strokeWidth={1.6} />
  </>,
);

export const Radio = svg(
  <>
    <circle cx="12" cy="12" r="2.2" fill="currentColor" stroke="none" />
    <path d="M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7" />
    <path d="M6 6a9 9 0 0 0 0 12M18 6a9 9 0 0 1 0 12" />
  </>,
);

export const VolumeHigh = svg(
  <>
    <path d="M5 9v6h3l4 4V5L8 9z" fill="currentColor" stroke="none" />
    <path d="M15.5 9.5a3.5 3.5 0 0 1 0 5M18 7a7 7 0 0 1 0 10" />
  </>,
);

export const VolumeMute = svg(
  <>
    <path d="M5 9v6h3l4 4V5L8 9z" fill="currentColor" stroke="none" />
    <path d="M16 9.5l4 5M20 9.5l-4 5" />
  </>,
);

export const Headphones = svg(
  <>
    <path d="M5 13v-1a7 7 0 0 1 14 0v1" />
    <rect x="3.5" y="13" width="4" height="7" rx="1.6" />
    <rect x="16.5" y="13" width="4" height="7" rx="1.6" />
  </>,
);

export const Spatial = svg(
  <>
    <circle cx="12" cy="12" r="8" />
    <ellipse cx="12" cy="12" rx="8" ry="3.4" />
    <ellipse cx="12" cy="12" rx="3.4" ry="8" />
  </>,
);

export const Search = svg(
  <>
    <circle cx="11" cy="11" r="6.5" />
    <path d="M20 20l-3.6-3.6" />
  </>,
);

export const Logout = svg(
  <>
    <path d="M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3" />
    <path d="M10 8l-4 4 4 4" />
    <path d="M6 12h11" />
  </>,
);

export const Bookmark = svg(<path d="M6 4h12v16l-6-4-6 4z" />);

export const More = svg(
  <>
    <circle cx="5" cy="12" r="1.7" fill="currentColor" stroke="none" />
    <circle cx="12" cy="12" r="1.7" fill="currentColor" stroke="none" />
    <circle cx="19" cy="12" r="1.7" fill="currentColor" stroke="none" />
  </>,
);

export const Check = svg(<path d="M5 12.5l4.5 4.5L19 7" />);

export const Chevron = svg(<path d="M6 9l6 6 6-6" />);

export const Grip = svg(
  <>
    <circle cx="9" cy="6" r="1.3" fill="currentColor" stroke="none" />
    <circle cx="9" cy="12" r="1.3" fill="currentColor" stroke="none" />
    <circle cx="9" cy="18" r="1.3" fill="currentColor" stroke="none" />
    <circle cx="15" cy="6" r="1.3" fill="currentColor" stroke="none" />
    <circle cx="15" cy="12" r="1.3" fill="currentColor" stroke="none" />
    <circle cx="15" cy="18" r="1.3" fill="currentColor" stroke="none" />
  </>,
);

export const Close = svg(<path d="M6 6l12 12M18 6L6 18" />);

export const Menu = svg(<path d="M4 7h16M4 12h16M4 17h16" />);

export const Plus = svg(<path d="M12 5v14M5 12h14" />);

export const Trash = svg(
  <>
    <path d="M4 7h16" />
    <path d="M9 7V5h6v2" />
    <path d="M6 7l1 13h10l1-13" />
  </>,
);

export const Queue = svg(
  <>
    <path d="M4 7h11M4 12h11M4 17h7" />
    <circle cx="18" cy="16" r="2.4" />
    <path d="M20.4 15V9l-2 .6" />
  </>,
);

export const Playlist = svg(
  <>
    <path d="M4 6h10M4 10h10M4 14h6" />
    <circle cx="17" cy="15" r="3.2" />
    <path d="M20.2 14V7l-3 .8" />
  </>,
);

export const Server = svg(
  <>
    <rect x="4" y="5" width="16" height="6" rx="2" />
    <rect x="4" y="13" width="16" height="6" rx="2" />
    <circle cx="8" cy="8" r="0.9" fill="currentColor" stroke="none" />
    <circle cx="8" cy="16" r="0.9" fill="currentColor" stroke="none" />
  </>,
);

export const Download = svg(
  <>
    <path d="M12 4v10" />
    <path d="M8 11l4 4 4-4" />
    <path d="M5 19h14" />
  </>,
);

export const Edit = svg(
  <>
    <path d="M4 20h4l10-10-4-4L4 16v4z" />
    <path d="M13.5 6.5l4 4" />
  </>,
);

export const Pin = svg(
  <>
    <path d="M9 4h6l-1 6 3 3H7l3-3-1-6z" />
    <path d="M12 16v4" />
  </>,
);

export const Clock = svg(
  <>
    <circle cx="12" cy="12" r="8" />
    <path d="M12 8v4.5l3 2" />
  </>,
);

export const Sun = svg(
  <>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M4 12H2M22 12h-2M5 5l1.5 1.5M17.5 17.5 19 19M19 5l-1.5 1.5M6.5 17.5 5 19" />
  </>,
);

export const Moon = svg(<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z" />);

// Discord glyph (fill treatment) — not present in the old ui.tsx; added for parity.
export const Discord = svg(
  <path d="M19.5 5.3A16 16 0 0 0 15.5 4l-.2.4a13 13 0 0 1 3.5 1.4 12.5 12.5 0 0 0-10.6 0A13 13 0 0 1 11.7 4.4L11.5 4a16 16 0 0 0-4 1.3C4 8.7 3.4 12 3.6 15.2a16 16 0 0 0 4.8 2.4l.5-.9a10 10 0 0 1-1.6-.8l.4-.3a11 11 0 0 0 9.4 0l.4.3a10 10 0 0 1-1.6.8l.5.9a16 16 0 0 0 4.8-2.4c.3-3.8-.6-7-2.6-9.9zM9.3 13.4c-.8 0-1.4-.7-1.4-1.6 0-.9.6-1.6 1.4-1.6.8 0 1.4.7 1.4 1.6 0 .9-.6 1.6-1.4 1.6zm5.4 0c-.8 0-1.4-.7-1.4-1.6 0-.9.6-1.6 1.4-1.6.8 0 1.4.7 1.4 1.6 0 .9-.6 1.6-1.4 1.6z" />,
  true,
);
