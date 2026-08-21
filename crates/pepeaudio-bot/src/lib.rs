//! Discord transport and Songbird integration boundaries for `PepeAudio`.

pub mod commands;
mod component_dispatch;
mod component_id;
mod components;
mod config;
mod dashboard_command_executor;
mod discord_status;
mod display_text;
mod error;
mod framework_error;
mod gateway_state;
mod guild_lifecycle;
mod guild_lifecycle_actor;
mod guild_policy;
mod interaction;
mod media;
mod permissions;
mod process_memory;
mod production;
mod production_build;
mod production_event_bridge;
#[cfg(test)]
mod production_event_bridge_tests;
mod production_factory;
mod production_janitor;
mod production_media;
mod production_media_lifecycle;
mod registry;
mod rest;
mod runtime;
mod shutdown;
mod voice_facts;
mod web_authorizer;

pub use component_id::{ComponentAction, ComponentIdCodec, ComponentIdError, DecodedComponentId};
pub use components::{
    HrirOption, build_ephemeral_status_panel, build_now_panel, build_status_panel,
};
pub use config::{BotConfig, ConfigError, ShardConfig, ShardConfigError};
pub use error::BotError;
pub use guild_lifecycle::{GuildLifecycleError, GuildLifecycleHandle};
pub use guild_policy::{
    GuildControlPolicy, GuildPolicyError, GuildPolicyProvider, NoopGuildPolicy,
};
pub use interaction::{InteractionInput, InteractionMapError, map_interaction};
pub use media::{AttachmentSource, MediaResolver, ResolveError, ResolvedMediaBatch};
pub use permissions::{
    ControlPolicy, VoiceContext, VoicePolicyError, authorize_guild_control, authorize_voice_control,
};
pub use registry::{PlayerFactory, PlayerRegistry, RegistryError};
pub use rest::{ComponentsV2Responder, DiscordComponentsV2Rest, RestBoundaryError};
pub use runtime::{BotData, commands as application_commands, run, run_with_data};

pub type CommandError = Box<dyn std::error::Error + Send + Sync>;

pub type Context<'a> = poise::Context<'a, BotData, CommandError>;
