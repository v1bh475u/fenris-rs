use crate::{FenrisMetadata, Result};
use std::path::Path;

#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync + 'static {
    async fn put_object(&self, path: &Path, data: &[u8]) -> Result<()>;

    async fn get_object(&self, path: &Path) -> Result<Vec<u8>>;

    async fn append_object(&self, path: &Path, data: &[u8]) -> Result<()>;

    async fn delete_object(&self, path: &Path) -> Result<()>;

    async fn metadata(&self, path: &Path) -> Result<FenrisMetadata>;

    async fn create_namespace(&self, path: &Path) -> Result<()>;

    async fn list_namespace(&self, path: &Path) -> Result<Vec<FenrisMetadata>>;

    async fn delete_namespace(&self, path: &Path) -> Result<()>;

    async fn exists(&self, path: &Path) -> bool;

    async fn is_namespace(&self, path: &Path) -> bool;

    async fn is_object(&self, path: &Path) -> bool;
}
