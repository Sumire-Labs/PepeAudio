// Thin barrel re-exporting the split-up src/sources/youtube/* modules under
// their original names/signatures, so existing importers (src/sources/index.ts,
// src/sources/youtubeMatch.ts) keep resolving without any edit.
export { YouTubeUnavailableError, type YouTubeSearchResult } from './youtube/types.js';
export { createYouTubeStreamGetter } from './youtube/streamResolvers.js';
export { resolveYouTubeVideoId, fetchYouTubeMetadata, type YouTubeMetadata } from './youtube/queueItemBuilders.js';
export { resolveYouTubeUrl } from './youtube/resolveUrl.js';
export { searchYouTube } from './youtube/search.js';
export { resolveAutoplayTracks } from './youtube/related.js';
