use std::path::PathBuf;

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
