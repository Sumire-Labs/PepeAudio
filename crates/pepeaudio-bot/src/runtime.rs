use std::sync::Arc;

use poise::serenity_prelude as serenity;
use serenity::all::GatewayIntents;
use songbird::{SerenityInit as _, Songbird};

use crate::{
    BotConfig, BotError, CommandError, ComponentIdCodec, ComponentsV2Responder,
    GuildLifecycleHandle, GuildPolicyProvider, HrirOption, MediaResolver, PlayerRegistry,
    commands::{leave, now, play, stop},
    component_dispatch,
    discord_status::{DiscordStatusRuntime, initial_activity},
    framework_error,
};

pub struct BotData {
    pub players: Arc<PlayerRegistry>,
    pub media: Arc<dyn MediaResolver>,
    pub components: Arc<dyn ComponentsV2Responder>,
    pub component_ids: ComponentIdCodec,
    pub hrir_options: Arc<[HrirOption]>,
    pub guild_policy: Arc<dyn GuildPolicyProvider>,
    /// Optional restart-safe guild lifecycle and presence handle.
    pub guild_lifecycle: Option<GuildLifecycleHandle>,
}

#[must_use]
pub fn commands() -> Vec<poise::Command<BotData, CommandError>> {
    vec![play(), now(), stop(), leave()]
}

/// # Errors
///
/// Returns [`BotError`] if startup, the gateway, or graceful shutdown fails.
pub async fn run(config: BotConfig) -> Result<(), BotError> {
    crate::production::run(config).await
}

/// This dependency-injection entrypoint is also usable by integration tests
/// that replace the production storage and media assembly.
///
/// # Errors
///
/// Returns [`BotError`] if client construction or the Discord gateway fails.
pub async fn run_with_data(config: BotConfig, data: BotData) -> Result<(), BotError> {
    let manager = Songbird::serenity();
    let mut client = build_client(&config, Arc::new(data), manager).await?;
    let status = DiscordStatusRuntime::start(client.shard_manager.clone());
    let result = start_client(&mut client, &config).await;
    if !status.shutdown().await && result.is_ok() {
        return Err(BotError::Shutdown);
    }
    result
}

pub(crate) async fn build_client(
    config: &BotConfig,
    data: Arc<BotData>,
    manager: Arc<Songbird>,
) -> Result<serenity::Client, BotError> {
    let setup_data = data.clone();
    let development_guild_id = config.development_guild_id;
    let should_register_commands = config.shards().range().contains(&0);
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands(),
            event_handler: component_dispatch::event_handler,
            on_error: framework_error::on_error,
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            let data = setup_data.clone();
            Box::pin(async move {
                if should_register_commands {
                    if let Some(guild_id) = development_guild_id {
                        let guild_id = serenity::GuildId::new(guild_id);
                        poise::builtins::register_in_guild(
                            ctx,
                            &framework.options().commands,
                            guild_id,
                        )
                        .await?;
                    } else {
                        poise::builtins::register_globally(ctx, &framework.options().commands)
                            .await?;
                    }
                }
                Ok((*data).clone_for_poise())
            })
        })
        .build();
    // Current member roles are part of every control authorization decision.
    // GUILD_MEMBERS keeps the cache-backed facts fresh across role changes;
    // missing cache entries still fail closed in the command adapters.
    let intents =
        GatewayIntents::GUILDS | GatewayIntents::GUILD_MEMBERS | GatewayIntents::GUILD_VOICE_STATES;
    let builder = serenity::ClientBuilder::new(&config.discord_token, intents);
    let builder = match initial_activity() {
        Some(activity) => builder.activity(activity),
        None => builder,
    };
    builder
        .framework(framework)
        .register_songbird_with(manager)
        .await
        .map_err(|error| BotError::ClientBuild(Box::new(error)))
}

pub(crate) async fn start_client(
    client: &mut serenity::Client,
    config: &BotConfig,
) -> Result<(), BotError> {
    client
        .start_shard_range(config.shards().range(), config.shards().total())
        .await
        .map_err(|error| BotError::Gateway(Box::new(error)))
}

impl BotData {
    fn clone_for_poise(&self) -> Self {
        Self {
            players: self.players.clone(),
            media: self.media.clone(),
            components: self.components.clone(),
            component_ids: self.component_ids.clone(),
            hrir_options: self.hrir_options.clone(),
            guild_policy: self.guild_policy.clone(),
            guild_lifecycle: self.guild_lifecycle.clone(),
        }
    }
}
