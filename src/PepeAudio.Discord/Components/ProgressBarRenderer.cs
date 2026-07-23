// SPDX-License-Identifier: Apache-2.0
using System.Text;

namespace PepeAudio.Discord.Components;

// Components V2 has no native progress bar, so /now renders a monospace one.
public static class ProgressBarRenderer
{
    private const int Slots = 18;

    public static string Render(long positionMs, long durationMs)
    {
        if (durationMs <= 0)
            return $"`{Time(positionMs)}` · 🔴 LIVE";

        var ratio = Math.Clamp((double)positionMs / durationMs, 0, 1);
        var knob = (int)Math.Round(ratio * (Slots - 1));
        var bar = new StringBuilder(Slots * 2);
        for (var i = 0; i < Slots; i++)
            bar.Append(i == knob ? "🔘" : i < knob ? "━" : "─");
        return $"`{Time(positionMs)}` {bar} `{Time(durationMs)}`";
    }

    public static string Time(long ms)
    {
        var t = TimeSpan.FromMilliseconds(Math.Max(0, ms));
        return t.TotalHours >= 1 ? $"{(int)t.TotalHours}:{t.Minutes:D2}:{t.Seconds:D2}" : $"{t.Minutes}:{t.Seconds:D2}";
    }
}
