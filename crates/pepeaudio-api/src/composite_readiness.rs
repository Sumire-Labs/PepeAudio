use std::sync::Arc;

use crate::{BoxPortFuture, PortError, ReadinessProbe};

#[derive(Clone, Default)]
pub struct CompositeReadiness {
    probes: Arc<[Arc<dyn ReadinessProbe>]>,
}

impl CompositeReadiness {
    #[must_use]
    pub fn new(probes: impl IntoIterator<Item = Arc<dyn ReadinessProbe>>) -> Self {
        Self {
            probes: probes.into_iter().collect(),
        }
    }
}

impl ReadinessProbe for CompositeReadiness {
    fn ready(&self) -> BoxPortFuture<'_, Result<(), PortError>> {
        Box::pin(async move {
            for probe in self.probes.iter() {
                probe.ready().await?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CompositeReadiness;
    use crate::{BoxPortFuture, PortError, ReadinessProbe};
    use std::sync::Arc;

    struct Fixed(Result<(), PortError>);

    impl ReadinessProbe for Fixed {
        fn ready(&self) -> BoxPortFuture<'_, Result<(), PortError>> {
            let result = self.0;
            Box::pin(async move { result })
        }
    }

    #[tokio::test]
    async fn requires_every_dependency() {
        let ready = CompositeReadiness::new([
            Arc::new(Fixed(Ok(()))) as Arc<dyn ReadinessProbe>,
            Arc::new(Fixed(Err(PortError::Unavailable))),
        ]);
        assert_eq!(ready.ready().await, Err(PortError::Unavailable));
    }
}
