// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;

namespace PepeAudio.Core.Contracts;

// A reference the audio pipeline can play. When NeedsResolution is true, Input is a
// source page URL that IStreamProvider resolves to a direct stream URL just before
// playback. Prefetched may carry a stream URL captured together with metadata in one
// yt-dlp call; the provider reuses it while it is still fresh and otherwise re-resolves.
// When NeedsResolution is false, Input is already a directly-playable URL or local path.
public sealed record PlayableRef(
    SourceKind Source,
    string Input,
    bool Seekable,
    TrackInfo Info,
    bool NeedsResolution = false,
    string? Prefetched = null);
