use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use futures_util::{StreamExt, stream::FuturesUnordered};
use pepeaudio_player::QueueTrack;
use tokio::{
    sync::{Semaphore, SemaphorePermit},
    time::{Duration, Instant, timeout_at},
};

use crate::ResolveError;

pub(super) const SITE_BATCH_CONCURRENCY: usize = 4;
pub(super) const SITE_BATCH_ADMISSION_WINDOW: Duration = Duration::from_mins(5);

pub(super) enum ItemOutcome {
    Completed,
    Skipped(ResolveError),
}

pub(super) struct BatchOutcome {
    pub(super) fatal_error: Option<ResolveError>,
    pub(super) first_skipped_error: Option<ResolveError>,
    pub(super) skipped_items: usize,
}

pub(super) fn classify_item(result: Result<(), ResolveError>) -> Result<ItemOutcome, ResolveError> {
    match result {
        Ok(()) => Ok(ItemOutcome::Completed),
        Err(error)
            if matches!(
                &error,
                ResolveError::NoSearchMatch
                    | ResolveError::UnsupportedStream
                    | ResolveError::TrackLimitExceeded
            ) =>
        {
            Ok(ItemOutcome::Skipped(error))
        }
        Err(error) => Err(error),
    }
}

pub(super) fn admission_deadline() -> Instant {
    Instant::now() + SITE_BATCH_ADMISSION_WINDOW
}

pub(super) async fn acquire_until(
    semaphore: &Semaphore,
    deadline: Instant,
) -> Result<SemaphorePermit<'_>, ResolveError> {
    timeout_at(deadline, semaphore.acquire())
        .await
        .map_err(|_| ResolveError::TimedOut)?
        .map_err(|_| ResolveError::Failed("site resolver is shutting down".into()))
}

#[derive(Clone)]
pub(super) struct CompletedTracks(Arc<Mutex<Vec<(usize, QueueTrack)>>>);

impl CompletedTracks {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self(Arc::new(Mutex::new(Vec::with_capacity(capacity))))
    }

    pub(super) fn register(&self, index: usize, track: QueueTrack) {
        self.0
            .lock()
            .expect("completed site-track registry")
            .push((index, track));
    }

    pub(super) fn take(&self) -> Vec<QueueTrack> {
        self.0
            .lock()
            .expect("completed site-track registry")
            .drain(..)
            .map(|(_, track)| track)
            .collect()
    }

    pub(super) fn take_ordered(&self) -> Vec<QueueTrack> {
        let mut tracks = self
            .0
            .lock()
            .expect("completed site-track registry")
            .drain(..)
            .collect::<Vec<_>>();
        tracks.sort_unstable_by_key(|(index, _)| *index);
        tracks.into_iter().map(|(_, track)| track).collect()
    }
}

pub(super) async fn run_until_error<I, F, Fut>(items: Vec<I>, mut operation: F) -> BatchOutcome
where
    F: FnMut(usize, I) -> Fut,
    Fut: Future<Output = Result<ItemOutcome, ResolveError>>,
{
    let mut remaining = items.into_iter().enumerate();
    let mut in_flight = FuturesUnordered::new();
    for _ in 0..SITE_BATCH_CONCURRENCY {
        let Some((index, item)) = remaining.next() else {
            break;
        };
        in_flight.push(operation(index, item));
    }

    let mut first_error = None;
    let mut first_skipped_error = None;
    let mut skipped_items = 0_usize;
    while let Some(result) = in_flight.next().await {
        match result {
            Ok(ItemOutcome::Completed) => {}
            Ok(ItemOutcome::Skipped(error)) => {
                skipped_items = skipped_items.saturating_add(1);
                first_skipped_error.get_or_insert(error);
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
        if first_error.is_none()
            && let Some((index, item)) = remaining.next()
        {
            in_flight.push(operation(index, item));
        }
    }
    BatchOutcome {
        fatal_error: first_error,
        first_skipped_error,
        skipped_items,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::{ItemOutcome, SITE_BATCH_CONCURRENCY, classify_item, run_until_error};
    use crate::ResolveError;

    #[tokio::test]
    async fn first_failure_stops_unstarted_items_and_drains_started_work() {
        let started = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        let error = run_until_error((0..20).collect(), |index, _| {
            let started = Arc::clone(&started);
            let completed = Arc::clone(&completed);
            async move {
                started.fetch_add(1, Ordering::SeqCst);
                if index == 0 {
                    return Err(ResolveError::CapacityExceeded);
                }
                tokio::task::yield_now().await;
                completed.fetch_add(1, Ordering::SeqCst);
                Ok(ItemOutcome::Completed)
            }
        })
        .await;

        assert_eq!(error.fatal_error, Some(ResolveError::CapacityExceeded));
        assert_eq!(started.load(Ordering::SeqCst), SITE_BATCH_CONCURRENCY);
        assert_eq!(completed.load(Ordering::SeqCst), SITE_BATCH_CONCURRENCY - 1);
    }

    #[tokio::test]
    async fn item_failures_are_skipped_without_stopping_later_items() {
        let outcome = run_until_error((0..8).collect(), |index, _| async move {
            classify_item(if index == 1 || index == 5 {
                Err(ResolveError::NoSearchMatch)
            } else {
                Ok(())
            })
        })
        .await;

        assert_eq!(outcome.fatal_error, None);
        assert_eq!(outcome.skipped_items, 2);
        assert_eq!(
            outcome.first_skipped_error,
            Some(ResolveError::NoSearchMatch)
        );
    }

    #[tokio::test]
    async fn fatal_batch_drain_restores_all_shared_admission_permits() {
        let admission = Arc::new(tokio::sync::Semaphore::new(2));
        let outcome = run_until_error((0..12).collect(), |index, _| {
            let admission = Arc::clone(&admission);
            async move {
                let _permit = admission.acquire_owned().await.expect("open semaphore");
                tokio::task::yield_now().await;
                if index == 0 {
                    Err(ResolveError::CapacityExceeded)
                } else {
                    Ok(ItemOutcome::Completed)
                }
            }
        })
        .await;

        assert_eq!(outcome.fatal_error, Some(ResolveError::CapacityExceeded));
        assert_eq!(admission.available_permits(), 2);
    }
}
