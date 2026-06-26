use std::path::PathBuf;

use crate::{
    FenrisError, FileMetadata, Request, RequestType, Response, ResponseType,
    proto::{DirectoryListing, FileInfo, response},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenrisCommand {
    Ping,
    CreateObject { path: PathBuf },
    ReadObject { path: PathBuf },
    WriteObject { path: PathBuf, data: Vec<u8> },
    AppendObject { path: PathBuf, data: Vec<u8> },
    DeleteObject { path: PathBuf },
    UploadObject { path: PathBuf, data: Vec<u8> },
    ObjectInfo { path: PathBuf },
    CreateNamespace { path: PathBuf },
    ListNamespace { path: PathBuf },
    ChangeNamespace { path: PathBuf },
    DeleteNamespace { path: PathBuf },
    Terminate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenrisMetadata {
    pub name: String,
    pub size: u64,
    pub is_namespace: bool,
    pub modified_time: u64,
    pub permissions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenrisOutput {
    Pong,
    Success { message: String },
    ObjectContent { data: Vec<u8> },
    ObjectInfo { metadata: FenrisMetadata },
    NamespaceListing { entries: Vec<FenrisMetadata> },
    NamespaceChanged { path: PathBuf },
    Terminated,
    Error { message: String },
}

impl TryFrom<Request> for FenrisCommand {
    type Error = FenrisError;

    fn try_from(request: Request) -> Result<Self, Self::Error> {
        let command = RequestType::try_from(request.command)
            .map_err(|_| FenrisError::InvalidProtocolMessage)?;
        let path = PathBuf::from(request.filename);

        match command {
            RequestType::Ping => Ok(Self::Ping),
            RequestType::CreateFile => Ok(Self::CreateObject { path }),
            RequestType::ReadFile => Ok(Self::ReadObject { path }),
            RequestType::WriteFile => Ok(Self::WriteObject {
                path,
                data: request.data,
            }),
            RequestType::AppendFile => Ok(Self::AppendObject {
                path,
                data: request.data,
            }),
            RequestType::DeleteFile => Ok(Self::DeleteObject { path }),
            RequestType::InfoFile => Ok(Self::ObjectInfo { path }),
            RequestType::CreateDir => Ok(Self::CreateNamespace { path }),
            RequestType::ListDir => Ok(Self::ListNamespace { path }),
            RequestType::ChangeDir => Ok(Self::ChangeNamespace { path }),
            RequestType::DeleteDir => Ok(Self::DeleteNamespace { path }),
            RequestType::UploadFile => Ok(Self::UploadObject {
                path,
                data: request.data,
            }),
            RequestType::Terminate => Ok(Self::Terminate),
        }
    }
}

impl From<FenrisCommand> for Request {
    fn from(command: FenrisCommand) -> Self {
        match command {
            FenrisCommand::Ping => request(RequestType::Ping, PathBuf::new(), Vec::new()),
            FenrisCommand::CreateObject { path } => {
                request(RequestType::CreateFile, path, Vec::new())
            }
            FenrisCommand::ReadObject { path } => request(RequestType::ReadFile, path, Vec::new()),
            FenrisCommand::WriteObject { path, data } => request(RequestType::WriteFile, path, data),
            FenrisCommand::AppendObject { path, data } => {
                request(RequestType::AppendFile, path, data)
            }
            FenrisCommand::DeleteObject { path } => {
                request(RequestType::DeleteFile, path, Vec::new())
            }
            FenrisCommand::UploadObject { path, data } => {
                request(RequestType::UploadFile, path, data)
            }
            FenrisCommand::ObjectInfo { path } => request(RequestType::InfoFile, path, Vec::new()),
            FenrisCommand::CreateNamespace { path } => {
                request(RequestType::CreateDir, path, Vec::new())
            }
            FenrisCommand::ListNamespace { path } => {
                request(RequestType::ListDir, path, Vec::new())
            }
            FenrisCommand::ChangeNamespace { path } => {
                request(RequestType::ChangeDir, path, Vec::new())
            }
            FenrisCommand::DeleteNamespace { path } => {
                request(RequestType::DeleteDir, path, Vec::new())
            }
            FenrisCommand::Terminate => request(RequestType::Terminate, PathBuf::new(), Vec::new()),
        }
    }
}

fn request(command: RequestType, path: PathBuf, data: Vec<u8>) -> Request {
    Request {
        command: command as i32,
        filename: path.to_string_lossy().to_string(),
        ip_addr: 0,
        data,
    }
}

impl TryFrom<Response> for FenrisOutput {
    type Error = FenrisError;

    fn try_from(response: Response) -> Result<Self, FenrisError> {
        if !response.success {
            return Ok(Self::Error {
                message: response.error_message,
            });
        }

        let response_type = ResponseType::try_from(response.r#type)
            .map_err(|_| FenrisError::InvalidProtocolMessage)?;

        match response_type {
            ResponseType::Pong => Ok(Self::Pong),
            ResponseType::FileInfo => match response.details {
                Some(response::Details::FileInfo(info)) => Ok(Self::ObjectInfo {
                    metadata: info.into(),
                }),
                _ => Err(FenrisError::SerializationError(
                    "missing file info".to_string(),
                )),
            },
            ResponseType::FileContent => Ok(Self::ObjectContent {
                data: response.data,
            }),
            ResponseType::DirListing => match response.details {
                Some(response::Details::DirectoryListing(listing)) => Ok(Self::NamespaceListing {
                    entries: listing.entries.into_iter().map(FenrisMetadata::from).collect(),
                }),
                _ => Err(FenrisError::SerializationError(
                    "missing directory listing".to_string(),
                )),
            },
            ResponseType::Success => Ok(Self::Success {
                message: String::from_utf8_lossy(&response.data).to_string(),
            }),
            ResponseType::Error => Ok(Self::Error {
                message: response.error_message,
            }),
            ResponseType::Terminated => Ok(Self::Terminated),
            ResponseType::ChangedDir => Ok(Self::NamespaceChanged {
                path: PathBuf::from(String::from_utf8_lossy(&response.data).to_string()),
            }),
        }
    }
}

impl From<FenrisOutput> for Response {
    fn from(output: FenrisOutput) -> Self {
        match output {
            FenrisOutput::Pong => response(ResponseType::Pong, true, String::new(), vec![], None),
            FenrisOutput::Success { message } => response(
                ResponseType::Success,
                true,
                String::new(),
                message.into_bytes(),
                None,
            ),
            FenrisOutput::ObjectContent { data } => {
                response(ResponseType::FileContent, true, String::new(), data, None)
            }
            FenrisOutput::ObjectInfo { metadata } => response(
                ResponseType::FileInfo,
                true,
                String::new(),
                vec![],
                Some(response::Details::FileInfo(metadata.into())),
            ),
            FenrisOutput::NamespaceListing { entries } => response(
                ResponseType::DirListing,
                true,
                String::new(),
                vec![],
                Some(response::Details::DirectoryListing(DirectoryListing {
                    entries: entries.into_iter().map(FileInfo::from).collect(),
                })),
            ),
            FenrisOutput::NamespaceChanged { path } => response(
                ResponseType::ChangedDir,
                true,
                String::new(),
                path.to_string_lossy().as_bytes().to_vec(),
                None,
            ),
            FenrisOutput::Terminated => {
                response(ResponseType::Terminated, true, String::new(), vec![], None)
            }
            FenrisOutput::Error { message } => {
                response(ResponseType::Error, false, message, vec![], None)
            }
        }
    }
}

impl From<FileMetadata> for FenrisMetadata {
    fn from(metadata: FileMetadata) -> Self {
        Self {
            name: metadata.name,
            size: metadata.size,
            is_namespace: metadata.is_directory,
            modified_time: metadata.modified_time,
            permissions: metadata.permissions,
        }
    }
}

impl From<FileInfo> for FenrisMetadata {
    fn from(info: FileInfo) -> Self {
        Self {
            name: info.name,
            size: info.size,
            is_namespace: info.is_directory,
            modified_time: info.modified_time,
            permissions: info.permissions,
        }
    }
}

impl From<FenrisMetadata> for FileInfo {
    fn from(metadata: FenrisMetadata) -> Self {
        Self {
            name: metadata.name,
            size: metadata.size,
            is_directory: metadata.is_namespace,
            modified_time: metadata.modified_time,
            permissions: metadata.permissions,
        }
    }
}

fn response(
    response_type: ResponseType,
    success: bool,
    error_message: String,
    data: Vec<u8>,
    details: Option<response::Details>,
) -> Response {
    Response {
        r#type: response_type as i32,
        success,
        error_message,
        data,
        details,
    }
}
