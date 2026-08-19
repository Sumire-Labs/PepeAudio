use super::*;
use crate::provider::apple::wire::{Resource, SongAttributes};

fn song(url: Option<&str>) -> Resource<SongAttributes> {
    Resource {
        id: "1440833542".to_owned(),
        object_type: "songs".to_owned(),
        attributes: Some(SongAttributes {
            name: Some("Example Song".to_owned()),
            artist_name: Some("Example Artist".to_owned()),
            album_name: None,
            duration_in_millis: None,
            isrc: None,
            url: url.map(str::to_owned),
        }),
    }
}

#[test]
fn song_requires_a_matching_official_sharing_url() {
    assert!(song_metadata(song(None), "jp").is_none());
    assert!(
        song_metadata(
            song(Some(
                "https://attacker.invalid/jp/album/example/1440833098?i=1440833542"
            )),
            "jp"
        )
        .is_none()
    );
    assert!(
        song_metadata(
            song(Some(
                "https://music.apple.com/jp/album/example/1440833098?i=9999999999"
            )),
            "jp"
        )
        .is_none()
    );
}

#[test]
fn song_retains_the_validated_sharing_url() {
    let metadata = song_metadata(
        song(Some(
            "https://music.apple.com/jp/album/example/1440833098?i=1440833542",
        )),
        "jp",
    )
    .expect("valid metadata");

    assert_eq!(
        metadata.reference.canonical_url().as_str(),
        "https://music.apple.com/jp/album/example/1440833098?i=1440833542"
    );
}
