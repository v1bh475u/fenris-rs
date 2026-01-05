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
    }

    #[test]
    fn test_build_read_file() {
        let manager = RequestManager::default();
        let request = manager.build_request("read test.txt").unwrap();

        assert_eq!(request.command, RequestType::ReadFile as i32);
        assert_eq!(request.filename, "test.txt");
    }

    #[test]
    fn test_build_write_file() {
        let manager = RequestManager::default();
        let request = manager.build_request("write test.txt Hello World").unwrap();

        assert_eq!(request.command, RequestType::WriteFile as i32);
        assert_eq!(request.filename, "test.txt");
        assert_eq!(request.data, b"Hello World");
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

    #[test]
    fn test_missing_args() {
        let manager = RequestManager::default();
        let result = manager.build_request("read");

        assert!(result.is_err());
        assert!(matches!(
            result.err().unwrap(),
            FenrisError::MissingField(_)
        ));
    }
}
