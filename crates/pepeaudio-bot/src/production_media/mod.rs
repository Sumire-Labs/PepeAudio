mod catalog;
mod ingest;
mod metadata;
mod site;
mod site_batch;
mod validation;

use std::{path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use pepeaudio_catalog::{CatalogResolver, parse_catalog_url};
use pepeaudio_config::ToolConfig;
use pepeaudio_core::{GuildId, UserId};
use pepeaudio_media::{
    DiscordAttachment, DownloadStore, FetchLimits, Ffprobe, ManagedDownloadJanitor,
    ManagedMediaLeaseRegistry, MediaFetcher, MediaIngestor, MediaRequest, OutputLimits,
    ProcessPool, RealProcessRunner, ReqwestTransport, SiteProvider, TokioDnsResolver, YtDlpClient,
    YtDlpConfig,
};
use url::Url;

use crate::{AttachmentSource, MediaResolver, ResolveError, ResolvedMediaBatch};

type Ingestor = MediaIngestor<TokioDnsResolver, ReqwestTransport, Ffprobe<RealProcessRunner>>;

pub(crate) struct ProductionMediaResolver {
    direct_ingestor: Ingestor,
    site_ingestor: Ingestor,
    site_client: Option<YtDlpClient>,
    catalog_resolver: Option<CatalogResolver>,
    site_admission: Arc<tokio::sync::Semaphore>,
    site_batches: Arc<tokio::sync::Semaphore>,
    media_leases: ManagedMediaLeaseRegistry,
    media_janitor: Arc<ManagedDownloadJanitor>,
    maximum_direct_bytes: u64,
    maximum_site_bytes: u64,
    maximum_duration: Duration,
    maximum_playlist_items: usize,
    queue_capacity: usize,
}

impl ProductionMediaResolver {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        tools: &ToolConfig,
        maximum_direct_bytes: u64,
        maximum_site_bytes: u64,
        maximum_duration: Duration,
        maximum_playlist_items: usize,
        queue_capacity: usize,
        media_leases: ManagedMediaLeaseRegistry,
        media_janitor: Arc<ManagedDownloadJanitor>,
        catalog_resolver: Option<CatalogResolver>,
    ) -> Result<Self, ResolveError> {
        let store = DownloadStore::new(media_leases.clone())
            .map_err(|_| ResolveError::Failed("managed media capacity is unavailable".into()))?;
        let pool = ProcessPool::new(4)
            .map_err(|_| ResolveError::Failed("invalid media process limits".into()))?;
        let runner = RealProcessRunner::new(pool);
        let direct_ingestor = ingestor(tools, store.clone(), maximum_direct_bytes, runner.clone())?;
        let site_ingestor = ingestor(tools, store, maximum_site_bytes, runner.clone())?;
        let site_client = if tools.site_extractors_enabled {
            Some(
                YtDlpClient::new(
                    YtDlpConfig {
                        executable: tools.ytdlp_path.clone(),
                        deno_executable: tools.deno_path.clone(),
                        deno_directory: tools.deno_directory.clone(),
                        maximum_track_duration: maximum_duration,
                        maximum_playlist_items,
                    },
                    Arc::new(runner),
                )
                .map_err(|error| site::map_site_error(&error))?,
            )
        } else {
            None
        };
        Ok(Self {
            direct_ingestor,
            site_ingestor,
            site_client,
            catalog_resolver,
            site_admission: Arc::new(tokio::sync::Semaphore::new(4)),
            site_batches: Arc::new(tokio::sync::Semaphore::new(4)),
            media_leases,
            media_janitor,
            maximum_direct_bytes,
            maximum_site_bytes,
            maximum_duration,
            maximum_playlist_items,
            queue_capacity,
        })
    }

    pub(crate) async fn verify_site_tools(&self) -> Result<(), ResolveError> {
        if let Some(client) = &self.site_client {
            client
                .verify_tools()
                .await
                .map_err(|error| site::map_site_error(&error))?;
        }
        Ok(())
    }

    async fn discard_download(&self, path: PathBuf) -> Result<(), ResolveError> {
        self.direct_ingestor
            .discard(&path)
            .await
            .map_err(|_| ResolveError::Failed("could not remove rejected media".into()))
    }
}

