use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use pepeaudio_core::UserId;

/// Process-local SSE admission policy. Production replicas each enforce these
/// bounds independently; the reverse proxy must still apply its own socket
/// limits across replicas.
pub(crate) const MAX_SSE_CONNECTIONS: usize = 1_024;
pub(crate) const MAX_SSE_CONNECTIONS_PER_USER: usize = 8;

#[derive(Clone)]
pub(crate) struct SseAdmission {
    inner: Arc<Mutex<AdmissionState>>,
    global_limit: usize,
    user_limit: usize,
}

#[derive(Default)]
struct AdmissionState {
    active: usize,
    users: HashMap<UserId, usize>,
}

pub(crate) struct SseLease {
    inner: Arc<Mutex<AdmissionState>>,
    user_id: UserId,
}

impl SseAdmission {
    pub(crate) fn production() -> Self {
        Self::new(MAX_SSE_CONNECTIONS, MAX_SSE_CONNECTIONS_PER_USER)
    }

    fn new(global_limit: usize, user_limit: usize) -> Self {
        debug_assert!(global_limit > 0 && user_limit > 0 && user_limit <= global_limit);
        Self {
            inner: Arc::new(Mutex::new(AdmissionState::default())),
            global_limit,
            user_limit,
        }
    }

    pub(crate) fn acquire(&self, user_id: UserId) -> Option<SseLease> {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let user_active = state.users.get(&user_id).copied().unwrap_or_default();
        if state.active >= self.global_limit || user_active >= self.user_limit {
            return None;
        }
        state.active += 1;
        state.users.insert(user_id, user_active + 1);
        drop(state);
        Some(SseLease {
            inner: Arc::clone(&self.inner),
            user_id,
        })
    }
}

impl Drop for SseLease {
    fn drop(&mut self) {
        let mut state = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.active = state.active.saturating_sub(1);
        if let Some(active) = state.users.get_mut(&self.user_id) {
            *active = active.saturating_sub(1);
            if *active == 0 {
                state.users.remove(&self.user_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pepeaudio_core::UserId;

    use super::SseAdmission;

    #[test]
    fn leases_enforce_both_bounds_and_release_on_drop() {
        let admission = SseAdmission::new(2, 1);
        let first_user = UserId::new(1).expect("user");
        let second_user = UserId::new(2).expect("user");
        let first = admission.acquire(first_user).expect("first lease");
        assert!(admission.acquire(first_user).is_none());
        let second = admission.acquire(second_user).expect("second lease");
        assert!(admission.acquire(UserId::new(3).expect("user")).is_none());

        drop(first);
        assert!(admission.acquire(first_user).is_some());
        drop(second);
    }
}
