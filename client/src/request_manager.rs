use std::fs;

use common::{
    FenrisError, Result,
    proto::{Request, RequestType},
};
use tracing::{debug, warn};

pub trait RequestBuilder: Send + Sync {
    fn build_request(&self, command: &str) -> Result<Request>;
}

#[derive(Debug, Clone, Default)]
pub struct DefaultRequestManager;

impl DefaultRequestManager {
    fn build_ping(&self) -> Result<Request> {
        debug!("Building PING request");
        Ok(Request {
            command: RequestType::Ping as i32,
            filename: String::new(),
            ip_addr: 0,
            data: vec![],
        })
    }

    fn build_list_dir(&self, args: &[&str]) -> Result<Request> {
        let path = args.first().unwrap_or(&".").to_string();
        debug!("Building LIST_DIR request for:  {}", path);

        Ok(Request {
            command: RequestType::ListDir as i32,
            filename: path,
            ip_addr: 0,
            data: vec![],
        })
    }

    fn build_change_dir(&self, args: &[&str]) -> Result<Request> {
        let path = args.first().unwrap_or(&"~").to_string();
        debug!("Building CHANGE_DIR request for: {}", path);

        Ok(Request {
            command: RequestType::ChangeDir as i32,
            filename: path,
            ip_addr: 0,
            data: vec![],
        })
    }

    fn build_read_file(&self, args: &[&str]) -> Result<Request> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "read requires a filename".to_string(),
            ));
        }

        let filename = args[0].to_string();
        debug!("Building READ_FILE request for: {}", filename);

        Ok(Request {
            command: RequestType::ReadFile as i32,
            filename,
            ip_addr: 0,
            data: vec![],
        })
    }

    fn build_write_file(&self, args: &[&str]) -> Result<Request> {
        if args.len() < 2 {
            return Err(FenrisError::MissingField(
                "write requires filename as well as data".to_string(),
            ));
        }

        let filename = args[0].to_string();
        let content = args[1..].join(" ");
        debug!("Building WRITE_FILE request for: {}", filename);

        Ok(Request {
            command: RequestType::WriteFile as i32,
            filename,
            ip_addr: 0,
            data: content.into_bytes(),
        })
    }

    fn build_create_file(&self, args: &[&str]) -> Result<Request> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "create requires a filename".to_string(),
            ));
        }

        let filename = args[0].to_string();
        debug!("Building CREATE_FILE request for: {}", filename);

        Ok(Request {
            command: RequestType::CreateFile as i32,
            filename,
            ip_addr: 0,
            data: vec![],
        })
    }

    fn build_delete_file(&self, args: &[&str]) -> Result<Request> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "rm requires a filename".to_string(),
            ));
        }

        let filename = args[0].to_string();
        debug!("Building DELETE_FILE request for: {}", filename);

        Ok(Request {
            command: RequestType::DeleteFile as i32,
            filename,
            ip_addr: 0,
            data: vec![],
        })
    }

    fn build_create_dir(&self, args: &[&str]) -> Result<Request> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "mkdir requires a directory name".to_string(),
            ));
        }

        let dirname = args[0].to_string();
        debug!("Building CREATE_DIR request for: {}", dirname);

        Ok(Request {
            command: RequestType::CreateDir as i32,
            filename: dirname,
            ip_addr: 0,
            data: vec![],
        })
    }

    fn build_delete_dir(&self, args: &[&str]) -> Result<Request> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "rmdir requires a directory name".to_string(),
            ));
        }

        let dirname = args[0].to_string();
        debug!("Building DELETE_DIR request for: {}", dirname);

        Ok(Request {
            command: RequestType::DeleteDir as i32,
            filename: dirname,
            ip_addr: 0,
            data: vec![],
        })
    }

    fn build_file_info(&self, args: &[&str]) -> Result<Request> {
        if args.is_empty() {
            return Err(FenrisError::MissingField(
                "info requires a filename".to_string(),
            ));
        }

        let filename = args[0].to_string();
        debug!("Building INFO_FILE request for: {}", filename);

        Ok(Request {
            command: RequestType::InfoFile as i32,
            filename,
            ip_addr: 0,
            data: vec![],
        })
    }

    fn build_append_file(&self, args: &[&str]) -> Result<Request> {
        if args.len() < 2 {
            return Err(FenrisError::MissingField(
                "append requires filename as well as data".to_string(),
            ));
        }

        let filename = args[0].to_string();
        let content = args[1..].join(" ");
        debug!("Building APPEND_FILE request for: {}", filename);

        Ok(Request {
            command: RequestType::AppendFile as i32,
            filename,
            ip_addr: 0,
            data: content.into_bytes(),
        })
    }

    fn build_upload_file(&self, args: &[&str]) -> Result<Request> {
        if args.len() < 2 {
            return Err(FenrisError::MissingField(
                "upload requires current location as well as destination path".to_string(),
            ));
        }
        let file_path = args[0];
        let file_data = fs::read(file_path).map_err(|e| {
            FenrisError::FileOperationError(format!("Failed to read file {}: {}", file_path, e))
        })?;
        Ok(Request {
            command: RequestType::UploadFile as i32,
            filename: String::from(args[1]),
            ip_addr: 0,
            data: file_data,
        })
    }
}

