use std::time::Duration;

use pepeaudio_core::PlayerCommand;
use uuid::Uuid;

use crate::common::{PlaybackCall, command, connect, harness, revision, track};

#[tokio::test]
async fn shuffle_reorders_upcoming_tracks_once_per_idempotency_key() {
    let test = harness(Duration::from_mins(5), 8);
    connect(&test.handle).await;
    let current = track("current");
    let first = track("first");
    let second = track("second");
    let third = track("third");
    let expected_plays = [
        current.track_id,
        third.track_id,
        first.track_id,
        second.track_id,
    ];

    test.handle
        .enqueue(current, revision(1))
        .await
        .expect("current enqueue");
    test.handle
        .enqueue(first, revision(2))
        .await
        .expect("first queued");
    test.handle
        .enqueue(second, revision(3))
        .await
        .expect("second queued");
    test.handle
        .enqueue(third, revision(4))
        .await
        .expect("third queued");

    let mut shuffle = command(5, PlayerCommand::SetShuffle { enabled: true });
    let mut seed_bytes = [0_u8; 16];
    seed_bytes[..8].copy_from_slice(&42_u64.to_le_bytes());
    shuffle.idempotency_key = Uuid::from_bytes(seed_bytes);
    let shuffled = test.handle.apply(shuffle.clone()).await.expect("shuffle");
    assert!(shuffled.shuffle_enabled);
    assert_eq!(shuffled.revision, revision(6));
    assert_eq!(
        shuffled
            .upcoming_tracks
            .iter()
            .map(|track| track.title.as_str())
            .collect::<Vec<_>>(),
        vec!["third", "first", "second"]
    );

    let duplicate = test.handle.apply(shuffle).await.expect("idempotent replay");
    assert_eq!(duplicate.revision, revision(6));

    for expected_revision in 6..9 {
        test.handle
            .apply(command(expected_revision, PlayerCommand::Skip))
            .await
            .expect("skip shuffled queue");
    }
    let actual_plays: Vec<_> = test
        .playback
        .calls()
        .await
        .into_iter()
        .filter_map(|call| match call {
            PlaybackCall::Play(track_id) => Some(track_id),
            _ => None,
        })
        .collect();
    assert_eq!(actual_plays, expected_plays);

    test.runtime.shutdown().await.expect("clean shutdown");
}