fn ingestor(
    tools: &ToolConfig,
    store: DownloadStore,
    maximum_bytes: u64,
    runner: RealProcessRunner,
) -> Result<Ingestor, ResolveError> {
    let fetcher = MediaFetcher::new(
        TokioDnsResolver,
        ReqwestTransport,
        store,
        FetchLimits {
            max_download_bytes: maximum_bytes,
            ..FetchLimits::default()
        },
    )
    .map_err(|_| ResolveError::Failed("invalid media download limits".into()))?;
    let probe = Ffprobe::new(
        &tools.ffprobe_path,
        runner,
        OutputLimits {
            timeout: Duration::from_secs(30),
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        },
    );
    Ok(MediaIngestor::new(fetcher, probe))
}

#[async_trait]
impl MediaResolver for ProductionMediaResolver {
    fn queue_capacity(&self) -> usize {
        self.queue_capacity
    }

    fn maximum_playlist_items(&self) -> usize {
        self.maximum_playlist_items
    }

    async fn resolve_url(
        &self,
        _guild_id: GuildId,
        requester: UserId,
        raw_url: &str,
        maximum_items: usize,
    ) -> Result<ResolvedMediaBatch, ResolveError> {
        if raw_url.len() > 4_096 || raw_url.chars().any(char::is_control) {
            return Err(ResolveError::UnsupportedUrl);
        }
        let parsed = Url::parse(raw_url).map_err(|_| ResolveError::UnsupportedUrl)?;
        if is_catalog_host(&parsed) {
            let reference = parse_catalog_url(&parsed).map_err(catalog::map_catalog_error)?;
            return self
                .resolve_catalog(reference, requester, maximum_items)
                .await;
        }
        if SiteProvider::classify(raw_url)
            .map_err(|error| site::map_site_error(&error))?
            .is_some()
        {
            return self.resolve_site(raw_url, requester, maximum_items).await;
        }
        if parsed.scheme() != "https" {
            return Err(ResolveError::UnsupportedUrl);
        }
        let track = self
            .ingest(
                &self.direct_ingestor,
                MediaRequest::DirectUrl {
                    url: raw_url.to_owned(),
                },
                requester,
                None,
                self.maximum_direct_bytes,
            )
            .await?;
        Ok(ResolvedMediaBatch::single(track))
    }

    async fn resolve_attachment(
        &self,
        _guild_id: GuildId,
        requester: UserId,
        attachment: AttachmentSource,
    ) -> Result<ResolvedMediaBatch, ResolveError> {
        let parsed =
            Url::parse(&attachment.url).map_err(|_| ResolveError::UnsupportedAttachment)?;
        if parsed.scheme() != "https" {
            return Err(ResolveError::UnsupportedAttachment);
        }
        let title = attachment.filename.clone();
        let track = self
            .ingest(
                &self.direct_ingestor,
                MediaRequest::DiscordAttachment(DiscordAttachment {
                    url: attachment.url,
                    filename: attachment.filename,
                    content_type: attachment.content_type,
                    declared_size_bytes: Some(attachment.size_bytes),
                }),
                requester,
                Some(&title),
                self.maximum_direct_bytes,
            )
            .await?;
        Ok(ResolvedMediaBatch::single(track))
    }

    async fn discard_uncommitted(&self, batch: ResolvedMediaBatch) -> Result<(), ResolveError> {
        self.discard_tracks(batch.tracks).await
    }
}

fn is_catalog_host(url: &Url) -> bool {
    matches!(
        url.host_str().map(|host| host.trim_end_matches('.')),
        Some("open.spotify.com" | "music.apple.com")
    )
}

#[cfg(test)]
mod tests {
    use super::is_catalog_host;
    use url::Url;

    #[test]
    fn catalog_hosts_and_trailing_dot_forms_never_fall_through_to_direct_fetch() {
        for value in [
            "https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC",
            "https://open.spotify.com./track/4uLU6hMCjMI75M1A2tKUQC",
            "https://music.apple.com/jp/song/example/123",
            "https://music.apple.com./jp/song/example/123",
        ] {
            assert!(is_catalog_host(&Url::parse(value).expect("URL")));
        }
        assert!(!is_catalog_host(
            &Url::parse("https://open.spotify.com.evil.test/track/example").expect("URL")
        ));
    }
}
