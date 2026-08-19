mod common;

use std::time::Duration;

use pepeaudio_core::{PlayerCommand, PlayerState};
use pepeaudio_player::{PlayerError, PlayerEvent};

use common::{command, connect, harness, revision, track};

const IDLE_TIMEOUT: Duration = Duration::from_mins(5);

#[tokio::test(start_paused = true)]
async fn enqueue_at_299_seconds_invalidates_the_old_idle_timer() {
    let test = harness(IDLE_TIMEOUT, 4);
    connect(&test.handle).await;

    tokio::time::advance(Duration::from_secs(299)).await;
    let playing = test
        .handle
        .enqueue(track("rescued"), revision(1))
        .await
        .expect("enqueue before deadline succeeds");
    assert_eq!(playing.state, PlayerState::Playing);

    tokio::time::advance(Duration::from_secs(2)).await;
    tokio::task::yield_now().await;
    let snapshot = test.handle.snapshot().await.expect("actor is alive");
    assert_eq!(snapshot.state, PlayerState::Playing);
    assert!(snapshot.voice_channel_id.is_some());

    test.runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test(start_paused = true)]
async fn continuously_idle_player_disconnects_at_300_seconds() {
    let test = harness(IDLE_TIMEOUT, 4);
    let mut events = test.handle.subscribe();
    connect(&test.handle).await;

    tokio::time::advance(Duration::from_secs(299)).await;
    assert!(
        test.handle
            .snapshot()
            .await
            .expect("snapshot")
            .voice_channel_id
            .is_some()
    );

    tokio::time::advance(Duration::from_secs(1)).await;
    wait_for_idle_disconnect(&mut events).await;
    assert!(matches!(
        test.handle.snapshot().await,
        Err(PlayerError::ActorStopped)
    ));
    wait_for_event_stream_close(&mut events).await;

    test.runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test(start_paused = true)]
async fn paused_track_is_not_idle() {
    let test = harness(IDLE_TIMEOUT, 4);
    connect(&test.handle).await;
    test.handle
        .enqueue(track("paused"), revision(1))
        .await
        .expect("enqueue");
    let paused = test
        .handle
        .apply(command(2, PlayerCommand::Pause))
        .await
        .expect("pause");
    assert_eq!(paused.state, PlayerState::Paused);

    tokio::time::advance(IDLE_TIMEOUT + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    let snapshot = test.handle.snapshot().await.expect("snapshot");
    assert_eq!(snapshot.state, PlayerState::Paused);
    assert!(snapshot.voice_channel_id.is_some());
    assert!(
        !test
            .playback
            .calls()
            .await
            .contains(&common::PlaybackCall::Disconnect)
    );

    test.runtime.shutdown().await.expect("clean shutdown");
}

#[tokio::test(start_paused = true)]
async fn snapshot_position_advances_only_while_playing() {
    let test = harness(IDLE_TIMEOUT, 4);
    connect(&test.handle).await;
    test.handle
        .enqueue(track("clocked"), revision(1))
        .await
        .expect("enqueue");

    tokio::time::advance(Duration::from_secs(10)).await;
    let playing = test.handle.snapshot().await.expect("playing snapshot");
    assert_eq!(
        playing.current_track.expect("current track").position_ms,
        10_000
    );

    let paused = test
        .handle
        .apply(command(2, PlayerCommand::Pause))
        .await
        .expect("pause");
    assert_eq!(
        paused.current_track.expect("paused track").position_ms,
        10_000
    );
    tokio::time::advance(Duration::from_secs(5)).await;
    let still_paused = test.handle.snapshot().await.expect("paused snapshot");
    assert_eq!(
        still_paused
            .current_track
            .expect("still paused track")
            .position_ms,
        10_000
    );

    test.handle
        .apply(command(3, PlayerCommand::Play))
        .await
        .expect("resume");
    tokio::time::advance(Duration::from_secs(2)).await;
    let resumed = test.handle.snapshot().await.expect("resumed snapshot");
    assert_eq!(
        resumed.current_track.expect("resumed track").position_ms,
        12_000
    );

    test.runtime.shutdown().await.expect("clean shutdown");
}

async fn wait_for_idle_disconnect(events: &mut tokio::sync::broadcast::Receiver<PlayerEvent>) {
    loop {
        if matches!(
            events.recv().await.expect("event channel remains open"),
            PlayerEvent::IdleDisconnected { .. }
        ) {
            return;
        }
    }
}

async fn wait_for_event_stream_close(events: &mut tokio::sync::broadcast::Receiver<PlayerEvent>) {
    loop {
        match events.recv().await {
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}
