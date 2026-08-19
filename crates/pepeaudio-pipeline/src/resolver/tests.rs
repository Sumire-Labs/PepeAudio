use pepeaudio_player::{PlaybackSource, QueueTrack};

use super::{ManagedMediaResolver, TrackResolver};
use crate::PipelineError;

#[tokio::test]
async fn resolves_only_regular_files_beneath_canonical_root() {
    let base = std::env::temp_dir().join(format!("pepeaudio-pipeline-{}", uuid::Uuid::new_v4()));
    let managed = base.join("managed");
    let inside = managed.join("object");
    let outside = base.join("outside");
    tokio::fs::create_dir_all(&managed)
        .await
        .expect("create managed root");
    tokio::fs::write(&inside, b"media")
        .await
        .expect("write inside object");
    tokio::fs::write(&outside, b"media")
        .await
        .expect("write outside object");

    let resolver = ManagedMediaResolver::new(&managed).await.expect("resolver");
    let accepted = track("object");
    let selected = resolver.resolve(&accepted).await.expect("inside file");
    assert_eq!(
        selected.path(),
        tokio::fs::canonicalize(&inside)
            .await
            .expect("canonical inside")
    );

    let escaped = track(outside.to_string_lossy());
    assert!(matches!(
        resolver.resolve(&escaped).await,
        Err(PipelineError::InvalidSource)
    ));

    tokio::fs::remove_dir_all(&base)
        .await
        .expect("remove exact test directory");
}

#[tokio::test]
async fn rejects_file_as_managed_root() {
    let file = std::env::temp_dir().join(format!("pepeaudio-root-{}", uuid::Uuid::new_v4()));
    tokio::fs::write(&file, b"not a directory")
        .await
        .expect("write root candidate");
    assert!(matches!(
        ManagedMediaResolver::new(&file).await,
        Err(PipelineError::InvalidSource)
    ));
    tokio::fs::remove_file(file)
        .await
        .expect("remove test file");
}

fn track(source: impl Into<String>) -> QueueTrack {
    QueueTrack::new("test", None, None, true, PlaybackSource::new(source))
}
