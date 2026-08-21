use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use pepeaudio_core::{ChannelId, GuildId, UserId};
use pepeaudio_player::{
    NoopPlayback, NoopSnapshotPublisher, PlaybackSource, PlayerConfig, QueueTrack, spawn_player,
};
use pepeaudio_storage::ControlPolicy as StoredControlPolicy;

use super::{
    MAX_COMMIT_ATTEMPTS, PlayInputError, PlaySource, authorize_play, authorized_player,
    commit_resolved_tracks, import_notice, play, retry_revision_conflicts, select_play_source,
};
use crate::{
    AttachmentSource, GuildControlPolicy, PlayerFactory, PlayerRegistry, RegistryError,
    ResolvedMediaBatch, VoiceContext,
};

struct CountingFactory(AtomicUsize);

#[async_trait]
impl PlayerFactory for CountingFactory {
    async fn create(
        &self,
        guild_id: GuildId,
    ) -> Result<pepeaudio_player::PlayerHandle, RegistryError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        let runtime = spawn_player(
            guild_id,
            PlayerConfig::default(),
            NoopPlayback,
            NoopSnapshotPublisher,
        );
        Ok(runtime.handle())
    }
}

fn example_attachment() -> AttachmentSource {
    AttachmentSource {
        filename: "track.opus".into(),
        url: "https://cdn.discordapp.com/attachments/1/2/track.opus".into(),
        content_type: Some("audio/ogg".into()),
        size_bytes: 42,
    }
}

#[test]
fn play_requires_exactly_one_source() {
    assert!(matches!(
        select_play_source(Some("https://example.com/audio".into()), None),
        Ok(PlaySource::Input(input)) if input == "https://example.com/audio"
    ));
    assert!(matches!(
        select_play_source(None, Some(example_attachment())),
        Ok(PlaySource::Attachment(source)) if source.filename == "track.opus"
    ));
    assert_eq!(select_play_source(None, None), Err(PlayInputError::Missing));
    assert_eq!(
        select_play_source(
            Some("https://example.com/audio".into()),
            Some(example_attachment()),
        ),
        Err(PlayInputError::Conflicting)
    );
}

#[test]
fn play_registers_one_command_with_optional_query_and_file_options() {
    let command = play();
    assert_eq!(command.name, "play");
    assert!(command.subcommands.is_empty());
    assert!(!command.subcommand_required);
    assert_eq!(command.parameters.len(), 2);

    let option = |name: &str| {
        let parameter = command
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .unwrap_or_else(|| panic!("missing {name} option"));
        assert!(!parameter.required);
        serde_json::to_value(
            parameter
                .create_as_slash_command_option()
                .expect("slash option builder"),
        )
        .expect("serializable slash option")
    };

    let query = option("query");
    assert_eq!(query["type"], 3);
    assert_eq!(query["required"], false);
    assert_eq!(query["max_length"], 4096);
    assert!(
        query["description"]
            .as_str()
            .is_some_and(|description| description.contains("/playは入力せず"))
    );

    let file = option("file");
    assert_eq!(file["type"], 11);
    assert_eq!(file["required"], false);
}

