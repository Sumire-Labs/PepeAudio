use async_trait::async_trait;

use crate::{CatalogCollection, CatalogReference, CatalogResult};

#[async_trait]
pub(crate) trait ProviderCatalog: Send + Sync {
    async fn resolve(
        &self,
        reference: &CatalogReference,
        collection_limit: usize,
    ) -> CatalogResult<CatalogCollection>;
}
