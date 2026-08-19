use std::{collections::HashMap, path::PathBuf, sync::Arc, sync::Mutex};

use super::{CapacityInner, CapacityState, CapacityTracker};

fn tracker(maximum_bytes: u64) -> CapacityTracker {
    CapacityTracker {
        inner: Some(Arc::new(CapacityInner {
            maximum_bytes,
            maximum_entries: 4,
            state: Mutex::new(CapacityState {
                charged_bytes: 0,
                entries: HashMap::new(),
                reservations: 0,
            }),
        })),
    }
}

#[test]
fn failed_file_cleanup_converts_reservation_to_retained_charge() {
    let tracker = tracker(16);
    let path = PathBuf::from("staging/opaque.part");
    let mut reservation = tracker.reserve(8).expect("reservation");

    reservation.retain_failed_file(path.clone());
    drop(reservation);

    let usage = tracker.usage().expect("usage");
    assert_eq!(usage.used_bytes, 8);
    assert_eq!(usage.reserved_bytes, 0);
    assert_eq!(usage.managed_files, 1);
    assert_eq!(usage.reservations, 0);

    tracker.removed(&path);
    assert_eq!(tracker.charged_bytes(), Some(0));
}
