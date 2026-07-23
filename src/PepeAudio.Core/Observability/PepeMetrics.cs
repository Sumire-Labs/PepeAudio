// SPDX-License-Identifier: Apache-2.0
using System.Diagnostics;
using System.Diagnostics.Metrics;

namespace PepeAudio.Core.Observability;

// Domain metrics and traces, emitted on the "PepeAudio" meter / activity source.
public static class PepeMetrics
{
    public const string MeterName = "PepeAudio";

    // Trace source for domain spans (play, control), exported via OpenTelemetry.
    public static readonly ActivitySource Trace = new(MeterName);

    private static readonly Meter Meter = new(MeterName, "1.0.0");

    public static readonly Counter<long> TracksEnqueued =
        Meter.CreateCounter<long>("pepe.tracks.enqueued", unit: "{track}", description: "Tracks added to a queue.");

    public static readonly Counter<long> ControlCommands =
        Meter.CreateCounter<long>("pepe.control.commands", unit: "{command}", description: "Player control commands applied.");

    public static readonly Counter<long> ResolveFailures =
        Meter.CreateCounter<long>("pepe.resolve.failures", unit: "{failure}", description: "Source resolutions that produced nothing playable.");

    public static void RegisterActiveVoices(Func<int> activeVoices) =>
        Meter.CreateObservableGauge("pepe.voice.active", activeVoices, unit: "{session}", description: "Active voice sessions on this host.");
}
