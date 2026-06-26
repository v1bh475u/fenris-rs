use std::{fs, path::PathBuf};

use common::{FenrisCommand, FenrisError, ObjectWriteMode, Result};
use tracing::{debug, warn};

#[derive(Debug, Clone, Default)]
pub struct RequestManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientCommandPlan {
    Single(FenrisCommand),
    ChunkedRead {
        path: PathBuf,
    },
    ChunkedInlineWrite {
        path: PathBuf,
        mode: ObjectWriteMode,
        data: Vec<u8>,
    },
    ChunkedUpload {
        source: PathBuf,
        destination: PathBuf,
        total_size: u64,
    },
}

impl RequestManager {
    pub fn build_request(&self, command: &str) -> Result<ClientCommandPlan> {
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            return Err(FenrisError::InvalidProtocolMessage);
        }

        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "ping" => self.build_ping(),
            "ls" => self.build_list_namespace(&parts[1..]),
            "cd" => self.build_change_namespace(&parts[1..]),
            "read" => self.build_read_object(&parts[1..]),
            "write" => self.build_write_object(&parts[1..]),
            "create" => self.build_create_object(&parts[1..]),
            "rm" => self.build_delete_object(&parts[1..]),
            "mkdir" => self.build_create_namespace(&parts[1..]),
            "rmdir" => self.build_delete_namespace(&parts[1..]),
            "info" => self.build_object_info(&parts[1..]),
            "append" => self.build_append_object(&parts[1..]),
            "upload" => self.build_upload_object(&parts[1..]),
            _ => {
                warn!("Unknown command:  {}", cmd);
                Err(FenrisError::InvalidProtocolMessage)
            }
        }
    }

    fn build_ping(&self) -> Result<ClientCommandPlan> {
        debug!("Building PING command");
        Ok(ClientCommandPlan::Single(FenrisCommand::Ping))
    }

    fn build_list_namespace(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        let path = args.first().unwrap_or(&".");
        debug!("Building LIST_NAMESPACE command for: {}", path);
        Ok(ClientCommandPlan::Single(FenrisCommand::ListNamespace {
            path: PathBuf::from(path),
        }))
    }

    fn build_change_namespace(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        let path = args.first().unwrap_or(&"~");
        debug!("Building CHANGE_NAMESPACE command for: {}", path);
        Ok(ClientCommandPlan::Single(FenrisCommand::ChangeNamespace {
            path: PathBuf::from(path),
        }))
    }

    fn build_read_object(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "read requires a filename".to_string(),
            ));
        }

        let path = PathBuf::from(args[0]);
        debug!("Building READ_OBJECT command for: {}", path.display());
        Ok(ClientCommandPlan::ChunkedRead { path })
    }

    fn build_write_object(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        if args.len() < 2 {
            return Err(FenrisError::MissingField(
                "write requires filename as well as data".to_string(),
            ));
        }

        let path = PathBuf::from(args[0]);
        let content = args[1..].join(" ");
        debug!("Building WRITE_OBJECT command for: {}", path.display());
        Ok(ClientCommandPlan::ChunkedInlineWrite {
            path,
            mode: ObjectWriteMode::Write,
            data: content.into_bytes(),
        })
    }

    fn build_create_object(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "create requires a filename".to_string(),
            ));
        }

        let path = PathBuf::from(args[0]);
        debug!("Building CREATE_OBJECT command for: {}", path.display());
        Ok(ClientCommandPlan::Single(FenrisCommand::CreateObject {
            path,
        }))
    }

    fn build_delete_object(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "rm requires a filename".to_string(),
            ));
        }

        let path = PathBuf::from(args[0]);
        debug!("Building DELETE_OBJECT command for: {}", path.display());
        Ok(ClientCommandPlan::Single(FenrisCommand::DeleteObject {
            path,
        }))
    }

    fn build_create_namespace(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "mkdir requires a directory name".to_string(),
            ));
        }

        let path = PathBuf::from(args[0]);
        debug!("Building CREATE_NAMESPACE command for: {}", path.display());
        Ok(ClientCommandPlan::Single(FenrisCommand::CreateNamespace {
            path,
        }))
    }

    fn build_delete_namespace(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "rmdir requires a directory name".to_string(),
            ));
        }

        let path = PathBuf::from(args[0]);
        debug!("Building DELETE_NAMESPACE command for: {}", path.display());
        Ok(ClientCommandPlan::Single(FenrisCommand::DeleteNamespace {
            path,
        }))
    }

    fn build_object_info(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "info requires a filename".to_string(),
            ));
        }

        let path = PathBuf::from(args[0]);
        debug!("Building OBJECT_INFO command for: {}", path.display());
        Ok(ClientCommandPlan::Single(FenrisCommand::ObjectInfo {
            path,
        }))
    }

    fn build_append_object(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        if args.len() < 2 {
            return Err(FenrisError::MissingField(
                "append requires filename as well as data".to_string(),
            ));
        }

        let path = PathBuf::from(args[0]);
        let content = args[1..].join(" ");
        debug!("Building APPEND_OBJECT command for: {}", path.display());
        Ok(ClientCommandPlan::ChunkedInlineWrite {
            path,
            mode: ObjectWriteMode::Append,
            data: content.into_bytes(),
        })
    }

    fn build_upload_object(&self, args: &[&str]) -> Result<ClientCommandPlan> {
        if args.len() < 2 {
            return Err(FenrisError::MissingField(
                "upload requires current location as well as destination path".to_string(),
            ));
        }

        let source = PathBuf::from(args[0]);
        let metadata = fs::metadata(&source).map_err(|e| {
            FenrisError::FileOperationError(format!(
                "Failed to inspect file {}: {}",
                source.display(),
                e
            ))
        })?;

        Ok(ClientCommandPlan::ChunkedUpload {
            source,
            destination: PathBuf::from(args[1]),
            total_size: metadata.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_build_ping() {
        let manager = RequestManager;
        let command = manager.build_request("ping").unwrap();

        assert_eq!(command, ClientCommandPlan::Single(FenrisCommand::Ping));
    }

    #[test]
    fn test_build_list_dir() {
        let manager = RequestManager;

        let command = manager.build_request("ls /home").unwrap();
        assert_eq!(
            command,
            ClientCommandPlan::Single(FenrisCommand::ListNamespace {
                path: PathBuf::from("/home")
            })
        );

        let command_default = manager.build_request("ls").unwrap();
        assert_eq!(
            command_default,
            ClientCommandPlan::Single(FenrisCommand::ListNamespace {
                path: PathBuf::from(".")
            })
        );
    }

    #[test]
    fn test_build_change_dir() {
        let manager = RequestManager;
        let command = manager.build_request("cd /tmp").unwrap();
        assert_eq!(
            command,
            ClientCommandPlan::Single(FenrisCommand::ChangeNamespace {
                path: PathBuf::from("/tmp")
            })
        );

        let command_default = manager.build_request("cd").unwrap();
        assert_eq!(
            command_default,
            ClientCommandPlan::Single(FenrisCommand::ChangeNamespace {
                path: PathBuf::from("~")
            })
        );
    }

    #[test]
    fn test_build_read_file() {
        let manager = RequestManager;
        let command = manager.build_request("read test.txt").unwrap();

        assert_eq!(
            command,
            ClientCommandPlan::ChunkedRead {
                path: PathBuf::from("test.txt")
            }
        );

        let result = manager.build_request("read");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_write_file() {
        let manager = RequestManager;
        let command = manager.build_request("write test.txt Hello World").unwrap();

        assert_eq!(
            command,
            ClientCommandPlan::ChunkedInlineWrite {
                path: PathBuf::from("test.txt"),
                mode: ObjectWriteMode::Write,
                data: b"Hello World".to_vec()
            }
        );

        let result = manager.build_request("write test.txt");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_create_file() {
        let manager = RequestManager;
        let command = manager.build_request("create newfile.txt").unwrap();

        assert_eq!(
            command,
            ClientCommandPlan::Single(FenrisCommand::CreateObject {
                path: PathBuf::from("newfile.txt")
            })
        );

        let result = manager.build_request("create");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_delete_file() {
        let manager = RequestManager;
        let command = manager.build_request("rm oldfile.txt").unwrap();

        assert_eq!(
            command,
            ClientCommandPlan::Single(FenrisCommand::DeleteObject {
                path: PathBuf::from("oldfile.txt")
            })
        );

        let result = manager.build_request("rm");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_create_dir() {
        let manager = RequestManager;
        let command = manager.build_request("mkdir newdir").unwrap();

        assert_eq!(
            command,
            ClientCommandPlan::Single(FenrisCommand::CreateNamespace {
                path: PathBuf::from("newdir")
            })
        );

        let result = manager.build_request("mkdir");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_delete_dir() {
        let manager = RequestManager;
        let command = manager.build_request("rmdir olddir").unwrap();

        assert_eq!(
            command,
            ClientCommandPlan::Single(FenrisCommand::DeleteNamespace {
                path: PathBuf::from("olddir")
            })
        );

        let result = manager.build_request("rmdir");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_file_info() {
        let manager = RequestManager;
        let command = manager.build_request("info myfile.txt").unwrap();

        assert_eq!(
            command,
            ClientCommandPlan::Single(FenrisCommand::ObjectInfo {
                path: PathBuf::from("myfile.txt")
            })
        );

        let result = manager.build_request("info");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_append_file() {
        let manager = RequestManager;
        let command = manager.build_request("append log.txt new entry").unwrap();

        assert_eq!(
            command,
            ClientCommandPlan::ChunkedInlineWrite {
                path: PathBuf::from("log.txt"),
                mode: ObjectWriteMode::Append,
                data: b"new entry".to_vec()
            }
        );

        let result = manager.build_request("append log.txt");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_upload_file() {
        let manager = RequestManager;

        let mut temp_path = std::env::temp_dir();
        temp_path.push("fenris_test_upload.txt");

        let test_content = b"Content to upload";
        {
            let mut file = File::create(&temp_path).expect("Failed to create temp file");
            file.write_all(test_content)
                .expect("Failed to write to temp file");
        }

        let temp_path_str = temp_path.to_str().unwrap();
        let cmd = format!("upload {} remote_file.txt", temp_path_str);

        let command = manager.build_request(&cmd).unwrap();

        assert_eq!(
            command,
            ClientCommandPlan::ChunkedUpload {
                source: temp_path.clone(),
                destination: PathBuf::from("remote_file.txt"),
                total_size: test_content.len() as u64
            }
        );

        let _ = fs::remove_file(temp_path);

        let result = manager.build_request("upload local_file");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));

        let result = manager.build_request("upload non_existent_file.txt dest.txt");
        assert!(matches!(
            result.unwrap_err(),
            FenrisError::FileOperationError(_)
        ));
    }

    #[test]
    fn test_invalid_command() {
        let manager = RequestManager;
        let result = manager.build_request("invalid");

        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            FenrisError::InvalidProtocolMessage
        ));
    }
}
