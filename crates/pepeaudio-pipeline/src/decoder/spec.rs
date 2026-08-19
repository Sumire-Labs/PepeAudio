use std::{ffi::OsString, path::Path, time::Duration};

use pepeaudio_media::CommandSpec;

pub(super) fn ffmpeg_spec(
    program: &Path,
    source: &Path,
    start_offset: Duration,
    maximum_duration: Duration,
) -> CommandSpec {
    CommandSpec::new(
        program,
        vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-nostats".into(),
            "-nostdin".into(),
            "-protocol_whitelist".into(),
            "file,pipe".into(),
            "-ss".into(),
            offset_argument(start_offset),
            "-i".into(),
            source.as_os_str().to_os_string(),
            "-t".into(),
            offset_argument(maximum_duration),
            "-map".into(),
            "0:a:0".into(),
            "-vn".into(),
            "-sn".into(),
            "-dn".into(),
            "-f".into(),
            "f32le".into(),
            "-acodec".into(),
            "pcm_f32le".into(),
            "-ar".into(),
            "48000".into(),
            "-ac".into(),
            "2".into(),
            "pipe:1".into(),
        ],
    )
}

fn offset_argument(offset: Duration) -> OsString {
    format!("{}.{:09}", offset.as_secs(), offset.subsec_nanos()).into()
}
