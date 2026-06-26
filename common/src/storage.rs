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
        self.file_ops
            .file_info(path)
            .await
            .map(FenrisMetadata::from)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FenrisError;
    use tempfile::TempDir;

    fn storage(temp_dir: &TempDir) -> TokioFsStorage {
        TokioFsStorage::new(temp_dir.path().to_path_buf())
    }

    #[tokio::test]
    async fn put_and_get_object_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let storage = storage(&temp_dir);

        storage
            .put_object(Path::new("data.txt"), b"hello")
            .await
            .unwrap();
        let data = storage.get_object(Path::new("data.txt")).await.unwrap();

        assert_eq!(data, b"hello");
    }

    #[tokio::test]
    async fn put_object_overwrites_existing_object() {
        let temp_dir = TempDir::new().unwrap();
        let storage = storage(&temp_dir);

        storage
            .put_object(Path::new("data.txt"), b"first")
            .await
            .unwrap();
        storage
            .put_object(Path::new("data.txt"), b"second")
            .await
            .unwrap();

        let data = storage.get_object(Path::new("data.txt")).await.unwrap();
        assert_eq!(data, b"second");
    }

    #[tokio::test]
    async fn append_object_extends_existing_object() {
        let temp_dir = TempDir::new().unwrap();
        let storage = storage(&temp_dir);

        storage
            .put_object(Path::new("log.txt"), b"first")
            .await
            .unwrap();
        storage
            .append_object(Path::new("log.txt"), b" second")
            .await
            .unwrap();

        let data = storage.get_object(Path::new("log.txt")).await.unwrap();
        assert_eq!(data, b"first second");
    }

    #[tokio::test]
    async fn append_object_creates_missing_object_when_parent_exists() {
        let temp_dir = TempDir::new().unwrap();
        let storage = storage(&temp_dir);

        storage.create_namespace(Path::new("logs")).await.unwrap();
        storage
            .append_object(Path::new("logs/today.txt"), b"entry")
            .await
            .unwrap();

        let data = storage
            .get_object(Path::new("logs/today.txt"))
            .await
            .unwrap();
        assert_eq!(data, b"entry");
    }

    #[tokio::test]
    async fn delete_object_removes_object() {
        let temp_dir = TempDir::new().unwrap();
        let storage = storage(&temp_dir);

        storage
            .put_object(Path::new("data.txt"), b"hello")
            .await
            .unwrap();
        assert!(storage.exists(Path::new("data.txt")).await);

        storage.delete_object(Path::new("data.txt")).await.unwrap();

        assert!(!storage.exists(Path::new("data.txt")).await);
    }

    #[tokio::test]
    async fn metadata_reports_object_and_namespace_shape() {
        let temp_dir = TempDir::new().unwrap();
        let storage = storage(&temp_dir);

        storage
            .put_object(Path::new("data.txt"), b"hello")
            .await
            .unwrap();
        storage.create_namespace(Path::new("docs")).await.unwrap();

        let object = storage.metadata(Path::new("data.txt")).await.unwrap();
        assert_eq!(object.name, "data.txt");
        assert_eq!(object.size, 5);
        assert!(!object.is_namespace);

        let namespace = storage.metadata(Path::new("docs")).await.unwrap();
        assert_eq!(namespace.name, "docs");
        assert!(namespace.is_namespace);
    }

    #[tokio::test]
    async fn namespace_create_list_and_delete() {
        let temp_dir = TempDir::new().unwrap();
        let storage = storage(&temp_dir);

        storage.create_namespace(Path::new("docs")).await.unwrap();
        storage
            .put_object(Path::new("docs/a.txt"), b"a")
            .await
            .unwrap();
        storage
            .create_namespace(Path::new("docs/nested"))
            .await
            .unwrap();

        let entries = storage.list_namespace(Path::new("docs")).await.unwrap();
        let names: Vec<String> = entries.into_iter().map(|entry| entry.name).collect();
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"nested".to_string()));

        storage
            .delete_namespace(Path::new("docs/nested"))
            .await
            .unwrap();
        assert!(!storage.exists(Path::new("docs/nested")).await);
    }

    #[tokio::test]
    async fn existence_and_kind_checks_reflect_storage_state() {
        let temp_dir = TempDir::new().unwrap();
        let storage = storage(&temp_dir);

        storage
            .put_object(Path::new("data.txt"), b"hello")
            .await
            .unwrap();
        storage.create_namespace(Path::new("docs")).await.unwrap();

        assert!(storage.exists(Path::new("data.txt")).await);
        assert!(storage.is_object(Path::new("data.txt")).await);
        assert!(!storage.is_namespace(Path::new("data.txt")).await);

        assert!(storage.exists(Path::new("docs")).await);
        assert!(storage.is_namespace(Path::new("docs")).await);
        assert!(!storage.is_object(Path::new("docs")).await);
    }

    #[tokio::test]
    async fn path_traversal_is_rejected() {
        let temp_dir = TempDir::new().unwrap();
        let storage = storage(&temp_dir);

        let result = storage.get_object(Path::new("../../../etc/passwd")).await;

        assert!(matches!(result, Err(FenrisError::FileOperationError(_))));
    }
}
