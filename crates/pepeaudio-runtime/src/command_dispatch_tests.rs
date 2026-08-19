mod support;

use pepeaudio_core::CommandResultStatus;

use self::support::{
    DispatchStore, LaneAuthorizer, StoreEvent, command, fast_id, guild, second_id, slow_id,
    spawn_worker, within,
};

#[tokio::test(start_paused = true)]
async fn slow_guild_does_not_block_another_guild_and_preserves_its_own_order() {
    let slow_id = slow_id();
    let second_id = second_id();
    let fast_id = fast_id();
    let store = DispatchStore::new(vec![
        command("1-0", slow_id, guild(10)),
        command("2-0", second_id, guild(10)),
        command("3-0", fast_id, guild(20)),
    ]);
    let authorizer = LaneAuthorizer::new(slow_id, second_id);
    let (shutdown, worker) = spawn_worker(store.clone(), authorizer.clone());

    within(authorizer.wait_for_slow(), "slow command did not start").await;
    within(
        store.wait_for_acknowledgement("3-0"),
        "another guild was head-of-line blocked",
    )
    .await;
    assert!(!authorizer.observed().contains(&second_id));

    let events = store.events();
    let result_index = events
        .iter()
        .position(|event| *event == StoreEvent::Result(fast_id))
        .expect("fast terminal result");
    let ack_index = events
        .iter()
        .position(|event| *event == StoreEvent::Acknowledged("3-0".into()))
        .expect("fast acknowledgement");
    assert!(result_index < ack_index, "result must be stored before ACK");

    authorizer.release_slow();
    within(
        authorizer.wait_for_second(),
        "second same-guild command did not start",
    )
    .await;
    within(
        store.wait_for_acknowledgement("2-0"),
        "second same-guild command did not finish",
    )
    .await;

    shutdown.send(true).expect("worker shutdown receiver");
    within(
        async { worker.await.expect("worker task") },
        "worker did not stop",
    )
    .await;
}

#[tokio::test(start_paused = true)]
async fn shutdown_cancels_a_stalled_guild_without_acknowledging_it() {
    let slow_id = slow_id();
    let store = DispatchStore::new(vec![command("1-0", slow_id, guild(10))]);
    let authorizer = LaneAuthorizer::new(slow_id, second_id());
    let (shutdown, worker) = spawn_worker(store.clone(), authorizer.clone());

    within(authorizer.wait_for_slow(), "slow command did not start").await;
    shutdown.send(true).expect("worker shutdown receiver");
    within(
        async { worker.await.expect("worker task") },
        "stalled command blocked worker shutdown",
    )
    .await;

    assert!(matches!(
        store.result_status(slow_id),
        CommandResultStatus::Pending
    ));
    assert!(
        store.events().is_empty(),
        "cancelled work must remain unacked"
    );
}
