// SPDX-License-Identifier: Apache-2.0
using Discord;

namespace PepeAudio.Discord.Interactions;

// Renders plain status/error replies as Components V2 (no embeds), matching the player card.
public static class InteractionResponses
{
    public static MessageComponent Text(string text)
        => new ComponentBuilderV2()
            .WithContainer(new ContainerBuilder().WithComponents(
                new List<IMessageComponentBuilder> { new TextDisplayBuilder(text) }))
            .Build();

    public static Task RespondTextAsync(this IDiscordInteraction interaction, string text)
        => interaction.RespondAsync(components: Text(text), ephemeral: true, flags: MessageFlags.ComponentsV2);

    public static Task<IUserMessage> FollowupTextAsync(this IDiscordInteraction interaction, string text)
        => interaction.FollowupAsync(components: Text(text), ephemeral: true, flags: MessageFlags.ComponentsV2);
}