impl RequestBuilder for DefaultRequestManager {
    fn build_request(&self, command: &str) -> Result<Request> {
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            return Err(FenrisError::InvalidProtocolMessage);
        }

        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "ping" => self.build_ping(),
            "ls" => self.build_list_dir(&parts[1..]),
            "cd" => self.build_change_dir(&parts[1..]),
            "read" => self.build_read_file(&parts[1..]),
            "write" => self.build_write_file(&parts[1..]),
            "create" => self.build_create_file(&parts[1..]),
            "rm" => self.build_delete_file(&parts[1..]),
            "mkdir" => self.build_create_dir(&parts[1..]),
            "rmdir" => self.build_delete_dir(&parts[1..]),
            "info" => self.build_file_info(&parts[1..]),
            "append" => self.build_append_file(&parts[1..]),
            "upload" => self.build_upload_file(&parts[1..]),
            _ => {
                warn!("Unknown command:  {}", cmd);
                Err(FenrisError::InvalidProtocolMessage)
            }
        }
    }
}

pub struct RequestManager {
    builder: Box<dyn RequestBuilder>,
}
impl RequestManager {
    pub fn build_request(&self, command: &str) -> Result<Request> {
        self.builder.build_request(command)
    }
}

impl Default for RequestManager {
    fn default() -> Self {
        Self {
            builder: Box::new(DefaultRequestManager),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn test_build_ping() {
        let manager = DefaultRequestManager;
        let request = manager.build_request("ping").unwrap();

        assert_eq!(request.command, RequestType::Ping as i32);
    }

    #[test]
    fn test_build_list_dir() {
        let manager = RequestManager::default();

        let request = manager.build_request("ls /home").unwrap();
        assert_eq!(request.command, RequestType::ListDir as i32);
        assert_eq!(request.filename, "/home");

        let request_default = manager.build_request("ls").unwrap();
        assert_eq!(request_default.command, RequestType::ListDir as i32);
        assert_eq!(request_default.filename, ".");
    }

    #[test]
    fn test_build_change_dir() {
        let manager = RequestManager::default();
        let request = manager.build_request("cd /tmp").unwrap();
        assert_eq!(request.command, RequestType::ChangeDir as i32);
        assert_eq!(request.filename, "/tmp");

        // Test without argument (defaults to "~")
        let request_default = manager.build_request("cd").unwrap();
        assert_eq!(request_default.command, RequestType::ChangeDir as i32);
        assert_eq!(request_default.filename, "~");
    }

    #[test]
    fn test_build_read_file() {
        let manager = RequestManager::default();
        let request = manager.build_request("read test.txt").unwrap();

        assert_eq!(request.command, RequestType::ReadFile as i32);
        assert_eq!(request.filename, "test.txt");

        // missing filename
        let result = manager.build_request("read");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_write_file() {
        let manager = RequestManager::default();
        let request = manager.build_request("write test.txt Hello World").unwrap();

        assert_eq!(request.command, RequestType::WriteFile as i32);
        assert_eq!(request.filename, "test.txt");
        assert_eq!(request.data, b"Hello World");

        // missing data
        let result = manager.build_request("write test.txt");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_create_file() {
        let manager = RequestManager::default();
        let request = manager.build_request("create newfile.txt").unwrap();

        assert_eq!(request.command, RequestType::CreateFile as i32);
        assert_eq!(request.filename, "newfile.txt");

        // missing filename
        let result = manager.build_request("create");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_delete_file() {
        let manager = RequestManager::default();
        let request = manager.build_request("rm oldfile.txt").unwrap();

        assert_eq!(request.command, RequestType::DeleteFile as i32);
        assert_eq!(request.filename, "oldfile.txt");

        // missing filename
        let result = manager.build_request("rm");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_create_dir() {
        let manager = RequestManager::default();
        let request = manager.build_request("mkdir newdir").unwrap();

        assert_eq!(request.command, RequestType::CreateDir as i32);
        assert_eq!(request.filename, "newdir");

        // missing dirname
        let result = manager.build_request("mkdir");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_delete_dir() {
        let manager = RequestManager::default();
        let request = manager.build_request("rmdir olddir").unwrap();

        assert_eq!(request.command, RequestType::DeleteDir as i32);
        assert_eq!(request.filename, "olddir");

        // missing dirname
        let result = manager.build_request("rmdir");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_file_info() {
        let manager = RequestManager::default();
        let request = manager.build_request("info myfile.txt").unwrap();

        assert_eq!(request.command, RequestType::InfoFile as i32);
        assert_eq!(request.filename, "myfile.txt");

        // missing filename
        let result = manager.build_request("info");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_append_file() {
        let manager = RequestManager::default();
        let request = manager.build_request("append log.txt new entry").unwrap();

        assert_eq!(request.command, RequestType::AppendFile as i32);
        assert_eq!(request.filename, "log.txt");
        assert_eq!(request.data, b"new entry");

        // missing data
        let result = manager.build_request("append log.txt");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));
    }

    #[test]
    fn test_build_upload_file() {
        let manager = RequestManager::default();

        let mut temp_path = std::env::temp_dir();
        temp_path.push("fenris_test_upload.txt");

        let test_content = b"Content to upload";
        {
            let mut file = File::create(&temp_path).expect("Failed to create temp file");
            file.write_all(test_content)
                .expect("Failed to write to temp file");
        }

        let temp_path_str = temp_path.to_str().unwrap();
        // upload <local_path> <remote_name>
        let cmd = format!("upload {} remote_file.txt", temp_path_str);

        let request = manager.build_request(&cmd).unwrap();

        assert_eq!(request.command, RequestType::UploadFile as i32);
        assert_eq!(request.filename, "remote_file.txt");
        assert_eq!(request.data, test_content);

        let _ = fs::remove_file(temp_path);

        // missing destination
        let result = manager.build_request("upload local_file");
        assert!(matches!(result.unwrap_err(), FenrisError::MissingField(_)));

        // file not found
        let result = manager.build_request("upload non_existent_file.txt dest.txt");
        assert!(matches!(
            result.unwrap_err(),
            FenrisError::FileOperationError(_)
        ));
    }

    #[test]
    fn test_invalid_command() {
        let manager = RequestManager::default();
        let result = manager.build_request("invalid");

        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            FenrisError::InvalidProtocolMessage
        ));
    }
}
