use url::Url;

use crate::{
    AppleMusicCatalog, CatalogCollection, CatalogError, CatalogProvider, CatalogReference,
    CatalogResult, SpotifyCatalog, parse_catalog_url, provider::ProviderCatalog,
};

const DEFAULT_COLLECTION_LIMIT: usize = 25;
const HARD_COLLECTION_LIMIT: usize = 100;

pub struct CatalogResolverBuilder {
    collection_limit: usize,
    spotify: Option<SpotifyCatalog>,
    apple_music: Option<AppleMusicCatalog>,
}

impl Default for CatalogResolverBuilder {
    fn default() -> Self {
        Self {
            collection_limit: DEFAULT_COLLECTION_LIMIT,
            spotify: None,
            apple_music: None,
        }
    }
}

impl CatalogResolverBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the number of source items processed from an album or playlist.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::InvalidCollectionLimit`] unless `limit` is in
    /// the inclusive range `1..=100`.
    pub fn collection_limit(mut self, limit: usize) -> CatalogResult<Self> {
        if !(1..=HARD_COLLECTION_LIMIT).contains(&limit) {
            return Err(CatalogError::InvalidCollectionLimit);
        }
        self.collection_limit = limit;
        Ok(self)
    }

    #[must_use]
    pub fn spotify(mut self, client: SpotifyCatalog) -> Self {
        self.spotify = Some(client);
        self
    }

    #[must_use]
    pub fn apple_music(mut self, client: AppleMusicCatalog) -> Self {
        self.apple_music = Some(client);
        self
    }

    #[must_use]
    pub fn build(self) -> CatalogResolver {
        CatalogResolver {
            collection_limit: self.collection_limit,
            spotify: self.spotify,
            apple_music: self.apple_music,
        }
    }
}

pub struct CatalogResolver {
    collection_limit: usize,
    spotify: Option<SpotifyCatalog>,
    apple_music: Option<AppleMusicCatalog>,
}

impl CatalogResolver {
    /// Resolves one official catalog link into metadata. This method does not
    /// resolve, download, or return playable audio.
    ///
    /// # Errors
    ///
    /// Returns a catalog parsing, credential, transport, or provider error.
    pub async fn resolve_url(&self, url: &Url) -> CatalogResult<CatalogCollection> {
        let reference = parse_catalog_url(url)?;
        self.resolve_reference(&reference).await
    }

    /// Resolves one catalog link without fetching more source items than the
    /// caller's current queue headroom.
    ///
    /// # Errors
    ///
    /// Returns a catalog parsing, limit, credential, transport, or provider
    /// error.
    pub async fn resolve_url_with_limit(
        &self,
        url: &Url,
        maximum_items: usize,
    ) -> CatalogResult<CatalogCollection> {
        let reference = parse_catalog_url(url)?;
        self.resolve_reference_with_limit(&reference, maximum_items)
            .await
    }

    /// Resolves an already validated reference.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::ProviderNotConfigured`] when the corresponding
    /// provider was not added to the builder.
    pub async fn resolve_reference(
        &self,
        reference: &CatalogReference,
    ) -> CatalogResult<CatalogCollection> {
        self.resolve_reference_with_limit(reference, self.collection_limit)
            .await
    }

    /// Resolves a validated reference with a per-request bound no greater than
    /// the operator-configured collection limit.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::InvalidCollectionLimit`] when `maximum_items`
    /// is zero or exceeds the configured limit. Provider configuration,
    /// transport, and response failures are propagated unchanged.
    pub async fn resolve_reference_with_limit(
        &self,
        reference: &CatalogReference,
        maximum_items: usize,
    ) -> CatalogResult<CatalogCollection> {
        if maximum_items == 0 || maximum_items > self.collection_limit {
            return Err(CatalogError::InvalidCollectionLimit);
        }
        match reference.provider() {
            CatalogProvider::Spotify => {
                self.spotify
                    .as_ref()
                    .ok_or(CatalogError::ProviderNotConfigured(
                        CatalogProvider::Spotify,
                    ))?
                    .resolve(reference, maximum_items)
                    .await
            }
            CatalogProvider::AppleMusic => {
                self.apple_music
                    .as_ref()
                    .ok_or(CatalogError::ProviderNotConfigured(
                        CatalogProvider::AppleMusic,
                    ))?
                    .resolve(reference, maximum_items)
                    .await
            }
        }
    }

    #[must_use]
    pub const fn collection_limit(&self) -> usize {
        self.collection_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_hard_collection_limits_are_enforced() {
        assert_eq!(
            CatalogResolverBuilder::new().build().collection_limit(),
            DEFAULT_COLLECTION_LIMIT
        );
        assert_eq!(
            CatalogResolverBuilder::new().collection_limit(0).err(),
            Some(CatalogError::InvalidCollectionLimit)
        );
        assert_eq!(
            CatalogResolverBuilder::new()
                .collection_limit(HARD_COLLECTION_LIMIT + 1)
                .err(),
            Some(CatalogError::InvalidCollectionLimit)
        );
    }

    #[tokio::test]
    async fn reports_unconfigured_provider_without_network_access() {
        let resolver = CatalogResolverBuilder::new().build();
        let url = Url::parse("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC").expect("URL");

        assert_eq!(
            resolver.resolve_url(&url).await,
            Err(CatalogError::ProviderNotConfigured(
                CatalogProvider::Spotify
            ))
        );
    }

    #[tokio::test]
    async fn per_request_limit_cannot_exceed_the_configured_boundary() {
        let resolver = CatalogResolverBuilder::new()
            .collection_limit(10)
            .expect("configured limit")
            .build();
        let url = Url::parse("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC").expect("URL");

        assert_eq!(
            resolver.resolve_url_with_limit(&url, 11).await,
            Err(CatalogError::InvalidCollectionLimit)
        );
        assert_eq!(
            resolver.resolve_url_with_limit(&url, 0).await,
            Err(CatalogError::InvalidCollectionLimit)
        );
    }
}
