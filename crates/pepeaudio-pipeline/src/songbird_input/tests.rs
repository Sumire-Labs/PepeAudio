use songbird::input::codecs::{get_codec_registry, get_probe};
use tokio::io::AsyncWriteExt;

use super::songbird_pcm_input;

#[tokio::test]
async fn raw_pcm_is_playable_without_voice_or_network() {
    let (reader, mut writer) = tokio::io::duplex(64);
    let input = songbird_pcm_input(reader, 64);
    let producer = tokio::spawn(async move {
        writer.write_all(&[0_u8; 8]).await.expect("write PCM frame");
        writer.shutdown().await.expect("close PCM writer");
    });

    let playable = input
        .make_playable_async(get_codec_registry(), get_probe())
        .await
        .expect("Songbird must parse RawAdapter f32 PCM");
    assert!(playable.is_playable());
    producer.await.expect("PCM producer");
}
