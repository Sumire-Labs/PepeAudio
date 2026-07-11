// Ambient declarations for third-party packages that ship no bundled/DefinitelyTyped types.
// Kept intentionally loose (any) — call sites should narrow with local interfaces.

declare module 'ffmpeg-static' {
  const ffmpegPath: string | null;
  export default ffmpegPath;
}

declare module 'spotify-url-info' {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  export default function spotifyUrlInfo(fetchFn: typeof fetch): any;
}

declare module '@vncsprd/soundcloud-downloader' {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const SoundCloud: any;
  export default SoundCloud;
}
