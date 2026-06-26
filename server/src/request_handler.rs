use common::{FenrisCommand, FenrisMetadata, FenrisOutput, FenrisError, FileOperations, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error};

pub struct RequestHandler {
    file_ops: Arc<dyn FileOperations>,
}

impl RequestHandler {
    pub fn new(file_ops: Arc<dyn FileOperations>) -> Self {
        Self { file_ops }
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
        self.file_ops.create_file(&path).await?;

        Ok(FenrisOutput::Success {
            message: format!("File created: {}", path.to_string_lossy()),
        })
    }

    async fn handle_read_object(&self, path: &Path, current_dir: &Path) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        let data = self.file_ops.read_file(&path).await?;

        Ok(FenrisOutput::ObjectContent { data })
    }

    async fn handle_write_object(
        &self,
        path: &Path,
        data: &[u8],
        current_dir: &Path,
    ) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.file_ops.write_file(&path, data).await?;

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
        self.file_ops.append_file(&path, data).await?;

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
        self.file_ops.delete_file(&path).await?;

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
        self.file_ops.write_file(&path, data).await?;

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
        let metadata = self.file_ops.file_info(&path).await?;

        Ok(FenrisOutput::ObjectInfo {
            metadata: metadata.into(),
        })
    }

    async fn handle_create_namespace(
        &self,
        path: &Path,
        current_dir: &Path,
    ) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.file_ops.create_dir(&path).await?;

        Ok(FenrisOutput::Success {
            message: format!("Directory created: {}", path.to_string_lossy()),
        })
    }

    async fn handle_list_namespace(
        &self,
        path: &Path,
        current_dir: &Path,
    ) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        let entries = self
            .file_ops
            .list_dir(&path)
            .await?
            .into_iter()
            .map(FenrisMetadata::from)
            .collect();

        Ok(FenrisOutput::NamespaceListing { entries })
    }

    async fn handle_delete_namespace(
        &self,
        path: &Path,
        current_dir: &Path,
    ) -> Result<FenrisOutput> {
        let path = self.resolve_path(path, current_dir);
        self.file_ops.delete_dir(&path).await?;

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

        if !self.file_ops.is_dir(&target_path).await {
            return Err(FenrisError::FileOperationError(
                "Not a directory".to_string(),
            ));
        }

        *current_dir = target_path.clone();

        Ok(FenrisOutput::NamespaceChanged { path: target_path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::FileMetadata;
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockFileOps {
        files: Mutex<HashMap<PathBuf, Vec<u8>>>,
        dirs: Mutex<HashSet<PathBuf>>,
    }

    impl MockFileOps {
        fn new() -> Self {
            let mut dirs = HashSet::new();
            dirs.insert(PathBuf::from("/"));
            Self {
                files: Mutex::new(HashMap::new()),
                dirs: Mutex::new(dirs),
            }
        }
    }

    #[async_trait::async_trait]
    impl FileOperations for MockFileOps {
        async fn create_file(&self, path: &Path) -> Result<()> {
            let mut files = self.files.lock().unwrap();
            files.insert(path.to_path_buf(), Vec::new());
            Ok(())
        }

        async fn read_file(&self, path: &Path) -> Result<Vec<u8>> {
            let files = self.files.lock().unwrap();
            files
                .get(path)
                .cloned()
                .ok_or_else(|| FenrisError::FileOperationError("File not found".into()))
        }

        async fn write_file(&self, path: &Path, data: &[u8]) -> Result<()> {
            let mut files = self.files.lock().unwrap();
            files.insert(path.to_path_buf(), data.to_vec());
            Ok(())
        }

        async fn append_file(&self, path: &Path, data: &[u8]) -> Result<()> {
            let mut files = self.files.lock().unwrap();
            if let Some(file_data) = files.get_mut(path) {
                file_data.extend_from_slice(data);
                Ok(())
            } else {
                Err(FenrisError::FileOperationError("File not found".into()))
            }
        }

        async fn delete_file(&self, path: &Path) -> Result<()> {
            let mut files = self.files.lock().unwrap();
            if files.remove(path).is_some() {
                Ok(())
            } else {
                Err(FenrisError::FileOperationError("File not found".into()))
            }
        }

        async fn file_info(&self, path: &Path) -> Result<FileMetadata> {
            let files = self.files.lock().unwrap();
            if let Some(data) = files.get(path) {
                return Ok(FileMetadata {
                    name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    size: data.len() as u64,
                    is_directory: false,
                    modified_time: 0,
                    permissions: 0o644,
                });
            }

            let dirs = self.dirs.lock().unwrap();
            if dirs.contains(path) {
                return Ok(FileMetadata {
                    name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    size: 0,
                    is_directory: true,
                    modified_time: 0,
                    permissions: 0o755,
                });
            }

            Err(FenrisError::FileOperationError("NotFound".into()))
        }

        async fn create_dir(&self, path: &Path) -> Result<()> {
            self.dirs.lock().unwrap().insert(path.to_path_buf());
            Ok(())
        }

        async fn list_dir(&self, path: &Path) -> Result<Vec<FileMetadata>> {
            let mut entries = Vec::new();
            let dirs = self.dirs.lock().unwrap();
            for dir in dirs.iter() {
                if dir.parent() == Some(path) {
                    entries.push(FileMetadata {
                        name: dir
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        size: 0,
                        is_directory: true,
                        modified_time: 0,
                        permissions: 0o755,
                    });
                }
            }

            let files = self.files.lock().unwrap();
            for (file, data) in files.iter() {
                if file.parent() == Some(path) {
                    entries.push(FileMetadata {
                        name: file
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        size: data.len() as u64,
                        is_directory: false,
                        modified_time: 0,
                        permissions: 0o644,
                    });
                }
            }

            Ok(entries)
        }

        async fn delete_dir(&self, path: &Path) -> Result<()> {
            if self.dirs.lock().unwrap().remove(path) {
                Ok(())
            } else {
                Err(FenrisError::FileOperationError("Dir not found".into()))
            }
        }

        async fn exists(&self, path: &Path) -> bool {
            self.files.lock().unwrap().contains_key(path)
                || self.dirs.lock().unwrap().contains(path)
        }

        async fn is_dir(&self, path: &Path) -> bool {
            self.dirs.lock().unwrap().contains(path)
        }

        async fn is_file(&self, path: &Path) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }
    }

    fn create_handler() -> (RequestHandler, Arc<MockFileOps>) {
        let file_ops = Arc::new(MockFileOps::new());
        let handler = RequestHandler::new(file_ops.clone());
        (handler, file_ops)
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
        ops.create_dir(&current_dir).await.unwrap();

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

        let files = ops.files.lock().unwrap();
        assert!(files.contains_key(&PathBuf::from("/home/test.txt")));
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

        assert_eq!(read_output, FenrisOutput::ObjectContent { data });
    }

    #[tokio::test]
    async fn test_append_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.write_file(Path::new("/log.txt"), b"Init")
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

        let content = ops.read_file(Path::new("/log.txt")).await.unwrap();
        assert_eq!(content, b"Init - More");
    }

    #[tokio::test]
    async fn test_delete_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.create_file(Path::new("/temp.txt")).await.unwrap();

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

        ops.create_dir(Path::new("/data")).await.unwrap();

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

        ops.create_dir(Path::new("/data")).await.unwrap();
        ops.create_file(Path::new("/data/f1.txt")).await.unwrap();
        ops.create_dir(Path::new("/data/sub")).await.unwrap();

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
        assert!(ops.is_dir(Path::new("/newdir")).await);

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
        assert!(!ops.is_dir(Path::new("/newdir")).await);
    }

    #[tokio::test]
    async fn test_file_info() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");
        ops.create_file(Path::new("/info.txt")).await.unwrap();

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

        let file_data = ops.read_file(Path::new("/upload.dat")).await.unwrap();
        assert_eq!(file_data, data);
    }
}
