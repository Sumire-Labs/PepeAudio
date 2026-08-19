use crate::CatalogProvider;

pub type CatalogResult<T> = Result<T, CatalogError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CatalogError {
    #[error("the URL is not a supported official catalog URL")]
    UnsupportedUrl,
    #[error("{0} catalog access is not configured")]
    ProviderNotConfigured(CatalogProvider),
    #[error("{0} credentials are missing or invalid")]
    InvalidCredentials(CatalogProvider),
    #[error("the requested {0} catalog item was not found")]
    NotFound(CatalogProvider),
    #[error(
        "Spotify playlist metadata requires an authorized owner or collaborator user and is unavailable to the app-only client"
    )]
    SpotifyPlaylistAccessDenied,
    #[error("{0} denied access to the requested catalog item")]
    AccessDenied(CatalogProvider),
    #[error("{provider} rate limited the catalog request")]
    RateLimited {
        provider: CatalogProvider,
        retry_after_seconds: Option<u64>,
    },
    #[error("{0} returned an unsupported or malformed response")]
    InvalidResponse(CatalogProvider),
    #[error("{0} catalog request failed")]
    Transport(CatalogProvider),
    #[error("{0} catalog response exceeded the allowed size")]
    ResponseTooLarge(CatalogProvider),
    #[error("the collection limit must be between 1 and 100")]
    InvalidCollectionLimit,
}
