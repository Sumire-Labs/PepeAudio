// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Core.Exceptions;

public class PepeAudioException : Exception
{
    public PepeAudioException(string message) : base(message) { }
    public PepeAudioException(string message, Exception inner) : base(message, inner) { }
}

// Thrown when a /play input cannot be resolved to a playable track.
public sealed class ResolveFailedException : PepeAudioException
{
    public ResolveFailedException(string message) : base(message) { }
}

// Thrown when this host is already serving its maximum concurrent voice sessions.
public sealed class CapacityExceededException : PepeAudioException
{
    public CapacityExceededException(string message) : base(message) { }
}
