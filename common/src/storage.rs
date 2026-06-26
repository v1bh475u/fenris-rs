use crate::{DefaultFileOperations, FenrisMetadata, FileOperations, Result};
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone)]
pub struct TokioFsStorage {
    file_ops: DefaultFileOperations,
}

impl TokioFsStorage {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            file_ops: DefaultFileOperations::new(base_dir),
        }
    }

    pub fn with_file_ops(file_ops: DefaultFileOperations) -> Self {
        Self { file_ops }
    }

    pub fn base_dir(&self) -> &Path {
        self.file_ops.base_dir()
    }
}

#[async_trait::async_trait]
impl StorageBackend for TokioFsStorage {
    async fn put_object(&self, path: &Path, data: &[u8]) -> Result<()> {
        self.file_ops.write_file(path, data).await
    }

    async fn get_object(&self, path: &Path) -> Result<Vec<u8>> {
        self.file_ops.read_file(path).await
    }

    async fn append_object(&self, path: &Path, data: &[u8]) -> Result<()> {
        self.file_ops.append_file(path, data).await
    }

    async fn delete_object(&self, path: &Path) -> Result<()> {
        self.file_ops.delete_file(path).await
    }

    async fn metadata(&self, path: &Path) -> Result<FenrisMetadata> {
        self.file_ops.file_info(path).await.map(FenrisMetadata::from)
    }

    async fn create_namespace(&self, path: &Path) -> Result<()> {
        self.file_ops.create_dir(path).await
    }

    async fn list_namespace(&self, path: &Path) -> Result<Vec<FenrisMetadata>> {
        Ok(self
            .file_ops
            .list_dir(path)
            .await?
            .into_iter()
            .map(FenrisMetadata::from)
            .collect())
    }

    async fn delete_namespace(&self, path: &Path) -> Result<()> {
        self.file_ops.delete_dir(path).await
    }

    async fn exists(&self, path: &Path) -> bool {
        self.file_ops.exists(path).await
    }

    async fn is_namespace(&self, path: &Path) -> bool {
        self.file_ops.is_dir(path).await
    }

    async fn is_object(&self, path: &Path) -> bool {
        self.file_ops.is_file(path).await
    }
}
