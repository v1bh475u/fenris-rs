use crate::error::{FenrisError, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, warn};

#[derive(Debug, Clone, PartialEq)]
pub struct FileMetadata {
    pub name: String,
    pub size: u64,
    pub is_directory: bool,
    pub modified_time: u64,
    pub permissions: u32,
}

impl FileMetadata {
    pub async fn from_path(path: &Path) -> Result<Self> {
        let metadata = fs::metadata(path).await.map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to get metadata: {}", e))
        })?;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let modified_time = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode()
        };

        #[cfg(not(unix))]
        let permissions = if metadata.permissions().readonly() {
            0o444
        } else {
            0o644
        };

        Ok(Self {
            name,
            size: metadata.len(),
            is_directory: metadata.is_dir(),
            modified_time,
            permissions,
        })
    }
}

#[async_trait::async_trait]
pub trait FileOperations: Send + Sync {
    async fn create_file(&self, path: &Path) -> Result<()>;

    async fn read_file(&self, path: &Path) -> Result<Vec<u8>>;

    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<()>;

    async fn append_file(&self, path: &Path, data: &[u8]) -> Result<()>;

    async fn delete_file(&self, path: &Path) -> Result<()>;

    async fn file_info(&self, path: &Path) -> Result<FileMetadata>;

    async fn create_dir(&self, path: &Path) -> Result<()>;

    async fn list_dir(&self, path: &Path) -> Result<Vec<FileMetadata>>;

    async fn delete_dir(&self, path: &Path) -> Result<()>;

    async fn exists(&self, path: &Path) -> bool;

    async fn is_dir(&self, path: &Path) -> bool;

    async fn is_file(&self, path: &Path) -> bool;
}

#[derive(Debug, Clone)]
pub struct DefaultFileOperations {
    base_dir: PathBuf,
}

impl DefaultFileOperations {
    pub fn new(base_dir: PathBuf) -> Self {
        let base_dir = base_dir.canonicalize().unwrap_or(base_dir);
        Self { base_dir }
    }

