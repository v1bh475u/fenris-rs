use common::{
    FenrisError, FileOperations, Request, RequestType, Response, ResponseType, Result,
    proto::response,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error};

use crate::client_info::ClientId;

pub struct RequestHandler {
    file_ops: Arc<dyn FileOperations>,
}

impl RequestHandler {
    pub fn new(file_ops: Arc<dyn FileOperations>) -> Self {
        Self { file_ops }
    }

    fn resolve_path(&self, path: &str, current_dir: &Path) -> PathBuf {
        if path.is_empty() || path == "." {
            current_dir.to_path_buf()
        } else if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            current_dir.join(path)
        }
    }

    pub async fn process_request(
        &self,
        client_id: ClientId,
        request: &Request,
        current_dir: &mut PathBuf,
    ) -> Response {
        debug!(
            "Processing request from client {} in dir {:?}:  command={}",
            client_id, current_dir, request.command
        );

        let request_type = match RequestType::try_from(request.command) {
            Ok(rt) => rt,
            Err(_) => {
                return self.error_response("Invalid request type");
            }
        };

        match self
            .handle_request(request_type, request, current_dir)
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("Request failed: {}", e);
                self.error_response(&e.to_string())
            }
        }
    }

    async fn handle_request(
        &self,
        request_type: RequestType,
        request: &Request,
        current_dir: &mut PathBuf,
    ) -> Result<Response> {
        match request_type {
            RequestType::Ping => self.handle_ping().await,
            RequestType::CreateFile => {
                self.handle_create_file(&request.filename, current_dir)
                    .await
            }
            RequestType::ReadFile => self.handle_read_file(&request.filename, current_dir).await,
            RequestType::WriteFile => {
                self.handle_write_file(&request.filename, &request.data, current_dir)
                    .await
            }
            RequestType::DeleteFile => {
                self.handle_delete_file(&request.filename, current_dir)
                    .await
            }
            RequestType::AppendFile => {
                self.handle_append_file(&request.filename, &request.data, current_dir)
                    .await
            }
            RequestType::UploadFile => {
                self.handle_upload(&request.filename, &request.data, current_dir)
                    .await
            }
            RequestType::InfoFile => self.handle_file_info(&request.filename, current_dir).await,
            RequestType::CreateDir => self.handle_create_dir(&request.filename, current_dir).await,
            RequestType::ListDir => self.handle_list_dir(&request.filename, current_dir).await,
            RequestType::DeleteDir => self.handle_delete_dir(&request.filename, current_dir).await,
            RequestType::ChangeDir => self.handle_change_dir(&request.filename, current_dir).await,
            RequestType::Terminate => Err(FenrisError::InvalidRequest(
                "Terminate request should be handled separately".to_string(),
            )),
        }
    }

    async fn handle_ping(&self) -> Result<Response> {
        Ok(Response {
            r#type: ResponseType::Pong as i32,
            success: true,
            error_message: String::new(),
            data: vec![],
            details: None,
        })
    }

    async fn handle_create_file(&self, filename: &str, current_dir: &Path) -> Result<Response> {
        let path = self.resolve_path(filename, current_dir);
        self.file_ops.create_file(&path).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("File created: {}", path.to_string_lossy()).into_bytes(),
            details: None,
        })
    }

    async fn handle_read_file(&self, filename: &str, current_dir: &Path) -> Result<Response> {
        let path = self.resolve_path(filename, current_dir);
        let data = self.file_ops.read_file(&path).await?;

        Ok(Response {
            r#type: ResponseType::FileContent as i32,
            success: true,
            error_message: String::new(),
            data,
            details: None,
        })
    }

    async fn handle_write_file(
        &self,
        filename: &str,
        data: &[u8],
        current_dir: &Path,
    ) -> Result<Response> {
        let path = self.resolve_path(filename, current_dir);
        self.file_ops.write_file(&path, data).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("File written: {} bytes", data.len()).into_bytes(),
            details: None,
        })
    }

    async fn handle_delete_file(&self, filename: &str, current_dir: &Path) -> Result<Response> {
        let path = self.resolve_path(filename, current_dir);
        self.file_ops.delete_file(&path).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("File deleted: {}", path.to_string_lossy()).into_bytes(),
            details: None,
        })
    }

    async fn handle_append_file(
        &self,
        filename: &str,
        data: &[u8],
        current_dir: &Path,
    ) -> Result<Response> {
        let path = self.resolve_path(filename, current_dir);
        self.file_ops.append_file(&path, data).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!(
                "Appended {} bytes to {}",
                data.len(),
                path.to_string_lossy()
            )
            .into_bytes(),
            details: None,
        })
    }

    async fn handle_upload(
        &self,
        filename: &str,
        data: &[u8],
        current_dir: &Path,
    ) -> Result<Response> {
        let path = self.resolve_path(filename, current_dir);
        self.file_ops.write_file(&path, data).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!(
                "Uploaded {} bytes to {}",
                data.len(),
                path.to_string_lossy()
            )
            .into_bytes(),
            details: None,
        })
    }

    async fn handle_file_info(&self, filename: &str, current_dir: &Path) -> Result<Response> {
        let path = self.resolve_path(filename, current_dir);
        let metadata = self.file_ops.file_info(&path).await?;

        let file_info = common::proto::FileInfo {
            name: metadata.name,
            size: metadata.size,
            is_directory: metadata.is_directory,
            modified_time: metadata.modified_time,
            permissions: metadata.permissions,
        };

        Ok(Response {
            r#type: ResponseType::FileInfo as i32,
            success: true,
            error_message: String::new(),
            data: vec![],
            details: Some(response::Details::FileInfo(file_info)),
        })
    }

    async fn handle_create_dir(&self, dirname: &str, current_dir: &Path) -> Result<Response> {
        let path = self.resolve_path(dirname, current_dir);
        self.file_ops.create_dir(&path).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("Directory created: {}", path.to_string_lossy()).into_bytes(),
            details: None,
        })
    }

    async fn handle_list_dir(&self, dirname: &str, current_dir: &Path) -> Result<Response> {
        let path = self.resolve_path(dirname, current_dir);

        let entries = self.file_ops.list_dir(&path).await?;

        let file_entries: Vec<common::proto::FileInfo> = entries
            .into_iter()
            .map(|e| common::proto::FileInfo {
                name: e.name,
                size: e.size,
                is_directory: e.is_directory,
                modified_time: e.modified_time,
                permissions: e.permissions,
            })
            .collect();

        let listing = common::proto::DirectoryListing {
            entries: file_entries,
        };

        Ok(Response {
            r#type: ResponseType::DirListing as i32,
            success: true,
            error_message: String::new(),
            data: vec![],
            details: Some(response::Details::DirectoryListing(listing)),
        })
    }

    async fn handle_delete_dir(&self, dirname: &str, current_dir: &Path) -> Result<Response> {
        let path = self.resolve_path(dirname, current_dir);
        self.file_ops.delete_dir(&path).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("Directory deleted: {}", path.to_string_lossy()).into_bytes(),
            details: None,
        })
    }

    async fn handle_change_dir(
        &self,
        dirname: &str,
        current_dir: &mut PathBuf,
    ) -> Result<Response> {
        let target_path = if dirname.is_empty() || dirname == "~" {
            PathBuf::from("/")
        } else if dirname == "." {
            current_dir.clone()
        } else if dirname == ".." {
            current_dir
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/"))
        } else if dirname.starts_with('/') {
            PathBuf::from(dirname)
        } else {
            current_dir.join(dirname)
        };

        if !self.file_ops.is_dir(&target_path).await {
            return Err(FenrisError::FileOperationError(
                "Not a directory".to_string(),
            ));
        }

        *current_dir = target_path.clone();

        let dir_str = target_path.to_string_lossy().to_string();
        Ok(Response {
            r#type: ResponseType::ChangedDir as i32,
            success: true,
            error_message: String::new(),
            data: dir_str.as_bytes().to_vec(),
            details: None,
        })
    }

    fn error_response(&self, message: &str) -> Response {
        Response {
            r#type: ResponseType::Error as i32,
            success: false,
            error_message: message.to_string(),
            data: vec![],
            details: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{FenrisError, FileMetadata};
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
            for d in dirs.iter() {
                if d.parent() == Some(path) {
                    entries.push(FileMetadata {
                        name: d
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
            for (f, data) in files.iter() {
                if f.parent() == Some(path) {
                    entries.push(FileMetadata {
                        name: f
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
        let request = Request {
            command: RequestType::Ping as i32,
            filename: "".to_string(),
            data: vec![],
            ip_addr: 0,
        };

        let response = handler.process_request(1, &request, &mut current_dir).await;
        assert_eq!(response.r#type, ResponseType::Pong as i32);
        assert!(response.success);
    }

    #[tokio::test]
    async fn test_create_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/home");
        // Pre-create /home for realism, though mock doesn't strictly enforce parent existence for simple ops
        ops.create_dir(&current_dir).await.unwrap();

        let request = Request {
            command: RequestType::CreateFile as i32,
            filename: "test.txt".to_string(),
            data: vec![],
            ip_addr: 0,
        };

        let response = handler.process_request(1, &request, &mut current_dir).await;
        assert!(response.success);

        let files = ops.files.lock().unwrap();
        assert!(files.contains_key(&PathBuf::from("/home/test.txt")));
    }

    #[tokio::test]
    async fn test_write_and_read_file() {
        let (handler, _) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let data = b"Hello, World!".to_vec();
        let request_write = Request {
            command: RequestType::WriteFile as i32,
            filename: "hello.txt".to_string(),
            data: data.clone(),
            ip_addr: 0,
        };

        let resp_write = handler
            .process_request(1, &request_write, &mut current_dir)
            .await;
        assert!(resp_write.success);

        let request_read = Request {
            command: RequestType::ReadFile as i32,
            filename: "hello.txt".to_string(),
            data: vec![],
            ip_addr: 0,
        };

        let resp_read = handler
            .process_request(1, &request_read, &mut current_dir)
            .await;
        assert!(resp_read.success);
        assert_eq!(resp_read.data, data);
        assert_eq!(resp_read.r#type, ResponseType::FileContent as i32);
    }

    #[tokio::test]
    async fn test_append_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.write_file(Path::new("/log.txt"), b"Init")
            .await
            .unwrap();

        let request = Request {
            command: RequestType::AppendFile as i32,
            filename: "log.txt".to_string(),
            data: b" - More".to_vec(),
            ip_addr: 0,
        };

        let response = handler.process_request(1, &request, &mut current_dir).await;
        assert!(response.success);

        let content = ops.read_file(Path::new("/log.txt")).await.unwrap();
        assert_eq!(content, b"Init - More");
    }

    #[tokio::test]
    async fn test_delete_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.create_file(Path::new("/temp.txt")).await.unwrap();

        let request = Request {
            command: RequestType::DeleteFile as i32,
            filename: "temp.txt".to_string(),
            data: vec![],
            ip_addr: 0,
        };

        let response = handler.process_request(1, &request, &mut current_dir).await;
        assert!(response.success);
        assert!(!ops.exists(Path::new("/temp.txt")).await);
    }

    #[tokio::test]
    async fn test_change_dir() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.create_dir(Path::new("/data")).await.unwrap();

        // cd data
        let req1 = Request {
            command: RequestType::ChangeDir as i32,
            filename: "data".to_string(),
            data: vec![],
            ip_addr: 0,
        };
        let resp1 = handler.process_request(1, &req1, &mut current_dir).await;
        assert!(resp1.success);
        assert_eq!(current_dir, PathBuf::from("/data"));

        // cd ..
        let req2 = Request {
            command: RequestType::ChangeDir as i32,
            filename: "..".to_string(),
            data: vec![],
            ip_addr: 0,
        };
        let resp2 = handler.process_request(1, &req2, &mut current_dir).await;
        assert!(resp2.success);
        assert_eq!(current_dir, PathBuf::from("/"));

        // cd to non-existent
        let req3 = Request {
            command: RequestType::ChangeDir as i32,
            filename: "missing".to_string(),
            data: vec![],
            ip_addr: 0,
        };
        let resp3 = handler.process_request(1, &req3, &mut current_dir).await;
        assert!(!resp3.success);
        assert_eq!(current_dir, PathBuf::from("/")); // Should not change
    }

    #[tokio::test]
    async fn test_list_dir() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        ops.create_dir(Path::new("/data")).await.unwrap();
        ops.create_file(Path::new("/data/f1.txt")).await.unwrap();
        ops.create_dir(Path::new("/data/sub")).await.unwrap();

        let request = Request {
            command: RequestType::ListDir as i32,
            filename: "data".to_string(),
            data: vec![],
            ip_addr: 0,
        };

        let response = handler.process_request(1, &request, &mut current_dir).await;
        assert!(response.success);

        if let Some(common::proto::response::Details::DirectoryListing(listing)) = response.details
        {
            assert_eq!(listing.entries.len(), 2);
            let names: Vec<String> = listing.entries.iter().map(|e| e.name.clone()).collect();
            assert!(names.contains(&"f1.txt".to_string()));
            assert!(names.contains(&"sub".to_string()));
        } else {
            panic!("Expected DirectoryListing details");
        }
    }

    #[tokio::test]
    async fn test_create_and_delete_dir() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let req_create = Request {
            command: RequestType::CreateDir as i32,
            filename: "newdir".to_string(),
            data: vec![],
            ip_addr: 0,
        };
        let resp_create = handler
            .process_request(1, &req_create, &mut current_dir)
            .await;
        assert!(resp_create.success);
        assert!(ops.is_dir(Path::new("/newdir")).await);

        let req_delete = Request {
            command: RequestType::DeleteDir as i32,
            filename: "newdir".to_string(),
            data: vec![],
            ip_addr: 0,
        };
        let resp_delete = handler
            .process_request(1, &req_delete, &mut current_dir)
            .await;
        assert!(resp_delete.success);
        assert!(!ops.is_dir(Path::new("/newdir")).await);
    }

    #[tokio::test]
    async fn test_file_info() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");
        ops.create_file(Path::new("/info.txt")).await.unwrap();

        let request = Request {
            command: RequestType::InfoFile as i32,
            filename: "info.txt".to_string(),
            data: vec![],
            ip_addr: 0,
        };
        let response = handler.process_request(1, &request, &mut current_dir).await;
        assert!(response.success);
        if let Some(common::proto::response::Details::FileInfo(info)) = response.details {
            assert_eq!(info.name, "info.txt");
            assert!(!info.is_directory);
        } else {
            panic!("Expected FileInfo details");
        }
    }

    #[tokio::test]
    async fn test_upload_file() {
        let (handler, ops) = create_handler();
        let mut current_dir = PathBuf::from("/");

        let data = b"Upload Data".to_vec();
        let request = Request {
            command: RequestType::UploadFile as i32,
            filename: "upload.dat".to_string(),
            data: data.clone(),
            ip_addr: 0,
        };
        let response = handler.process_request(1, &request, &mut current_dir).await;
        assert!(response.success);

        let file_data = ops.read_file(Path::new("/upload.dat")).await.unwrap();
        assert_eq!(file_data, data);
    }
}
