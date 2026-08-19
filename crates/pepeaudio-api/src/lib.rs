//! Axum HTTP/SSE adapter for the `PepeAudio` domain.
//!
//! Production Discord `OAuth2` and opaque-session details live in the sibling
//! `pepeaudio-auth` adapter. [`DevHeaderAuthenticator`] remains an explicit
//! development and test mechanism, not a secure deployment substitute.

#![forbid(unsafe_code)]

mod auth;
mod composite_readiness;
mod config;
pub mod dev;
mod error;
mod handlers;
mod hrir_catalog;
mod ports;
mod router;
mod shutdown;
mod sse;
mod sse_admission;
mod state;
mod validation;

pub use auth::{
    AuthenticationError, CSRF_HEADER, DEV_USER_HEADER, DevAuthConfigError, DevHeaderAuthenticator,
    Principal, PrincipalAuthenticator, PrincipalConfigError, SESSION_COOKIE, SessionAuthenticator,
    SessionFingerprint, SessionRecord, SessionStore,
};
pub use composite_readiness::CompositeReadiness;
pub use config::{ApiConfig, ConfigError, DEFAULT_COMMAND_TTL};
pub use error::{ApiError, ErrorBody, ErrorDocument};
pub use hrir_catalog::{HrirPresetCatalog, HrirPresetSummary, HrirSourceMetadata};
pub use ports::{
    Access, AuthorizationError, Authorizer, BoxPortFuture, Clock, CommandReceipt,
    CommandResultSource, CommandRouter, HrirPresetCatalogSource, PlayerEvent, PlayerEventSource,
    PortError, ReadinessProbe, RouteError, SnapshotSource, SystemClock,
};
pub use router::build_router;
pub use shutdown::ApiShutdown;
pub use state::AppState;
