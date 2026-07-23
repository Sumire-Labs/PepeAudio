// SPDX-License-Identifier: Apache-2.0
using Discord;
using Discord.WebSocket;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;

namespace PepeAudio.Web.Realtime;

// The web-facing player DTO: the canonical PlayerState enriched with resolved requester
// names/avatars and reshaped into the flat, string-id'd form the dashboard consumes.
public sealed record QueueItemDto(
    string Id,
    string Title,
    string Artist,
    long DurationMs,
    string? ThumbnailUrl,
    int Source,
    string SourceUrl,
    bool IsLive,
    string RequestedBy,
    string? RequesterName,
    string? RequesterAvatarUrl);

public sealed record PlayerSnapshotDto(
    string GuildId,
    string Status,           // idle | playing | paused
    QueueItemDto? Current,
    long PositionMs,
    IReadOnlyList<QueueItemDto> Queue,
    IReadOnlyList<QueueItemDto> History,
    string LoopMode,         // off | track | queue
    bool Shuffle,
    bool Autoplay,
    int Volume,
    bool AuraEnabled,
    string PresetName,
    IReadOnlyList<string> Presets,
    int CrossfadeMs,
    long Epoch,
    DateTimeOffset UpdatedAt);

public static class PlayerSnapshot
{
    public static PlayerSnapshotDto From(PlayerState s, DiscordShardedClient client, IReadOnlyList<string> presets)
    {
        var guild = client.GetGuild(s.GuildId);
        var cache = new Dictionary<ulong, (string? Name, string? Avatar)>();

        (string? Name, string? Avatar) Resolve(ulong uid)
        {
            if (uid == 0) return (null, null);
            if (cache.TryGetValue(uid, out var hit)) return hit;
            // Prefer the guild member (nickname + per-guild avatar), fall back to the global user.
            var member = guild?.GetUser(uid);
            var user = (IUser?)member ?? client.GetUser(uid);
            var name = member?.DisplayName ?? user?.Username;
            var avatar = member?.GetDisplayAvatarUrl(ImageFormat.Auto, 64)
                ?? user?.GetAvatarUrl(ImageFormat.Auto, 64);
            var val = (name, avatar);
            cache[uid] = val;
            return val;
        }

        QueueItemDto Map(string id, TrackInfo t)
        {
            var (name, avatar) = Resolve(t.RequestedBy);
            return new QueueItemDto(
                id, t.Title, t.Artist, t.DurationMs, t.ThumbnailUrl,
                (int)t.Source, t.Url, t.IsLive, t.RequestedBy.ToString(), name, avatar);
        }

        var status = s.Current is null ? "idle" : s.IsPlaying ? "playing" : "paused";
        var loop = s.Loop switch { LoopMode.Track => "track", LoopMode.Queue => "queue", _ => "off" };

        return new PlayerSnapshotDto(
            s.GuildId.ToString(),
            status,
            s.Current is null ? null : Map("current", s.Current),
            s.PositionMs,
            s.Queue.Select(q => Map(q.Id, q.Track)).ToList(),
            s.History.Select(q => Map(q.Id, q.Track)).ToList(),
            loop, s.Shuffle, s.Autoplay, s.Volume,
            s.AuraEnabled, s.PresetName, presets, s.CrossfadeMs, s.Epoch, s.UpdatedAt);
    }
}
