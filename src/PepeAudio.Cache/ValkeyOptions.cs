// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Cache;

public sealed class ValkeyOptions
{
    public const string Section = "ConnectionStrings";

    // Bound from ConnectionStrings:Valkey
    public string? Valkey { get; set; }
}
