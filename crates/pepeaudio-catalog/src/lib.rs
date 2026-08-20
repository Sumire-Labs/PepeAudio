//! Metadata-only catalog adapters for supported music services.
//!
//! This crate never obtains or exposes playable media URLs. It converts an
//! official catalog link into provider-neutral metadata that a separate media
//! resolver may use as a search request.

mod error;
mod http;
mod model;
mod provider;
mod secret;
mod service;
mod url;

pub use error::{CatalogError, CatalogResult};
pub use model::{
    CatalogCollection, CatalogItemKind, CatalogProvider, CatalogReference, CatalogSearchRequest,
    CatalogTrackMetadata,
};
pub use provider::{
    AppleMusicCatalog, AppleMusicPublicCatalog, SpotifyCatalog, SpotifyPublicCatalog,
};
pub use service::{CatalogResolver, CatalogResolverBuilder};
pub use url::parse_catalog_url;
