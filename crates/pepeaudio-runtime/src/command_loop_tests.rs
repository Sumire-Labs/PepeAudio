mod support;

use std::sync::atomic::{AtomicUsize, Ordering};

use pepeaudio_core::{
    CommandResultCode, CommandResultStatus, GuildId, StateRevision, UnixTimeMillis,
};
use pepeaudio_player::{NoopPlayback, NoopSnapshotPublisher, PlayerConfig, spawn_player};
use pepeaudio_storage::DedupeClaim;

use self::support::{
    TestAuthorizer, TestDirectory, TestStore, authorizer, empty_directory, process,
    process_with_deadline,
};
use crate::CommandAuthorization;

#[tokio::test]
async fn allowed_command_completes_and_acknowledges_after_player_apply() {
    let runtime = spawn_player(
        GuildId::new(10).expect("guild"),
        PlayerConfig::default(),
        NoopPlayback,
        NoopSnapshotPublisher,
    );
    let store = TestStore::new(DedupeClaim::Acquired);
    let directory = TestDirectory {
        player: Some(runtime.handle()),
        lookups: AtomicUsize::new(0),
    };
    let authorizer = TestAuthorizer {
        outcome: CommandAuthorization::Allowed,
        calls: AtomicUsize::new(0),
    };

    process(&store, &directory, &authorizer).await;

    {
        let state = store.state.lock().expect("store lock");
        assert_eq!((state.claims, state.completions, state.releases), (1, 1, 0));
        assert_eq!(state.acknowledgements, ["1-0"]);
        assert_eq!(
            state.result.status,
            CommandResultStatus::Applied {
                resulting_revision: StateRevision::new(1)
            }
        );
    }
    assert_eq!(directory.lookups.load(Ordering::SeqCst), 1);
    assert_eq!(authorizer.calls.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.expect("player shutdown");
}

#[tokio::test]
async fn denied_command_records_a_sanitized_result_before_acknowledgement() {
    let store = TestStore::new(DedupeClaim::Acquired);
    let directory = empty_directory();
    let authorizer = authorizer(CommandAuthorization::Denied);

    process(&store, &directory, &authorizer).await;

    let state = store.state.lock().expect("store lock");
    assert_eq!((state.claims, state.completions, state.releases), (0, 0, 0));
    assert_eq!(state.acknowledgements, ["1-0"]);
    assert_eq!(
        state.result.status,
        CommandResultStatus::Denied {
            code: CommandResultCode::NotAuthorized
        }
    );
    assert_eq!(directory.lookups.load(Ordering::SeqCst), 0);
    assert_eq!(authorizer.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn transient_authorization_failure_remains_pending_without_claim() {
    let store = TestStore::new(DedupeClaim::Acquired);
    let directory = empty_directory();
    let authorizer = authorizer(CommandAuthorization::RetryableFailure);

    process(&store, &directory, &authorizer).await;

    let state = store.state.lock().expect("store lock");
    assert_eq!((state.claims, state.completions, state.releases), (0, 0, 0));
    assert!(state.acknowledgements.is_empty());
    assert_eq!(directory.lookups.load(Ordering::SeqCst), 0);
    assert_eq!(authorizer.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn expired_command_is_terminal_before_authorization_dependencies() {
    let store = TestStore::new(DedupeClaim::Acquired);
    let directory = empty_directory();
    let authorizer = authorizer(CommandAuthorization::RetryableFailure);

    process_with_deadline(&store, &directory, &authorizer, UnixTimeMillis::new(1)).await;

    let state = store.state.lock().expect("store lock");
    assert_eq!(
        state.result.status,
        CommandResultStatus::Rejected {
            code: CommandResultCode::DeadlineExpired,
            current_revision: None,
        }
    );
    assert_eq!(state.acknowledgements, ["1-0"]);
    assert_eq!(authorizer.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn completed_replay_is_reauthorized_before_acknowledgement() {
    let store = TestStore::new(DedupeClaim::Completed);
    let directory = empty_directory();
    let authorizer = authorizer(CommandAuthorization::Allowed);

    process(&store, &directory, &authorizer).await;

    let state = store.state.lock().expect("store lock");
    assert_eq!(state.claims, 1);
    assert_eq!(state.acknowledgements, ["1-0"]);
    assert_eq!(
        state.result.status,
        CommandResultStatus::Rejected {
            code: CommandResultCode::IdempotencyReplayed,
            current_revision: None,
        }
    );
    assert_eq!(directory.lookups.load(Ordering::SeqCst), 0);
    assert_eq!(authorizer.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn terminal_result_storage_failure_prevents_acknowledgement() {
    let store = TestStore::new(DedupeClaim::Acquired);
    store.fail_result_writes();
    let directory = empty_directory();
    let authorizer = authorizer(CommandAuthorization::Denied);

    process(&store, &directory, &authorizer).await;

    let state = store.state.lock().expect("store lock");
    assert!(matches!(state.result.status, CommandResultStatus::Pending));
    assert_eq!(state.result_writes, 1);
    assert!(state.acknowledgements.is_empty());
}

#[tokio::test]
async fn applied_result_storage_failure_also_prevents_acknowledgement() {
    let runtime = spawn_player(
        GuildId::new(10).expect("guild"),
        PlayerConfig::default(),
        NoopPlayback,
        NoopSnapshotPublisher,
    );
    let store = TestStore::new(DedupeClaim::Acquired);
    store.fail_result_writes();
    let directory = TestDirectory {
        player: Some(runtime.handle()),
        lookups: AtomicUsize::new(0),
    };
    let authorizer = authorizer(CommandAuthorization::Allowed);

    process(&store, &directory, &authorizer).await;

    {
        let state = store.state.lock().expect("store lock");
        assert!(matches!(state.result.status, CommandResultStatus::Pending));
        assert_eq!(state.completions, 1);
        assert!(state.acknowledgements.is_empty());
    }
    runtime.shutdown().await.expect("player shutdown");
}

#[tokio::test]
async fn transient_player_lookup_failure_releases_the_lease_and_stays_pending() {
    let store = TestStore::new(DedupeClaim::Acquired);
    let directory = empty_directory();
    let authorizer = authorizer(CommandAuthorization::Allowed);

    process(&store, &directory, &authorizer).await;

    let state = store.state.lock().expect("store lock");
    assert!(matches!(state.result.status, CommandResultStatus::Pending));
    assert_eq!((state.claims, state.completions, state.releases), (1, 0, 1));
    assert!(state.acknowledgements.is_empty());
}
