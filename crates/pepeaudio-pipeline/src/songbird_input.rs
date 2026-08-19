use std::{
    io::{self, SeekFrom},
    pin::Pin,
    task::{Context, Poll},
};

use async_trait::async_trait;
use songbird::input::{AsyncAdapterStream, AsyncMediaSource, AudioStreamError, Input, RawAdapter};
use tokio::io::{AsyncRead, AsyncSeek, DuplexStream, ReadBuf};

const SAMPLE_RATE_HZ: u32 = 48_000;
const CHANNELS: u32 = 2;

pub(crate) fn songbird_pcm_input(reader: DuplexStream, buffer_bytes: usize) -> Input {
    let source = AsyncAdapterStream::new(Box::new(PcmPipeReader { reader }), buffer_bytes);
    RawAdapter::new(source, SAMPLE_RATE_HZ, CHANNELS).into()
}

struct PcmPipeReader {
    reader: DuplexStream,
}

impl AsyncRead for PcmPipeReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.reader).poll_read(context, buffer)
    }
}

impl AsyncSeek for PcmPipeReader {
    fn start_seek(self: Pin<&mut Self>, _: SeekFrom) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "live PCM is not seekable",
        ))
    }

    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<u64>> {
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "live PCM is not seekable",
        )))
    }
}

#[async_trait]
impl AsyncMediaSource for PcmPipeReader {
    fn is_seekable(&self) -> bool {
        false
    }

    async fn byte_len(&self) -> Option<u64> {
        None
    }

    async fn try_resume(&mut self, _: u64) -> Result<Box<dyn AsyncMediaSource>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }
}

#[cfg(test)]
mod tests;
