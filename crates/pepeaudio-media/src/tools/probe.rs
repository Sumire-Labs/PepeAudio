use std::{path::Path, path::PathBuf};

use async_trait::async_trait;
use serde::Deserialize;

use super::{CommandSpec, OutputLimits, ProcessRunner};
use crate::{MediaProbe, ProbeMetadata, ProbeStream, ProcessError};

/// JSON `ffprobe` wrapper using a fakeable bounded process runner.
#[derive(Clone, Debug)]
pub struct Ffprobe<R> {
    program: PathBuf,
    runner: R,
    limits: OutputLimits,
}

impl<R> Ffprobe<R> {
    #[must_use]
    pub fn new(program: impl Into<PathBuf>, runner: R, limits: OutputLimits) -> Self {
        Self {
            program: program.into(),
            runner,
            limits,
        }
    }

    /// Builds the exact non-shell invocation used for probing.
    #[must_use]
    pub fn command_spec(&self, path: &Path) -> CommandSpec {
        CommandSpec::new(
            self.program.clone(),
            vec![
                "-v".into(),
                "error".into(),
                "-protocol_whitelist".into(),
                "file,pipe".into(),
                "-show_entries".into(),
                "format=duration,format_name:stream=index,codec_type,codec_name,sample_rate,channels,channel_layout".into(),
                "-of".into(),
                "json".into(),
                "-i".into(),
                path.as_os_str().to_os_string(),
            ],
        )
    }
}

#[async_trait]
impl<R> MediaProbe for Ffprobe<R>
where
    R: ProcessRunner,
{
    async fn probe(&self, path: &Path) -> Result<ProbeMetadata, ProcessError> {
        if !self.limits.is_valid() {
            return Err(ProcessError::InvalidConfig);
        }
        let output = self
            .runner
            .run(&self.command_spec(path), self.limits)
            .await?;
        parse_probe(&output.stdout)
    }
}

#[derive(Debug, Deserialize)]
struct RawProbe {
    #[serde(default)]
    streams: Vec<RawStream>,
    format: Option<RawFormat>,
}

#[derive(Debug, Deserialize)]
struct RawFormat {
    format_name: Option<String>,
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u16>,
    channel_layout: Option<String>,
}

fn parse_probe(json: &[u8]) -> Result<ProbeMetadata, ProcessError> {
    let raw: RawProbe = serde_json::from_slice(json).map_err(|_| ProcessError::InvalidProbe)?;
    let audio_streams: Vec<_> = raw
        .streams
        .into_iter()
        .filter(|stream| stream.codec_type.as_deref() == Some("audio"))
        .map(|stream| ProbeStream {
            index: stream.index,
            codec_name: stream.codec_name,
            sample_rate_hz: parse_positive_integer(stream.sample_rate),
            channels: stream.channels.filter(|channels| *channels > 0),
            channel_layout: stream.channel_layout,
        })
        .collect();
    if audio_streams.is_empty() {
        return Err(ProcessError::NoAudioStream);
    }
    let (format_name, duration_seconds) = raw.format.map_or((None, None), |format| {
        (
            format.format_name,
            format
                .duration
                .and_then(|duration| duration.parse::<f64>().ok())
                .filter(|duration| duration.is_finite() && *duration >= 0.0),
        )
    });
    Ok(ProbeMetadata {
        format_name,
        duration_seconds,
        audio_streams,
    })
}

fn parse_positive_integer(value: Option<String>) -> Option<u32> {
    value
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
}
