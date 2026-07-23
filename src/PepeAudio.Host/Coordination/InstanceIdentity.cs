// SPDX-License-Identifier: Apache-2.0
using Microsoft.Extensions.Configuration;

namespace PepeAudio.Host.Coordination;

// Stable id for this process in the shard fleet (from PEPE:ContainerId or random).
public sealed class InstanceIdentity
{
    public InstanceIdentity(IConfiguration config)
        => InstanceId = config["Pepe:ContainerId"] is { Length: > 0 } id ? id : $"inst-{Guid.NewGuid():N}";

    public string InstanceId { get; }
}
