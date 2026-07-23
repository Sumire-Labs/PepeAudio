// SPDX-License-Identifier: Apache-2.0
using PepeAudio.Core.Enums;

namespace PepeAudio.Discord.Components;

// Custom IDs for /now player buttons. Pattern: "player:{action}".
public static class PlayerCustomIds
{
    public const string Prefix = "player";

    // Select-menu / button ids — deliberately outside the "player:*" button namespace.
    public const string VolumeSelect = "pvol";
    public const string PresetSelect = "ppreset";
    public const string AddTrack = "padd";
    public const string AddTrackModal = "paddmodal";

    public static string For(PlayerControl control) => $"{Prefix}:{control}";

    public static bool TryParse(string action, out PlayerControl control)
        => Enum.TryParse(action, ignoreCase: true, out control);
}
