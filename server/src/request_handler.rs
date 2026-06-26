use common::{
    DEFAULT_TRANSFER_CHUNK_SIZE, FenrisCommand, FenrisError, FenrisOutput, ObjectWriteMode, Result,
    StorageBackend, TransferChunk,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error};

pub struct RequestHandler<B: StorageBackend> {
    storage: Arc<B>,
}

#[derive(Debug, Clone)]
pub struct ActiveWriteTransfer {
    path: PathBuf,
    expected_offset: u64,
    total_size: u64,
}

impl<B: StorageBackend> RequestHandler<B> {
    pub fn new(storage: Arc<B>) -> Self {
        Self { storage }
    }

    fn resolve_path(&self, path: &Path, current_dir: &Path) -> PathBuf {
        if path.as_os_str().is_empty() || path == Path::new(".") {
            current_dir.to_path_buf()
        } else if path.is_absolute() {
            path.to_path_buf()
        } else {
            current_dir.join(path)
        }
    }

    pub async fn process_command(
        &self,
        client_id: u64,
        command: &FenrisCommand,
        current_dir: &mut PathBuf,
    ) -> FenrisOutput {
        debug!(
            "Processing command from client {} in dir {:?}: {:?}",
            client_id, current_dir, command
        );

        match self.handle_command(command, current_dir).await {
            Ok(output) => output,
            Err(e) => {
                error!("Command failed: {}", e);
                FenrisOutput::Error {
                    message: e.to_string(),
                }
            }
        }
    }

    async fn handle_command(
        &self,
        command: &FenrisCommand,
        current_dir: &mut PathBuf,
    ) -> Result<FenrisOutput> {
        match command {
            FenrisCommand::Ping => Ok(FenrisOutput::Pong),
            FenrisCommand::CreateObject { path } => {
                self.handle_create_object(path, current_dir).await
            }
            FenrisCommand::ReadObject { path } => self.handle_read_object(path, current_dir).await,
            FenrisCommand::WriteObject { path, data } => {
                self.handle_write_object(path, data, current_dir).await
            }
            FenrisCommand::AppendObject { path, data } => {
                self.handle_append_object(path, data, current_dir).await
            }
            FenrisCommand::DeleteObject { path } => {
                self.handle_delete_object(path, current_dir).await
            }
            FenrisCommand::UploadObject { path, data } => {
                self.handle_upload_object(path, data, current_dir).await
            }
            FenrisCommand::BeginObjectWrite { .. } => Err(FenrisError::InvalidRequest(
                "chunked write must be handled by a connection".to_string(),
            )),
            FenrisCommand::WriteObjectChunk(_) => Err(FenrisError::InvalidRequest(
                "chunked write must be handled by a connection".to_string(),
            )),
            FenrisCommand::ObjectInfo { path } => self.handle_object_info(path, current_dir).await,
            FenrisCommand::CreateNamespace { path } => {
                self.handle_create_namespace(path, current_dir).await
            }
            FenrisCommand::ListNamespace { path } => {
                self.handle_list_namespace(path, current_dir).await
            }
            FenrisCommand::ChangeNamespace { path } => {
                self.handle_change_namespace(path, current_dir).await
            }
            FenrisCommand::DeleteNamespace { path } => {
                self.handle_delete_namespace(path, current_dir).await
            }
            FenrisCommand::Terminate => Ok(FenrisOutput::Terminated),
        }
    }

    async fn handle_create_object(&self, path: &Path, current_dir: &Path) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.storage.put_object(&path, b"").await?;

