// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Core.Contracts;

// One search result shown in the web queue's "add" panel. Url is the source page URL the
// user can enqueue directly.
public sealed record SearchCandidate(string Title, string Author, string Url, string ThumbnailUrl);
