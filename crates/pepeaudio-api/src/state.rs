use std::sync::Arc;

use crate::{
    ApiConfig, ApiShutdown, Authorizer, Clock, CommandResultSource, CommandRouter,
    HrirPresetCatalogSource, PlayerEventSource, PrincipalAuthenticator, ReadinessProbe,
    SnapshotSource,
};

#[derive(Clone)]
pub struct AppState {
    pub(crate) config: ApiConfig,
    pub(crate) authenticator: Arc<dyn PrincipalAuthenticator>,
    pub(crate) authorizer: Arc<dyn Authorizer>,
    pub(crate) snapshots: Arc<dyn SnapshotSource>,
    pub(crate) hrir_presets: Arc<dyn HrirPresetCatalogSource>,
    pub(crate) commands: Arc<dyn CommandRouter>,
    pub(crate) command_results: Arc<dyn CommandResultSource>,
    pub(crate) events: Arc<dyn PlayerEventSource>,
    pub(crate) readiness: Arc<dyn ReadinessProbe>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) shutdown: ApiShutdown,
    pub(crate) sse_admission: crate::sse_admission::SseAdmission,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        config: ApiConfig,
        authenticator: Arc<dyn PrincipalAuthenticator>,
        authorizer: Arc<dyn Authorizer>,
        snapshots: Arc<dyn SnapshotSource>,
        hrir_presets: Arc<dyn HrirPresetCatalogSource>,
        commands: Arc<dyn CommandRouter>,
        command_results: Arc<dyn CommandResultSource>,
        events: Arc<dyn PlayerEventSource>,
        readiness: Arc<dyn ReadinessProbe>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            config,
            authenticator,
            authorizer,
            snapshots,
            hrir_presets,
            commands,
            command_results,
            events,
            readiness,
            clock,
            shutdown: ApiShutdown::new(),
            sse_admission: crate::sse_admission::SseAdmission::production(),
        }
    }

    #[must_use]
    pub fn with_shutdown(mut self, shutdown: ApiShutdown) -> Self {
        self.shutdown = shutdown;
        self
    }
}
