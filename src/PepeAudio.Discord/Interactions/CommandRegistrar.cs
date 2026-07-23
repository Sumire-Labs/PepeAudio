// SPDX-License-Identifier: Apache-2.0
using Discord.Interactions;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;

namespace PepeAudio.Discord.Interactions;

// Registers slash commands once. Dev registers to a guild (instant); prod registers globally.
public sealed class CommandRegistrar
{
    private readonly InteractionService _interactions;
    private readonly DiscordOptions _opt;
    private readonly ILogger<CommandRegistrar> _log;

    public CommandRegistrar(InteractionService interactions, IOptions<DiscordOptions> opt, ILogger<CommandRegistrar> log)
    {
        _interactions = interactions;
        _opt = opt.Value;
        _log = log;
    }

    public async Task RegisterAsync()
    {
        if (!_opt.UseGlobalCommands && _opt.DevGuildId is > 0)
        {
            await _interactions.RegisterCommandsToGuildAsync(_opt.DevGuildId.Value, deleteMissing: true);
            _log.LogInformation("Registered commands to dev guild {Guild}", _opt.DevGuildId.Value);
        }
        else
        {
            await _interactions.RegisterCommandsGloballyAsync(deleteMissing: true);
            _log.LogInformation("Registered global commands (propagation can take up to ~1h).");
        }
    }
}
