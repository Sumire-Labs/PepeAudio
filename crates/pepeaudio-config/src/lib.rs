//! Typed, validated runtime configuration for `PepeAudio` services.
//!
//! Configuration is deliberately loaded through [`ConfigSource`]. Production
//! uses [`Environment`], while tests and embedding applications can provide a
//! deterministic source without mutating process-global environment variables.

mod api_runtime;
mod catalog;
mod error;
mod load;
mod model;
mod secret;
mod source;
mod validate;

pub use api_runtime::ApiRuntimeConfig;
pub use catalog::{AppleMusicCatalogConfig, CatalogConfig, SpotifyCatalogConfig};
pub use error::{ConfigError, ConfigResult, ShardConfigError};
pub use model::{
    AppConfig, BotRuntimeConfig, DiscordConfig, PlayerLimits, ServiceConfig, ShardConfig,
    ToolConfig,
};
pub use secret::SecretString;
pub use source::{ConfigSource, Environment, MapSource};
