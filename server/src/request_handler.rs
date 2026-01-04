use common::{
    FenrisError, FileOperations, Request, RequestType, Response, ResponseType, Result,
    proto::response,
};
use std::path::Path;
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

    pub async fn process_request(&self, client_id: ClientId, request: &Request) -> Response {
        debug!(
            "Processing request from client {}:  command={}",
            client_id, request.command
        );

        let request_type = match RequestType::try_from(request.command) {
            Ok(rt) => rt,
            Err(_) => {
                return self.error_response("Invalid request type");
            }
        };

        match self.handle_request(request_type, request).await {
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
    ) -> Result<Response> {
        match request_type {
            RequestType::Ping => self.handle_ping().await,
            RequestType::CreateFile => self.handle_create_file(&request.filename).await,
            RequestType::ReadFile => self.handle_read_file(&request.filename).await,
            RequestType::WriteFile => {
                self.handle_write_file(&request.filename, &request.data)
                    .await
            }
            RequestType::DeleteFile => self.handle_delete_file(&request.filename).await,
            RequestType::AppendFile => {
                self.handle_append_file(&request.filename, &request.data)
                    .await
            }
            RequestType::InfoFile => self.handle_file_info(&request.filename).await,
            RequestType::CreateDir => self.handle_create_dir(&request.filename).await,
            RequestType::ListDir => self.handle_list_dir(&request.filename).await,
            RequestType::DeleteDir => self.handle_delete_dir(&request.filename).await,
            RequestType::ChangeDir => self.handle_change_dir(&request.filename).await,
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

    async fn handle_create_file(&self, filename: &str) -> Result<Response> {
        let path = Path::new(filename);
        self.file_ops.create_file(path).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("File created: {}", filename).into_bytes(),
            details: None,
        })
    }

    async fn handle_read_file(&self, filename: &str) -> Result<Response> {
        let path = Path::new(filename);
        let data = self.file_ops.read_file(path).await?;

        Ok(Response {
            r#type: ResponseType::FileContent as i32,
            success: true,
            error_message: String::new(),
            data,
            details: None,
        })
    }

    async fn handle_write_file(&self, filename: &str, data: &[u8]) -> Result<Response> {
        let path = Path::new(filename);
        self.file_ops.write_file(path, data).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("File written: {} bytes", data.len()).into_bytes(),
            details: None,
        })
    }

    async fn handle_delete_file(&self, filename: &str) -> Result<Response> {
        let path = Path::new(filename);
        self.file_ops.delete_file(path).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("File deleted: {}", filename).into_bytes(),
            details: None,
        })
    }

    async fn handle_append_file(&self, filename: &str, data: &[u8]) -> Result<Response> {
        let path = Path::new(filename);
        self.file_ops.append_file(path, data).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("Appended {} bytes to {}", data.len(), filename).into_bytes(),
            details: None,
        })
    }

    async fn handle_file_info(&self, filename: &str) -> Result<Response> {
        let path = Path::new(filename);
        let metadata = self.file_ops.file_info(path).await?;

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

    async fn handle_create_dir(&self, dirname: &str) -> Result<Response> {
        let path = Path::new(dirname);
        self.file_ops.create_dir(path).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("Directory created: {}", dirname).into_bytes(),
            details: None,
        })
    }

    async fn handle_list_dir(&self, dirname: &str) -> Result<Response> {
        let path = if dirname.is_empty() || dirname == "." {
            Path::new("/")
        } else {
            Path::new(dirname)
        };

        let entries = self.file_ops.list_dir(path).await?;

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

    async fn handle_delete_dir(&self, dirname: &str) -> Result<Response> {
        let path = Path::new(dirname);
        self.file_ops.delete_dir(path).await?;

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: format!("Directory deleted: {}", dirname).into_bytes(),
            details: None,
        })
    }

    async fn handle_change_dir(&self, dirname: &str) -> Result<Response> {
        let path = Path::new(dirname);

        if !self.file_ops.is_dir(path).await {
            return Err(FenrisError::FileOperationError(
                "Not a directory".to_string(),
            ));
        }

        Ok(Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: dirname.as_bytes().to_vec(),
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
