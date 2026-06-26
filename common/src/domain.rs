use std::path::PathBuf;

use crate::{
    FenrisError, FileMetadata, Request, RequestType, Response, ResponseType,
    proto::{
        DirectoryListing, FileInfo, TransferAck, TransferChunk as ProtoTransferChunk, TransferMode,
        TransferStart, request, response,
    },
};

pub const DEFAULT_TRANSFER_CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectWriteMode {
    Write,
    Append,
    Upload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferChunk {
    pub offset: u64,
    pub data: Vec<u8>,
    pub is_last: bool,
    pub total_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenrisCommand {
    Ping,
    CreateObject {
        path: PathBuf,
    },
    ReadObject {
        path: PathBuf,
    },
    WriteObject {
        path: PathBuf,
        data: Vec<u8>,
    },
    AppendObject {
        path: PathBuf,
        data: Vec<u8>,
    },
    DeleteObject {
        path: PathBuf,
    },
    UploadObject {
        path: PathBuf,
        data: Vec<u8>,
    },
    BeginObjectWrite {
        path: PathBuf,
        mode: ObjectWriteMode,
        total_size: u64,
    },
    WriteObjectChunk(TransferChunk),
    ObjectInfo {
        path: PathBuf,
    },
    CreateNamespace {
        path: PathBuf,
    },
    ListNamespace {
        path: PathBuf,
    },
    ChangeNamespace {
        path: PathBuf,
    },
    DeleteNamespace {
        path: PathBuf,
    },
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
    Success {
        message: String,
    },
    ObjectContent {
        data: Vec<u8>,
        total_size: u64,
        truncated: bool,
    },
    ObjectContentChunk(TransferChunk),
    ObjectInfo {
        metadata: FenrisMetadata,
    },
    NamespaceListing {
        entries: Vec<FenrisMetadata>,
    },
    NamespaceChanged {
        path: PathBuf,
    },
    TransferReady {
        chunk_size: usize,
    },
    TransferProgress {
        offset: u64,
    },
    Terminated,
    Error {
        message: String,
    },
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
            RequestType::BeginObjectWrite => match request.details {
                Some(request::Details::TransferStart(start)) => Ok(Self::BeginObjectWrite {
                    path: PathBuf::from(start.filename),
                    mode: ObjectWriteMode::try_from(
                        TransferMode::try_from(start.mode)
                            .map_err(|_| FenrisError::InvalidProtocolMessage)?,
                    )?,
                    total_size: start.total_size,
                }),
                _ => Err(FenrisError::InvalidProtocolMessage),
            },
            RequestType::WriteObjectChunk => match request.details {
                Some(request::Details::TransferChunk(chunk)) => {
                    Ok(Self::WriteObjectChunk(chunk.into()))
                }
                _ => Err(FenrisError::InvalidProtocolMessage),
            },
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
            FenrisCommand::WriteObject { path, data } => {
                request(RequestType::WriteFile, path, data)
            }
            FenrisCommand::AppendObject { path, data } => {
                request(RequestType::AppendFile, path, data)
            }
            FenrisCommand::DeleteObject { path } => {
                request(RequestType::DeleteFile, path, Vec::new())
            }
            FenrisCommand::UploadObject { path, data } => {
                request(RequestType::UploadFile, path, data)
            }
            FenrisCommand::BeginObjectWrite {
                path,
                mode,
                total_size,
            } => request_with_details(
                RequestType::BeginObjectWrite,
                path.clone(),
                Vec::new(),
                Some(request::Details::TransferStart(TransferStart {
                    filename: path.to_string_lossy().to_string(),
                    mode: TransferMode::from(mode) as i32,
                    total_size,
                })),
            ),
            FenrisCommand::WriteObjectChunk(chunk) => request_with_details(
                RequestType::WriteObjectChunk,
                PathBuf::new(),
                Vec::new(),
                Some(request::Details::TransferChunk(chunk.into())),
            ),
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
    request_with_details(command, path, data, None)
}

fn request_with_details(
    command: RequestType,
    path: PathBuf,
    data: Vec<u8>,
    details: Option<request::Details>,
) -> Request {
    Request {
        command: command as i32,
        filename: path.to_string_lossy().to_string(),
        ip_addr: 0,
        data,
        details,
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
            ResponseType::FileContent => {
                let total_size = response.data.len() as u64;
                Ok(Self::ObjectContent {
                    data: response.data,
                    total_size,
                    truncated: false,
                })
            }
            ResponseType::DirListing => match response.details {
                Some(response::Details::DirectoryListing(listing)) => Ok(Self::NamespaceListing {
                    entries: listing
                        .entries
                        .into_iter()
                        .map(FenrisMetadata::from)
                        .collect(),
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
            ResponseType::TransferReady => match response.details {
                Some(response::Details::TransferAck(ack)) => Ok(Self::TransferReady {
                    chunk_size: ack.chunk_size as usize,
                }),
                _ => Err(FenrisError::SerializationError(
                    "missing transfer ack".to_string(),
                )),
            },
            ResponseType::TransferProgress => match response.details {
                Some(response::Details::TransferAck(ack)) => {
                    Ok(Self::TransferProgress { offset: ack.offset })
                }
                _ => Err(FenrisError::SerializationError(
                    "missing transfer ack".to_string(),
                )),
            },
            ResponseType::FileContentChunk => match response.details {
                Some(response::Details::TransferChunk(chunk)) => {
                    Ok(Self::ObjectContentChunk(chunk.into()))
                }
                _ => Err(FenrisError::SerializationError(
                    "missing transfer chunk".to_string(),
                )),
            },
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
            FenrisOutput::ObjectContent { data, .. } => {
                response(ResponseType::FileContent, true, String::new(), data, None)
            }
            FenrisOutput::ObjectContentChunk(chunk) => {
                let data = chunk.data.clone();
                response(
                    ResponseType::FileContentChunk,
                    true,
                    String::new(),
                    data,
                    Some(response::Details::TransferChunk(chunk.into())),
                )
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
            FenrisOutput::TransferReady { chunk_size } => response(
                ResponseType::TransferReady,
                true,
                String::new(),
                vec![],
                Some(response::Details::TransferAck(TransferAck {
                    offset: 0,
                    chunk_size: chunk_size.min(u32::MAX as usize) as u32,
                })),
            ),
            FenrisOutput::TransferProgress { offset } => response(
                ResponseType::TransferProgress,
                true,
                String::new(),
                vec![],
                Some(response::Details::TransferAck(TransferAck {
                    offset,
                    chunk_size: 0,
                })),
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

impl TryFrom<TransferMode> for ObjectWriteMode {
    type Error = FenrisError;

    fn try_from(mode: TransferMode) -> Result<Self, Self::Error> {
        match mode {
            TransferMode::TransferWrite => Ok(Self::Write),
            TransferMode::TransferAppend => Ok(Self::Append),
            TransferMode::TransferUpload => Ok(Self::Upload),
            TransferMode::Unspecified => Err(FenrisError::InvalidProtocolMessage),
        }
    }
}

impl From<ObjectWriteMode> for TransferMode {
    fn from(mode: ObjectWriteMode) -> Self {
        match mode {
            ObjectWriteMode::Write => Self::TransferWrite,
            ObjectWriteMode::Append => Self::TransferAppend,
            ObjectWriteMode::Upload => Self::TransferUpload,
        }
    }
}

impl From<ProtoTransferChunk> for TransferChunk {
    fn from(chunk: ProtoTransferChunk) -> Self {
        Self {
            offset: chunk.offset,
            data: chunk.data,
            is_last: chunk.is_last,
            total_size: chunk.total_size,
        }
    }
}

impl From<TransferChunk> for ProtoTransferChunk {
    fn from(chunk: TransferChunk) -> Self {
        Self {
            offset: chunk.offset,
            data: chunk.data,
            is_last: chunk.is_last,
            total_size: chunk.total_size,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProtobufCodec, ProtocolCodec};

    #[test]
    fn protobuf_request_decodes_into_domain_commands() {
        let cases = [
            (
                request(RequestType::Ping, PathBuf::new(), Vec::new()),
                FenrisCommand::Ping,
            ),
            (
                request(RequestType::CreateFile, PathBuf::from("a.txt"), Vec::new()),
                FenrisCommand::CreateObject {
                    path: PathBuf::from("a.txt"),
                },
            ),
            (
                request(RequestType::ReadFile, PathBuf::from("a.txt"), Vec::new()),
                FenrisCommand::ReadObject {
                    path: PathBuf::from("a.txt"),
                },
            ),
            (
                request(
                    RequestType::WriteFile,
                    PathBuf::from("a.txt"),
                    b"data".to_vec(),
                ),
                FenrisCommand::WriteObject {
                    path: PathBuf::from("a.txt"),
                    data: b"data".to_vec(),
                },
            ),
            (
                request(
                    RequestType::AppendFile,
                    PathBuf::from("a.txt"),
                    b"more".to_vec(),
                ),
                FenrisCommand::AppendObject {
                    path: PathBuf::from("a.txt"),
                    data: b"more".to_vec(),
                },
            ),
            (
                request(RequestType::DeleteFile, PathBuf::from("a.txt"), Vec::new()),
                FenrisCommand::DeleteObject {
                    path: PathBuf::from("a.txt"),
                },
            ),
            (
                request(RequestType::InfoFile, PathBuf::from("a.txt"), Vec::new()),
                FenrisCommand::ObjectInfo {
                    path: PathBuf::from("a.txt"),
                },
            ),
            (
                request(RequestType::CreateDir, PathBuf::from("dir"), Vec::new()),
                FenrisCommand::CreateNamespace {
                    path: PathBuf::from("dir"),
                },
            ),
            (
                request(RequestType::ListDir, PathBuf::from("dir"), Vec::new()),
                FenrisCommand::ListNamespace {
                    path: PathBuf::from("dir"),
                },
            ),
            (
                request(RequestType::ChangeDir, PathBuf::from("dir"), Vec::new()),
                FenrisCommand::ChangeNamespace {
                    path: PathBuf::from("dir"),
                },
            ),
            (
                request(RequestType::DeleteDir, PathBuf::from("dir"), Vec::new()),
                FenrisCommand::DeleteNamespace {
                    path: PathBuf::from("dir"),
                },
            ),
            (
                request(
                    RequestType::UploadFile,
                    PathBuf::from("a.txt"),
                    b"upload".to_vec(),
                ),
                FenrisCommand::UploadObject {
                    path: PathBuf::from("a.txt"),
                    data: b"upload".to_vec(),
                },
            ),
            (
                request(RequestType::Terminate, PathBuf::new(), Vec::new()),
                FenrisCommand::Terminate,
            ),
        ];

        for (request, expected) in cases {
            assert_eq!(FenrisCommand::try_from(request).unwrap(), expected);
        }
    }

    #[test]
    fn domain_commands_encode_into_expected_protobuf_requests() {
        let cases = [
            (
                FenrisCommand::Ping,
                (RequestType::Ping, String::new(), Vec::new()),
            ),
            (
                FenrisCommand::CreateObject {
                    path: PathBuf::from("a.txt"),
                },
                (RequestType::CreateFile, "a.txt".to_string(), Vec::new()),
            ),
            (
                FenrisCommand::WriteObject {
                    path: PathBuf::from("a.txt"),
                    data: b"data".to_vec(),
                },
                (
                    RequestType::WriteFile,
                    "a.txt".to_string(),
                    b"data".to_vec(),
                ),
            ),
            (
                FenrisCommand::Terminate,
                (RequestType::Terminate, String::new(), Vec::new()),
            ),
        ];

        for (command, (request_type, filename, data)) in cases {
            let request = Request::from(command);
            assert_eq!(request.command, request_type as i32);
            assert_eq!(request.filename, filename);
            assert_eq!(request.data, data);
        }
    }

    #[test]
    fn protobuf_response_decodes_into_domain_outputs() {
        let metadata = FenrisMetadata {
            name: "a.txt".to_string(),
            size: 4,
            is_namespace: false,
            modified_time: 5,
            permissions: 0o644,
        };

        let cases = [
            (
                response(ResponseType::Pong, true, String::new(), vec![], None),
                FenrisOutput::Pong,
            ),
            (
                response(
                    ResponseType::Success,
                    true,
                    String::new(),
                    b"ok".to_vec(),
                    None,
                ),
                FenrisOutput::Success {
                    message: "ok".to_string(),
                },
            ),
            (
                response(
                    ResponseType::FileContent,
                    true,
                    String::new(),
                    b"body".to_vec(),
                    None,
                ),
                FenrisOutput::ObjectContent {
                    data: b"body".to_vec(),
                    total_size: 4,
                    truncated: false,
                },
            ),
            (
                response(
                    ResponseType::FileInfo,
                    true,
                    String::new(),
                    vec![],
                    Some(response::Details::FileInfo(metadata.clone().into())),
                ),
                FenrisOutput::ObjectInfo {
                    metadata: metadata.clone(),
                },
            ),
            (
                response(
                    ResponseType::DirListing,
                    true,
                    String::new(),
                    vec![],
                    Some(response::Details::DirectoryListing(DirectoryListing {
                        entries: vec![metadata.clone().into()],
                    })),
                ),
                FenrisOutput::NamespaceListing {
                    entries: vec![metadata],
                },
            ),
            (
                response(
                    ResponseType::ChangedDir,
                    true,
                    String::new(),
                    b"/tmp".to_vec(),
                    None,
                ),
                FenrisOutput::NamespaceChanged {
                    path: PathBuf::from("/tmp"),
                },
            ),
            (
                response(ResponseType::Terminated, true, String::new(), vec![], None),
                FenrisOutput::Terminated,
            ),
            (
                response(ResponseType::Error, false, "nope".to_string(), vec![], None),
                FenrisOutput::Error {
                    message: "nope".to_string(),
                },
            ),
        ];

        for (response, expected) in cases {
            assert_eq!(FenrisOutput::try_from(response).unwrap(), expected);
        }
    }

    #[test]
    fn domain_outputs_encode_into_expected_protobuf_responses() {
        let metadata = FenrisMetadata {
            name: "dir".to_string(),
            size: 0,
            is_namespace: true,
            modified_time: 7,
            permissions: 0o755,
        };

        let output = FenrisOutput::NamespaceListing {
            entries: vec![metadata.clone()],
        };
        let response = Response::from(output);
        assert_eq!(response.r#type, ResponseType::DirListing as i32);
        assert!(response.success);
        assert!(matches!(
            response.details,
            Some(response::Details::DirectoryListing(_))
        ));

        let response = Response::from(FenrisOutput::Error {
            message: "bad".to_string(),
        });
        assert_eq!(response.r#type, ResponseType::Error as i32);
        assert!(!response.success);
        assert_eq!(response.error_message, "bad");

        let response = Response::from(FenrisOutput::ObjectInfo { metadata });
        assert_eq!(response.r#type, ResponseType::FileInfo as i32);
        assert!(matches!(
            response.details,
            Some(response::Details::FileInfo(_))
        ));
    }

    #[test]
    fn transfer_commands_map_to_protobuf_details() {
        let begin = FenrisCommand::BeginObjectWrite {
            path: PathBuf::from("big.bin"),
            mode: ObjectWriteMode::Upload,
            total_size: 7,
        };
        let request = Request::from(begin.clone());
        assert_eq!(request.command, RequestType::BeginObjectWrite as i32);
        assert_eq!(FenrisCommand::try_from(request).unwrap(), begin);

        let chunk = TransferChunk {
            offset: 3,
            data: b"data".to_vec(),
            is_last: true,
            total_size: 7,
        };
        let request = Request::from(FenrisCommand::WriteObjectChunk(chunk.clone()));
        assert_eq!(request.command, RequestType::WriteObjectChunk as i32);
        assert!(matches!(
            request.details,
            Some(request::Details::TransferChunk(_))
        ));
        assert_eq!(
            FenrisCommand::try_from(request).unwrap(),
            FenrisCommand::WriteObjectChunk(chunk)
        );
    }

    #[test]
    fn transfer_outputs_map_to_protobuf_details() {
        let ready = Response::from(FenrisOutput::TransferReady { chunk_size: 4096 });
        assert_eq!(ready.r#type, ResponseType::TransferReady as i32);
        assert_eq!(
            FenrisOutput::try_from(ready).unwrap(),
            FenrisOutput::TransferReady { chunk_size: 4096 }
        );

        let progress = Response::from(FenrisOutput::TransferProgress { offset: 9 });
        assert_eq!(progress.r#type, ResponseType::TransferProgress as i32);
        assert_eq!(
            FenrisOutput::try_from(progress).unwrap(),
            FenrisOutput::TransferProgress { offset: 9 }
        );

        let chunk = TransferChunk {
            offset: 0,
            data: b"body".to_vec(),
            is_last: true,
            total_size: 4,
        };
        let response = Response::from(FenrisOutput::ObjectContentChunk(chunk.clone()));
        assert_eq!(response.r#type, ResponseType::FileContentChunk as i32);
        assert_eq!(
            FenrisOutput::try_from(response).unwrap(),
            FenrisOutput::ObjectContentChunk(chunk)
        );
    }

    #[test]
    fn invalid_transfer_details_are_rejected() {
        let request = request_with_details(
            RequestType::BeginObjectWrite,
            PathBuf::from("a.txt"),
            Vec::new(),
            None,
        );
        assert!(matches!(
            FenrisCommand::try_from(request),
            Err(FenrisError::InvalidProtocolMessage)
        ));

        let request = request_with_details(
            RequestType::WriteObjectChunk,
            PathBuf::new(),
            Vec::new(),
            None,
        );
        assert!(matches!(
            FenrisCommand::try_from(request),
            Err(FenrisError::InvalidProtocolMessage)
        ));

        let response = response(
            ResponseType::FileContentChunk,
            true,
            String::new(),
            vec![],
            None,
        );
        assert!(matches!(
            FenrisOutput::try_from(response),
            Err(FenrisError::SerializationError(_))
        ));
    }

    #[test]
    fn protobuf_codec_round_trips_domain_command() {
        let command = FenrisCommand::WriteObject {
            path: PathBuf::from("a.txt"),
            data: b"payload".to_vec(),
        };

        let encoded = ProtobufCodec::encode(&command).unwrap();
        let decoded: FenrisCommand = ProtobufCodec::decode(&encoded).unwrap();

        assert_eq!(decoded, command);
    }

    #[test]
    fn protobuf_codec_round_trips_domain_output() {
        let output = FenrisOutput::NamespaceChanged {
            path: PathBuf::from("/data"),
        };

        let encoded = ProtobufCodec::encode(&output).unwrap();
        let decoded: FenrisOutput = ProtobufCodec::decode(&encoded).unwrap();

        assert_eq!(decoded, output);
    }

    #[test]
    fn invalid_request_and_response_types_are_rejected() {
        let request = Request {
            command: 99,
            filename: String::new(),
            ip_addr: 0,
            data: vec![],
            details: None,
        };
        assert!(matches!(
            FenrisCommand::try_from(request),
            Err(FenrisError::InvalidProtocolMessage)
        ));

        let response = Response {
            r#type: 99,
            success: true,
            error_message: String::new(),
            data: vec![],
            details: None,
        };
        assert!(matches!(
            FenrisOutput::try_from(response),
            Err(FenrisError::InvalidProtocolMessage)
        ));
    }
}