    pub fn with_current_dir() -> Result<Self> {
        let base_dir = std::env::current_dir().map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to get current dir: {}", e))
        })?;
        Ok(Self { base_dir })
    }

    fn resolve_path(&self, path: &Path) -> Result<PathBuf> {
        let path = path.strip_prefix("/").unwrap_or(path);

        let full_path = self.base_dir.join(path);

        let canonical = full_path.canonicalize().or_else(|_| {
            if let Some(parent) = full_path.parent()
                && let Ok(canonical_parent) = parent.canonicalize()
                && let Some(filename) = full_path.file_name()
            {
                return Ok(canonical_parent.join(filename));
            }
            Err(FenrisError::FileOperationError("Invalid path".to_string()))
        })?;

        if !canonical.starts_with(&self.base_dir) {
            warn!("Path traversal attempt: {:?}", path);
            return Err(FenrisError::FileOperationError(
                "Path outside base directory".to_string(),
            ));
        }

        Ok(canonical)
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[async_trait::async_trait]
impl FileOperations for DefaultFileOperations {
    async fn create_file(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path)?;

        debug!("Creating file: {:?}", full_path);

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                FenrisError::FileOperationError(format!("Failed to create parent dirs: {}", e))
            })?;
        }

        fs::File::create(&full_path).await.map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to create file: {}", e))
        })?;

        debug!("File created: {:?}", full_path);

        Ok(())
    }

    async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
        let full_path = self.resolve_path(path)?;

        debug!("Reading file: {:?}", full_path);

        let mut file = fs::File::open(&full_path)
            .await
            .map_err(|e| FenrisError::FileOperationError(format!("Failed to open file: {}", e)))?;

        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .await
            .map_err(|e| FenrisError::FileOperationError(format!("Failed to read file: {}", e)))?;

        debug!("Read {} bytes from {:?}", contents.len(), full_path);

        Ok(contents)
    }

    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<()> {
        let full_path = self.resolve_path(path)?;

        debug!("Writing {} bytes to {:?}", data.len(), full_path);

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                FenrisError::FileOperationError(format!("Failed to create parent dirs: {}", e))
            })?;
        }

        let mut file = fs::File::create(&full_path).await.map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to create file: {}", e))
        })?;

        file.write_all(data)
            .await
            .map_err(|e| FenrisError::FileOperationError(format!("Failed to write file: {}", e)))?;

        debug!("Wrote {} bytes to {:?}", data.len(), full_path);

        Ok(())
    }

    async fn append_file(&self, path: &Path, data: &[u8]) -> Result<()> {
        let full_path = self.resolve_path(path)?;

        debug!("Appending {} bytes to {:? }", data.len(), full_path);

        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&full_path)
            .await
            .map_err(|e| {
                FenrisError::FileOperationError(format!("Failed to open file for append: {}", e))
            })?;

        file.write_all(data).await.map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to append to file: {}", e))
        })?;

        debug!("Appended {} bytes to {:?}", data.len(), full_path);

        Ok(())
    }

    async fn delete_file(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path)?;

        debug!("Deleting file: {:?}", full_path);

        fs::remove_file(&full_path).await.map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to delete file: {}", e))
        })?;

        debug!("File deleted: {:?}", full_path);

        Ok(())
    }

    async fn file_info(&self, path: &Path) -> Result<FileMetadata> {
        let full_path = self.resolve_path(path)?;

        debug!("Getting file info:  {:?}", full_path);

        FileMetadata::from_path(&full_path).await
    }

    async fn create_dir(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path)?;

        debug!("Creating directory: {:?}", full_path);

        fs::create_dir_all(&full_path).await.map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to create directory: {}", e))
        })?;

        debug!("Directory created: {:?}", full_path);

        Ok(())
    }

    async fn list_dir(&self, path: &Path) -> Result<Vec<FileMetadata>> {
        let full_path = self.resolve_path(path)?;

        debug!("Listing directory:  {:?}", full_path);

        let mut entries = Vec::new();
        let mut dir = fs::read_dir(&full_path).await.map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to read directory: {}", e))
        })?;

        while let Some(entry) = dir
            .next_entry()
            .await
            .map_err(|e| FenrisError::FileOperationError(format!("Failed to read entry: {}", e)))?
        {
            let entry_path = entry.path();
            match FileMetadata::from_path(&entry_path).await {
                Ok(metadata) => entries.push(metadata),
                Err(e) => {
                    warn!("Failed to get metadata for {:?}: {}", entry_path, e);
                }
            }
        }

        debug!("Listed {} entries in {:?}", entries.len(), full_path);

        Ok(entries)
    }

    async fn delete_dir(&self, path: &Path) -> Result<()> {
        let full_path = self.resolve_path(path)?;

        debug!("Deleting directory: {:?}", full_path);

        fs::remove_dir(&full_path).await.map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to delete directory: {}", e))
        })?;

        debug!("Directory deleted: {:?}", full_path);

        Ok(())
    }

    async fn exists(&self, path: &Path) -> bool {
        if let Ok(full_path) = self.resolve_path(path) {
            fs::metadata(&full_path).await.is_ok()
        } else {
            false
        }
    }

    async fn is_dir(&self, path: &Path) -> bool {
        if let Ok(full_path) = self.resolve_path(path)
            && let Ok(metadata) = fs::metadata(&full_path).await
        {
            return metadata.is_dir();
        }
        false
    }

    async fn is_file(&self, path: &Path) -> bool {
        if let Ok(full_path) = self.resolve_path(path)
            && let Ok(metadata) = fs::metadata(&full_path).await
        {
            return metadata.is_file();
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_and_read_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_ops = DefaultFileOperations::new(temp_dir.path().to_path_buf());

        let path = Path::new("test.txt");
        file_ops.create_file(path).await.unwrap();

        assert!(file_ops.exists(path).await);
        assert!(file_ops.is_file(path).await);
    }

    #[tokio::test]
    async fn test_write_and_read_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_ops = DefaultFileOperations::new(temp_dir.path().to_path_buf());

        let path = Path::new("test.txt");
        let data = b"Hello, World!";

        file_ops.write_file(path, data).await.unwrap();
        let read_data = file_ops.read_file(path).await.unwrap();

        assert_eq!(read_data, data);
    }

    #[tokio::test]
    async fn test_append_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_ops = DefaultFileOperations::new(temp_dir.path().to_path_buf());

        let path = Path::new("test.txt");

        file_ops.write_file(path, b"Hello").await.unwrap();
        file_ops.append_file(path, b", World!").await.unwrap();

        let data = file_ops.read_file(path).await.unwrap();
        assert_eq!(data, b"Hello, World!");
    }

    #[tokio::test]
    async fn test_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_ops = DefaultFileOperations::new(temp_dir.path().to_path_buf());

        let path = Path::new("test.txt");
        file_ops.create_file(path).await.unwrap();
        assert!(file_ops.exists(path).await);

        file_ops.delete_file(path).await.unwrap();
        assert!(!file_ops.exists(path).await);
    }

    #[tokio::test]
    async fn test_create_and_list_dir() {
        let temp_dir = TempDir::new().unwrap();
        let file_ops = DefaultFileOperations::new(temp_dir.path().to_path_buf());

        let dir_path = Path::new("testdir");
        file_ops.create_dir(dir_path).await.unwrap();

        assert!(file_ops.exists(dir_path).await);
        assert!(file_ops.is_dir(dir_path).await);

        file_ops
            .write_file(&dir_path.join("file1.txt"), b"test1")
            .await
            .unwrap();
        file_ops
            .write_file(&dir_path.join("file2.txt"), b"test2")
            .await
            .unwrap();

        let entries = file_ops.list_dir(dir_path).await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_path_traversal_prevention() {
        let temp_dir = TempDir::new().unwrap();
        let file_ops = DefaultFileOperations::new(temp_dir.path().to_path_buf());

        let result = file_ops.read_file(Path::new("../../../etc/passwd")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let file_ops = DefaultFileOperations::new(temp_dir.path().to_path_buf());

        let path = Path::new("test.txt");
        let data = b"Hello, World! ";
        file_ops.write_file(path, data).await.unwrap();

        let metadata = file_ops.file_info(path).await.unwrap();
        assert_eq!(metadata.name, "test.txt");
        assert_eq!(metadata.size, data.len() as u64);
        assert!(!metadata.is_directory);
    }
}
