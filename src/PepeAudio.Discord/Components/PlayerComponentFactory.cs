// SPDX-License-Identifier: Apache-2.0
using Discord;
using PepeAudio.Core.Contracts;
using PepeAudio.Core.Enums;

namespace PepeAudio.Discord.Components;

// Builds the /now player entirely with Components V2 (no embeds, default colors).
public static class PlayerComponentFactory
{
    public static MessageComponent Build(PlayerState s, IReadOnlyCollection<string> presets)
    {
        var title = s.Current is null ? "## 待機中" : $"## {Link(s.Current, s.Current.Title)}";
        var subtitle = s.Current is null
            ? "キューは空です — `/play` で再生できます"
            : $"{Link(s.Current, s.Current.Artist)} | {SourceLabel(s.Current.Source)}";
        var progress = ProgressBarRenderer.Render(s.PositionMs, s.Current?.DurationMs ?? 0);
        var thumbnail = s.Current?.ThumbnailUrl;

        var transport = new ActionRowBuilder()
            .WithButton("⏮", PlayerCustomIds.For(PlayerControl.Previous), ButtonStyle.Secondary)
            .WithButton(s.IsPlaying ? "⏸" : "▶", PlayerCustomIds.For(PlayerControl.PlayPause), ButtonStyle.Primary)
            .WithButton("⏭", PlayerCustomIds.For(PlayerControl.Skip), ButtonStyle.Secondary);

        var controls = new ActionRowBuilder()
            .WithButton("⏹", PlayerCustomIds.For(PlayerControl.Stop), ButtonStyle.Danger)
            .WithButton(s.Loop == LoopMode.Track ? "🔂" : "🔁", PlayerCustomIds.For(PlayerControl.Loop),
                s.Loop == LoopMode.Off ? ButtonStyle.Secondary : ButtonStyle.Success)
            .WithButton("🔀", PlayerCustomIds.For(PlayerControl.Shuffle), s.Shuffle ? ButtonStyle.Success : ButtonStyle.Secondary);

        var actions = new ActionRowBuilder()
            .WithButton("AURA HRIR", PlayerCustomIds.For(PlayerControl.ToggleAura), s.AuraEnabled ? ButtonStyle.Success : ButtonStyle.Secondary)
            .WithButton("AURA 360°", PlayerCustomIds.For(PlayerControl.ToggleAura360), s.Aura360Enabled ? ButtonStyle.Success : ButtonStyle.Secondary)
            .WithButton("➕ 曲を追加", PlayerCustomIds.AddTrack, ButtonStyle.Success);

        var children = new List<IMessageComponentBuilder> { new TextDisplayBuilder($"{title}\n{subtitle}") };
        // Big album/thumbnail image between the title and the seek bar, like a music player.
        if (!string.IsNullOrWhiteSpace(thumbnail))
            children.Add(new MediaGalleryBuilder().AddItem(thumbnail));
        children.Add(new TextDisplayBuilder(progress));
        children.Add(new SeparatorBuilder());
        children.Add(transport);
        children.Add(controls);
        children.Add(new SeparatorBuilder());
        children.Add(actions);
        children.Add(new ActionRowBuilder().WithSelectMenu(BuildVolumeMenu(s.Volume)));
        if (presets.Count > 0)
            children.Add(new ActionRowBuilder().WithSelectMenu(BuildPresetMenu(presets, s.PresetName)));
        children.Add(new TextDisplayBuilder("-# Built-in Aura Sound System"));

        return new ComponentBuilderV2()
            .WithContainer(new ContainerBuilder().WithComponents(children))
            .Build();
    }

    private static string Link(TrackInfo t, string text)
    {
        var s = Format.Sanitize(text);
        return t.Url.StartsWith("http", StringComparison.OrdinalIgnoreCase) ? $"[{s}]({t.Url})" : s;
    }

    private static string SourceLabel(SourceKind s) => s switch
    {
        SourceKind.YouTube => "YouTube",
        SourceKind.AppleMusic => "Apple Music",
        SourceKind.Spotify => "Spotify",
        SourceKind.SoundCloud => "SoundCloud",
        SourceKind.Attachment => "ファイル",
        _ => "URL",
    };

    private static readonly int[] VolumeSteps = { 0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 150, 200 };

    private static SelectMenuBuilder BuildVolumeMenu(int current)
    {
        var menu = new SelectMenuBuilder()
            .WithCustomId(PlayerCustomIds.VolumeSelect)
            .WithPlaceholder($"🔊 音量 {current}%")
            .WithMinValues(1).WithMaxValues(1);
        foreach (var v in VolumeSteps)
            menu.AddOption($"{v}%", v.ToString(), isDefault: v == current);
        return menu;
    }

    private static SelectMenuBuilder BuildPresetMenu(IReadOnlyCollection<string> presets, string current)
    {
        var menu = new SelectMenuBuilder()
            .WithCustomId(PlayerCustomIds.PresetSelect)
            .WithPlaceholder($"🎧 HRIR: {current}")
            .WithMinValues(1).WithMaxValues(1);
        foreach (var p in presets.Take(25))
            menu.AddOption(p, p, isDefault: string.Equals(p, current, StringComparison.OrdinalIgnoreCase));
        return menu;
    }
}