#[tokio::test]
async fn revoked_current_permissions_do_not_create_a_player() {
    let guild_id = GuildId::new(1).expect("guild");
    let channel = ChannelId::new(2).expect("channel");
    let policy = GuildControlPolicy {
        control: StoredControlPolicy::DjOnly,
        dj_role_id: Some(7),
    };
    let mut facts = VoiceContext {
        actor_user_id: UserId::new(3).expect("user"),
        actor_voice_channel_id: Some(channel),
        bot_voice_channel_id: None,
        has_manage_guild: true,
        has_dj_role: false,
    };
    assert!(authorize_play(facts, policy).is_ok());

    facts.has_manage_guild = false;
    let factory = Arc::new(CountingFactory(AtomicUsize::new(0)));
    let players = PlayerRegistry::new(factory.clone());
    let result = authorized_player(&players, guild_id, Ok((facts, policy))).await;

    assert!(result.is_err());
    assert_eq!(factory.0.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn enqueue_rechecks_permissions_after_player_initialization() {
    let guild_id = GuildId::new(11).expect("guild");
    let channel = ChannelId::new(12).expect("channel");
    let policy = GuildControlPolicy {
        control: StoredControlPolicy::DjOnly,
        dj_role_id: Some(17),
    };
    let mut facts = VoiceContext {
        actor_user_id: UserId::new(13).expect("user"),
        actor_voice_channel_id: Some(channel),
        bot_voice_channel_id: None,
        has_manage_guild: true,
        has_dj_role: false,
    };
    let factory = Arc::new(CountingFactory(AtomicUsize::new(0)));
    let players = PlayerRegistry::new(factory.clone());
    let player = authorized_player(&players, guild_id, Ok((facts, policy)))
        .await
        .expect("initial authorization creates the player");
    facts.has_manage_guild = false;
    let resolved = QueueTrack::new(
        "resolved",
        Some(facts.actor_user_id),
        Some(1_000),
        true,
        PlaybackSource::new("memory://resolved"),
    );

    let result = commit_resolved_tracks(&player, &[resolved], Ok((facts, policy))).await;

    assert!(result.is_err());
    assert_eq!(factory.0.load(Ordering::SeqCst), 1);
    let snapshot = player.snapshot().await.expect("unchanged player");
    assert!(snapshot.voice_channel_id.is_none());
    assert!(snapshot.current_track.is_none());
}

#[tokio::test]
async fn cache_disconnect_cannot_enqueue_into_a_stale_actor_channel() {
    let guild_id = GuildId::new(21).expect("guild");
    let channel = ChannelId::new(22).expect("channel");
    let user = UserId::new(23).expect("user");
    let factory = Arc::new(CountingFactory(AtomicUsize::new(0)));
    let players = PlayerRegistry::new(factory);
    let player = players.get_or_create(guild_id).await.expect("player actor");
    let before = player.snapshot().await.expect("initial snapshot");
    player
        .connect(channel, before.revision)
        .await
        .expect("actor connected before external kick");
    let facts = VoiceContext {
        actor_user_id: user,
        actor_voice_channel_id: Some(channel),
        bot_voice_channel_id: None,
        has_manage_guild: false,
        has_dj_role: false,
    };
    let track = QueueTrack::new(
        "resolved",
        Some(user),
        Some(1_000),
        true,
        PlaybackSource::new("memory://resolved"),
    );

    let result = commit_resolved_tracks(
        &player,
        &[track],
        Ok((facts, GuildControlPolicy::default())),
    )
    .await;

    assert!(result.is_err());
    let unchanged = player.snapshot().await.expect("actor remains available");
    assert!(unchanged.current_track.is_none());
    players.shutdown_all().await.expect("cleanup");
}

#[tokio::test]
async fn revision_conflicts_are_retried_but_other_failures_are_not() {
    let attempts = AtomicUsize::new(0);
    let result = retry_revision_conflicts(|| {
        let attempt = attempts.fetch_add(1, Ordering::SeqCst);
        async move {
            if attempt < 2 {
                Err(pepeaudio_player::PlayerError::RevisionConflict {
                    expected: attempt as u64,
                    actual: attempt as u64 + 1,
                }
                .into())
            } else {
                Ok(42_u8)
            }
        }
    })
    .await
    .expect("a later revision retry succeeds");

    assert_eq!(result, 42);
    assert_eq!(attempts.load(Ordering::SeqCst), 3);

    let permanent_attempts = AtomicUsize::new(0);
    let error = retry_revision_conflicts(|| {
        permanent_attempts.fetch_add(1, Ordering::SeqCst);
        async { Err::<(), _>(pepeaudio_player::PlayerError::NotConnected.into()) }
    })
    .await
    .expect_err("a permanent failure is returned immediately");
    assert!(
        error
            .downcast_ref::<pepeaudio_player::PlayerError>()
            .is_some()
    );
    assert_eq!(permanent_attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn revision_retries_are_bounded() {
    let attempts = AtomicUsize::new(0);
    let error = retry_revision_conflicts(|| {
        let attempt = attempts.fetch_add(1, Ordering::SeqCst);
        async move {
            Err::<(), _>(
                pepeaudio_player::PlayerError::RevisionConflict {
                    expected: attempt as u64,
                    actual: attempt as u64 + 1,
                }
                .into(),
            )
        }
    })
    .await
    .expect_err("continuous contention eventually returns to the caller");

    assert!(
        error
            .downcast_ref::<pepeaudio_player::PlayerError>()
            .is_some()
    );
    assert_eq!(attempts.load(Ordering::SeqCst), MAX_COMMIT_ATTEMPTS);
}

#[test]
fn import_notice_reports_processed_entries_instead_of_mislabeling_skips_as_tracks() {
    let tracks = vec![QueueTrack::new(
        "resolved",
        None,
        Some(1_000),
        true,
        PlaybackSource::new("memory://resolved"),
    )];
    let known_total = ResolvedMediaBatch {
        tracks: tracks.clone(),
        source_title: Some("Collection".into()),
        source_item_count: Some(40),
        skipped_items: 2,
        truncated: true,
    };
    let unknown_total = ResolvedMediaBatch {
        tracks,
        source_title: None,
        source_item_count: None,
        skipped_items: 2,
        truncated: true,
    };

    assert!(import_notice(&known_total).contains("先頭3件を処理"));
    assert!(import_notice(&unknown_total).contains("先頭3件までを処理"));
    assert!(!import_notice(&known_total).contains("先頭1曲"));
}

#[test]
fn import_notice_escapes_external_titles_that_look_like_links() {
    let batch = ResolvedMediaBatch {
        tracks: Vec::new(),
        source_title: Some("[公式](https://evil.example) <https://evil.example>".into()),
        source_item_count: Some(0),
        skipped_items: 0,
        truncated: false,
    };

    let notice = import_notice(&batch);

    assert!(notice.contains(r"\[公式\]\(https://evil.example\) \<https://evil.example\>"));
    assert!(!notice.contains("[公式](https://evil.example)"));
    assert!(!notice.contains("<https://evil.example>"));
}
