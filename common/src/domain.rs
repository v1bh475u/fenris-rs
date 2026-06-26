use std::path::PathBuf;

use crate::{FenrisError, Request, RequestType};

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
