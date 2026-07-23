// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Core.Contracts;

// Id is the owning PlayableRef's stable id; queue order is the list order, so no index field.
public sealed record QueueEntry(string Id, TrackInfo Track);