        Ok(FenrisOutput::Success {
            message: format!("File created: {}", path.to_string_lossy()),
        })
    }

    async fn handle_read_object(&self, path: &Path, current_dir: &Path) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        let data = self.storage.get_object(&path).await?;
        let total_size = data.len() as u64;

        Ok(FenrisOutput::ObjectContent {
            data,
            total_size,
            truncated: false,
        })
    }

    async fn handle_write_object(
        &self,
        path: &Path,
        data: &[u8],
        current_dir: &Path,
    ) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.storage.put_object(&path, data).await?;

        Ok(FenrisOutput::Success {
            message: format!("File written: {} bytes", data.len()),
        })
    }

    async fn handle_append_object(
        &self,
        path: &Path,
        data: &[u8],
        current_dir: &Path,
    ) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.storage.append_object(&path, data).await?;

        Ok(FenrisOutput::Success {
            message: format!(
                "Appended {} bytes to {}",
                data.len(),
                path.to_string_lossy()
            ),
        })
    }

    async fn handle_delete_object(&self, path: &Path, current_dir: &Path) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.storage.delete_object(&path).await?;

        Ok(FenrisOutput::Success {
            message: format!("File deleted: {}", path.to_string_lossy()),
        })
    }

    async fn handle_upload_object(
        &self,
        path: &Path,
        data: &[u8],
        current_dir: &Path,
    ) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.storage.put_object(&path, data).await?;

        Ok(FenrisOutput::Success {
            message: format!(
                "Uploaded {} bytes to {}",
                data.len(),
                path.to_string_lossy()
            ),
        })
    }

    async fn handle_object_info(&self, path: &Path, current_dir: &Path) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        let metadata = self.storage.metadata(&path).await?;

        Ok(FenrisOutput::ObjectInfo { metadata })
    }

    async fn handle_create_namespace(
        &self,
        path: &Path,
        current_dir: &Path,
    ) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.storage.create_namespace(&path).await?;

        Ok(FenrisOutput::Success {
            message: format!("Directory created: {}", path.to_string_lossy()),
        })
    }

    async fn handle_list_namespace(&self, path: &Path, current_dir: &Path) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        let entries = self.storage.list_namespace(&path).await?;

        Ok(FenrisOutput::NamespaceListing { entries })
    }

    async fn handle_delete_namespace(
        &self,
        path: &Path,
        current_dir: &Path,
    ) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.storage.delete_namespace(&path).await?;

        Ok(FenrisOutput::Success {
            message: format!("Directory deleted: {}", path.to_string_lossy()),
        })
    }

    async fn handle_change_namespace(
        &self,
        path: &Path,
        current_dir: &mut PathBuf,
    ) -> Result<FenrisOutput> {
        let target_path = if path.as_os_str().is_empty() || path == Path::new("~") {
            PathBuf::from("/")
        } else if path == Path::new(".") {
            current_dir.clone()
        } else if path == Path::new("..") {
            current_dir
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/"))
        } else if path.is_absolute() {
            path.to_path_buf()
        } else {
            current_dir.join(path)
        };

        if !self.storage.is_namespace(&target_path).await {
            return Err(FenrisError::FileOperationError(
                "Not a directory".to_string(),
            ));
        }

        *current_dir = target_path.clone();

        Ok(FenrisOutput::NamespaceChanged { path: target_path })
    }

    pub async fn begin_object_write(
        &self,
        path: &Path,
        mode: ObjectWriteMode,
        total_size: u64,
        current_dir: &Path,
    ) -> Result<ActiveWriteTransfer> {
        let path = self.resolve_path(path, current_dir);

        match mode {
            ObjectWriteMode::Write | ObjectWriteMode::Upload => {
                self.storage.put_object(&path, b"").await?;
            }
            ObjectWriteMode::Append => {
                self.storage.append_object(&path, b"").await?;
            }
        }

        Ok(ActiveWriteTransfer {
            path,
            expected_offset: 0,
            total_size,
        })
    }

    pub async fn write_object_chunk(
        &self,
        transfer: &mut ActiveWriteTransfer,
        chunk: &TransferChunk,
        max_chunk_size: usize,
    ) -> Result<FenrisOutput> {
        if chunk.data.len() > max_chunk_size {
            return Err(FenrisError::InvalidRequest(format!(
                "chunk too large: max {} bytes, got {} bytes",
                max_chunk_size,
                chunk.data.len()
            )));
        }

        if chunk.offset != transfer.expected_offset {
            return Err(FenrisError::InvalidRequest(format!(
                "unexpected chunk offset: expected {}, got {}",
                transfer.expected_offset, chunk.offset
            )));
        }

        if chunk.total_size != transfer.total_size {
            return Err(FenrisError::InvalidRequest(format!(
                "transfer size changed: expected {}, got {}",
                transfer.total_size, chunk.total_size
            )));
        }

        self.storage
            .append_object(&transfer.path, &chunk.data)
            .await?;
        transfer.expected_offset += chunk.data.len() as u64;

        if chunk.is_last {
            if transfer.expected_offset != transfer.total_size {
                return Err(FenrisError::InvalidRequest(format!(
                    "incomplete transfer: expected {} bytes, got {} bytes",
                    transfer.total_size, transfer.expected_offset
                )));
            }

            return Ok(FenrisOutput::Success {
                message: format!("Transfer complete: {} bytes", transfer.total_size),
            });
        }

        Ok(FenrisOutput::TransferProgress {
            offset: transfer.expected_offset,
        })
    }

    pub async fn read_object_chunk(
        &self,
        path: &Path,
        current_dir: &Path,
        offset: u64,
    ) -> Result<TransferChunk> {
        let path = self.resolve_path(path, current_dir);
        let chunk = self
            .storage
            .get_object_chunk(&path, offset, DEFAULT_TRANSFER_CHUNK_SIZE)
            .await?;

        Ok(TransferChunk {
            offset: chunk.offset,
            data: chunk.data,
            is_last: chunk.is_last,
            total_size: chunk.total_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::MemoryStorage;

    fn create_handler() -> (RequestHandler<MemoryStorage>, Arc<MemoryStorage>) {
        let storage = Arc::new(MemoryStorage::new());
        let handler = RequestHandler::new(storage.clone());
        (handler, storage)
    }

    #[tokio::test]
    async fn test_ping() {
        let (handler, _) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let output = handler
            .process_command(1, &FenrisCommand::Ping, &mut current_dir)
            .await;

        assert_eq!(output, FenrisOutput::Pong);
    }

    #[tokio::test]
    async fn test_create_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/home");
        ops.create_namespace(&current_dir).await.unwrap();

        let output = handler
            .process_command(
                1,
                &FenrisCommand::CreateObject {
                    path: PathBuf::from("test.txt"),
                },
                &mut current_dir,
            )
            .await;

        assert!(matches!(output, FenrisOutput::Success { .. }));

        assert!(ops.exists(Path::new("/home/test.txt")).await);
    }

    #[tokio::test]
    async fn test_write_and_read_file() {
        let (handler, _) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let data = b"Hello, World!".to_vec();
        let write_output = handler
            .process_command(
                1,
                &FenrisCommand::WriteObject {
                    path: PathBuf::from("hello.txt"),
                    data: data.clone(),
                },
                &mut current_dir,
            )
            .await;
        assert!(matches!(write_output, FenrisOutput::Success { .. }));

        let read_output = handler
            .process_command(
                1,
                &FenrisCommand::ReadObject {
                    path: PathBuf::from("hello.txt"),
                },
                &mut current_dir,
            )
            .await;

        assert_eq!(
            read_output,
            FenrisOutput::ObjectContent {
                data,
                total_size: 13,
                truncated: false
            }
        );
    }

    #[tokio::test]
    async fn test_append_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.put_object(Path::new("/log.txt"), b"Init")
            .await
            .unwrap();

        let output = handler
            .process_command(
                1,
                &FenrisCommand::AppendObject {
                    path: PathBuf::from("log.txt"),
                    data: b" - More".to_vec(),
                },
                &mut current_dir,
            )
            .await;

        assert!(matches!(output, FenrisOutput::Success { .. }));

        let content = ops.get_object(Path::new("/log.txt")).await.unwrap();
        assert_eq!(content, b"Init - More");
    }

    #[tokio::test]
    async fn test_append_file_uses_storage_create_semantics() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let output = handler
            .process_command(
                1,
                &FenrisCommand::AppendObject {
                    path: PathBuf::from("created.log"),
                    data: b"Created by append".to_vec(),
                },
                &mut current_dir,
            )
            .await;

        assert!(matches!(output, FenrisOutput::Success { .. }));

        let content = ops.get_object(Path::new("/created.log")).await.unwrap();
        assert_eq!(content, b"Created by append");
    }

    #[tokio::test]
    async fn test_delete_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.put_object(Path::new("/temp.txt"), b"").await.unwrap();

        let output = handler
            .process_command(
                1,
                &FenrisCommand::DeleteObject {
                    path: PathBuf::from("temp.txt"),
                },
                &mut current_dir,
            )
            .await;

        assert!(matches!(output, FenrisOutput::Success { .. }));
        assert!(!ops.exists(Path::new("/temp.txt")).await);
    }

    #[tokio::test]
    async fn test_change_dir() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.create_namespace(Path::new("/data")).await.unwrap();

        let output = handler
            .process_command(
                1,
                &FenrisCommand::ChangeNamespace {
                    path: PathBuf::from("data"),
                },
                &mut current_dir,
            )
            .await;
        assert_eq!(
            output,
            FenrisOutput::NamespaceChanged {
                path: PathBuf::from("/data")
            }
        );
        assert_eq!(current_dir, PathBuf::from("/data"));

        let output = handler
            .process_command(
                1,
                &FenrisCommand::ChangeNamespace {
                    path: PathBuf::from(".."),
                },
                &mut current_dir,
            )
            .await;
        assert_eq!(
            output,
            FenrisOutput::NamespaceChanged {
                path: PathBuf::from("/")
            }
        );
        assert_eq!(current_dir, PathBuf::from("/"));

        let output = handler
            .process_command(
                1,
                &FenrisCommand::ChangeNamespace {
                    path: PathBuf::from("missing"),
                },
                &mut current_dir,
            )
            .await;
        assert!(matches!(output, FenrisOutput::Error { .. }));
        assert_eq!(current_dir, PathBuf::from("/"));
    }

    #[tokio::test]
    async fn test_list_dir() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.create_namespace(Path::new("/data")).await.unwrap();
        ops.put_object(Path::new("/data/f1.txt"), b"")
            .await
            .unwrap();
        ops.create_namespace(Path::new("/data/sub")).await.unwrap();

        let output = handler
            .process_command(
                1,
                &FenrisCommand::ListNamespace {
                    path: PathBuf::from("data"),
                },
                &mut current_dir,
            )
            .await;

        let FenrisOutput::NamespaceListing { entries } = output else {
            panic!("Expected namespace listing");
        };

        assert_eq!(entries.len(), 2);
        let names: Vec<String> = entries.iter().map(|entry| entry.name.clone()).collect();
        assert!(names.contains(&"f1.txt".to_string()));
        assert!(names.contains(&"sub".to_string()));
    }

    #[tokio::test]
    async fn test_create_and_delete_dir() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let output = handler
            .process_command(
                1,
                &FenrisCommand::CreateNamespace {
                    path: PathBuf::from("newdir"),
                },
                &mut current_dir,
            )
            .await;
        assert!(matches!(output, FenrisOutput::Success { .. }));
        assert!(ops.is_namespace(Path::new("/newdir")).await);

        let output = handler
            .process_command(
                1,
                &FenrisCommand::DeleteNamespace {
                    path: PathBuf::from("newdir"),
                },
                &mut current_dir,
            )
            .await;
        assert!(matches!(output, FenrisOutput::Success { .. }));
        assert!(!ops.is_namespace(Path::new("/newdir")).await);
    }

    #[tokio::test]
    async fn test_file_info() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");
        ops.put_object(Path::new("/info.txt"), b"").await.unwrap();

        let output = handler
            .process_command(
                1,
                &FenrisCommand::ObjectInfo {
                    path: PathBuf::from("info.txt"),
                },
                &mut current_dir,
            )
            .await;

        let FenrisOutput::ObjectInfo { metadata } = output else {
            panic!("Expected object info");
        };

        assert_eq!(metadata.name, "info.txt");
        assert!(!metadata.is_namespace);
    }

    #[tokio::test]
    async fn test_upload_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let data = b"Upload Data".to_vec();
        let output = handler
            .process_command(
                1,
                &FenrisCommand::UploadObject {
                    path: PathBuf::from("upload.dat"),
                    data: data.clone(),
                },
                &mut current_dir,
            )
            .await;
        assert!(matches!(output, FenrisOutput::Success { .. }));

        let file_data = ops.get_object(Path::new("/upload.dat")).await.unwrap();
        assert_eq!(file_data, data);
    }

    #[tokio::test]
    async fn test_chunked_write_creates_file() {
        let (handler, ops) = create_handler();
        let current_dir = PathBuf::from("/");
        let mut transfer = handler
            .begin_object_write(
                Path::new("chunked.txt"),
                ObjectWriteMode::Write,
                10,
                &current_dir,
            )
            .await
            .unwrap();

        let output = handler
            .write_object_chunk(
                &mut transfer,
                &TransferChunk {
                    offset: 0,
                    data: b"hello".to_vec(),
                    is_last: false,
                    total_size: 10,
                },
                DEFAULT_TRANSFER_CHUNK_SIZE,
            )
            .await
            .unwrap();
        assert_eq!(output, FenrisOutput::TransferProgress { offset: 5 });

        let output = handler
            .write_object_chunk(
                &mut transfer,
                &TransferChunk {
                    offset: 5,
                    data: b"world".to_vec(),
                    is_last: true,
                    total_size: 10,
                },
                DEFAULT_TRANSFER_CHUNK_SIZE,
            )
            .await
            .unwrap();
        assert!(matches!(output, FenrisOutput::Success { .. }));

        let data = ops.get_object(Path::new("/chunked.txt")).await.unwrap();
        assert_eq!(data, b"helloworld");
    }

    #[tokio::test]
    async fn test_chunked_append_preserves_existing_content() {
        let (handler, ops) = create_handler();
        let current_dir = PathBuf::from("/");
        ops.put_object(Path::new("/log.txt"), b"existing")
            .await
            .unwrap();
        let mut transfer = handler
            .begin_object_write(
                Path::new("log.txt"),
                ObjectWriteMode::Append,
                5,
                &current_dir,
            )
            .await
            .unwrap();

        let output = handler
            .write_object_chunk(
                &mut transfer,
                &TransferChunk {
                    offset: 0,
                    data: b"+more".to_vec(),
                    is_last: true,
                    total_size: 5,
                },
                DEFAULT_TRANSFER_CHUNK_SIZE,
            )
            .await
            .unwrap();
        assert!(matches!(output, FenrisOutput::Success { .. }));

        let data = ops.get_object(Path::new("/log.txt")).await.unwrap();
        assert_eq!(data, b"existing+more");
    }

    #[tokio::test]
    async fn test_chunked_upload_writes_all_chunks() {
        let (handler, ops) = create_handler();
        let current_dir = PathBuf::from("/");
        let mut transfer = handler
            .begin_object_write(
                Path::new("upload.bin"),
                ObjectWriteMode::Upload,
                6,
                &current_dir,
            )
            .await
            .unwrap();

        handler
            .write_object_chunk(
                &mut transfer,
                &TransferChunk {
                    offset: 0,
                    data: b"abc".to_vec(),
                    is_last: false,
                    total_size: 6,
                },
                DEFAULT_TRANSFER_CHUNK_SIZE,
            )
            .await
            .unwrap();
        let output = handler
            .write_object_chunk(
                &mut transfer,
                &TransferChunk {
                    offset: 3,
                    data: b"def".to_vec(),
                    is_last: true,
                    total_size: 6,
                },
                DEFAULT_TRANSFER_CHUNK_SIZE,
            )
            .await
            .unwrap();

        assert!(matches!(output, FenrisOutput::Success { .. }));
        let data = ops.get_object(Path::new("/upload.bin")).await.unwrap();
        assert_eq!(data, b"abcdef");
    }

    #[tokio::test]
    async fn test_chunked_write_rejects_offset_mismatch() {
        let (handler, _) = create_handler();
        let current_dir = PathBuf::from("/");
        let mut transfer = handler
            .begin_object_write(
                Path::new("bad.txt"),
                ObjectWriteMode::Write,
                4,
                &current_dir,
            )
            .await
            .unwrap();

        let result = handler
            .write_object_chunk(
                &mut transfer,
                &TransferChunk {
                    offset: 2,
                    data: b"bad".to_vec(),
                    is_last: true,
                    total_size: 4,
                },
                DEFAULT_TRANSFER_CHUNK_SIZE,
            )
            .await;

        assert!(matches!(result, Err(FenrisError::InvalidRequest(_))));
    }

    #[tokio::test]
    async fn test_chunked_write_rejects_oversized_chunk() {
        let (handler, _) = create_handler();
        let current_dir = PathBuf::from("/");
        let mut transfer = handler
            .begin_object_write(
                Path::new("large.txt"),
                ObjectWriteMode::Write,
                5,
                &current_dir,
            )
            .await
            .unwrap();

        let result = handler
            .write_object_chunk(
                &mut transfer,
                &TransferChunk {
                    offset: 0,
                    data: b"large".to_vec(),
                    is_last: true,
                    total_size: 5,
                },
                4,
            )
            .await;

        assert!(matches!(result, Err(FenrisError::InvalidRequest(_))));
    }

    #[tokio::test]
    async fn test_chunked_read_emits_ordered_chunks() {
        let (handler, ops) = create_handler();
        let current_dir = PathBuf::from("/");
        let data = vec![b'x'; DEFAULT_TRANSFER_CHUNK_SIZE + 3];
        ops.put_object(Path::new("/large.txt"), &data)
            .await
            .unwrap();

        let first = handler
            .read_object_chunk(Path::new("large.txt"), &current_dir, 0)
            .await
            .unwrap();
        assert_eq!(first.offset, 0);
        assert_eq!(first.data.len(), DEFAULT_TRANSFER_CHUNK_SIZE);
        assert!(!first.is_last);
        assert_eq!(first.total_size, data.len() as u64);

        let second = handler
            .read_object_chunk(
                Path::new("large.txt"),
                &current_dir,
                DEFAULT_TRANSFER_CHUNK_SIZE as u64,
            )
            .await
            .unwrap();
        assert_eq!(second.data.len(), 3);
        assert!(second.is_last);
    }

    #[tokio::test]
    async fn test_chunked_read_handles_empty_object() {
        let (handler, ops) = create_handler();
        let current_dir = PathBuf::from("/");
        ops.put_object(Path::new("/empty.txt"), b"").await.unwrap();

        let chunk = handler
            .read_object_chunk(Path::new("empty.txt"), &current_dir, 0)
            .await
            .unwrap();

        assert!(chunk.data.is_empty());
        assert!(chunk.is_last);
        assert_eq!(chunk.total_size, 0);
    }

    #[tokio::test]
    async fn test_terminate_command() {
        let (handler, _) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let output = handler
            .process_command(1, &FenrisCommand::Terminate, &mut current_dir)
            .await;

        assert_eq!(output, FenrisOutput::Terminated);
    }

    #[tokio::test]
    async fn test_missing_object_returns_error_output() {
        let (handler, _) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let output = handler
            .process_command(
                1,
                &FenrisCommand::ReadObject {
                    path: PathBuf::from("missing.txt"),
                },
                &mut current_dir,
            )
            .await;

        assert!(matches!(output, FenrisOutput::Error { .. }));
    }
}
