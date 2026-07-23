// SPDX-License-Identifier: Apache-2.0
using Discord;
using Discord.Interactions;
using Discord.WebSocket;
using Microsoft.Extensions.Logging;

namespace PepeAudio.Discord.Interactions;

// Wires the InteractionService into the sharded client and surfaces command failures.
// RunMode.Async runs command bodies detached, so their exceptions arrive via
// InteractionExecuted rather than being thrown back to OnInteractionCreated.
public sealed class InteractionHandler
{
    private readonly DiscordShardedClient _client;
    private readonly InteractionService _interactions;
    private readonly IServiceProvider _services;
    private readonly ILogger<InteractionHandler> _log;

    public InteractionHandler(DiscordShardedClient client, InteractionService interactions,
        IServiceProvider services, ILogger<InteractionHandler> log)
    {
        _client = client;
        _interactions = interactions;
        _services = services;
        _log = log;
    }

    public async Task InitializeAsync()
    {
        await _interactions.AddModulesAsync(typeof(InteractionHandler).Assembly, _services);
        _client.InteractionCreated += OnInteractionCreated;
        _interactions.InteractionExecuted += OnInteractionExecuted;
        _interactions.Log += OnServiceLog;
    }

    private async Task OnInteractionCreated(SocketInteraction interaction)
    {
        try
        {
            var context = new ShardedInteractionContext(_client, interaction);
            await _interactions.ExecuteCommandAsync(context, _services);
        }
        catch (Exception ex)
        {
            _log.LogError(ex, "Interaction dispatch failed");
        }
    }

    private async Task OnInteractionExecuted(ICommandInfo command, IInteractionContext context, IResult result)
    {
        if (result.IsSuccess)
            return;

        var ex = result is ExecuteResult er ? er.Exception : null;
        _log.LogError(ex, "Command {Command} failed ({Error}): {Reason}",
            command?.Name ?? "?", result.Error, result.ErrorReason);

        try
        {
            var msg = $"コマンドの実行中にエラーが発生しました: {result.ErrorReason}";
            if (context.Interaction.HasResponded)
                await context.Interaction.FollowupTextAsync(msg);
            else
                await context.Interaction.RespondTextAsync(msg);
        }
        catch (Exception postEx)
        {
            _log.LogWarning(postEx, "Could not report the command failure to the user");
        }
    }

    private Task OnServiceLog(LogMessage msg)
    {
        var level = msg.Severity switch
        {
            LogSeverity.Critical => LogLevel.Critical,
            LogSeverity.Error => LogLevel.Error,
            LogSeverity.Warning => LogLevel.Warning,
            LogSeverity.Info => LogLevel.Information,
            LogSeverity.Verbose => LogLevel.Debug,
            _ => LogLevel.Trace,
        };
        _log.Log(level, msg.Exception, "[Interactions/{Source}] {Message}", msg.Source, msg.Message);
        return Task.CompletedTask;
    }
}
