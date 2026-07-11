export class YouTubeUnavailableError extends Error {}

export interface YouTubeSearchResult {
  videoId: string;
  title: string;
  author: string;
  url: string;
}

export interface YtDlpVideoInfo {
  id?: string;
  title?: string;
  uploader?: string;
  channel?: string;
  duration?: number; // seconds
  thumbnail?: string;
  webpage_url?: string;
}
