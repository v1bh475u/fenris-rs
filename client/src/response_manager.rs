use chrono;
use common::proto::{DirectoryListing, FileInfo, Response, ResponseType, response};
use tracing::debug;

pub struct FormattedResponse {
    pub success: bool,
    pub message: String,
    pub details: Option<String>,
    pub current_dir: Option<String>,
}

pub trait ResponseFormatter: Send + Sync {
    fn format_response(&self, response: &Response) -> FormattedResponse;

    fn extract_current_dir(&self, response: &Response) -> Option<String> {
        if response.success && !response.data.is_empty() {
            Some(String::from_utf8_lossy(&response.data).to_string())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DefaultResponseFormatter;

impl DefaultResponseFormatter {
    pub fn new() -> Self {
        Self
    }

    fn format_pong(&self, _response: &Response) -> FormattedResponse {
        FormattedResponse {
            success: true,
            message: "PONG - Server is alive! ".to_string(),
            details: None,
            current_dir: None,
        }
    }

    fn format_success(&self, response: &Response) -> FormattedResponse {
        let msg = if response.data.is_empty() {
            "Operation successful".to_string()
        } else {
            String::from_utf8_lossy(&response.data).to_string()
        };

        FormattedResponse {
            success: true,
            message: msg,
            details: None,
            current_dir: None,
        }
    }

    fn format_change_dir(&self, response: &Response) -> FormattedResponse {
        let dir = if response.data.is_empty() {
            "/".to_string()
        } else {
            String::from_utf8_lossy(&response.data).to_string()
        };

        FormattedResponse {
            success: true,
            message: format!("Changed directory to {}", dir),
            details: None,
            current_dir: Some(dir),
        }
    }

    fn format_file_content(&self, response: &Response) -> FormattedResponse {
        let content = String::from_utf8_lossy(&response.data).to_string();
        let preview = if content.len() > 500 {
            format!("{}...  ({} bytes total)", &content[..500], content.len())
        } else {
            content.clone()
        };

        FormattedResponse {
            success: true,
            message: format!("File content ({} bytes):", response.data.len()),
            details: Some(preview),
            current_dir: None,
        }
    }

    fn format_file_info(&self, response: &Response) -> FormattedResponse {
        if let Some(ref details) = response.details {
            if let response::Details::FileInfo(file_info) = details {
                return self.format_file_info_detail(file_info);
            }
        }

        FormattedResponse {
            success: true,
            message: "File info received".to_string(),
            details: None,
            current_dir: None,
        }
    }

    fn format_file_info_detail(&self, info: &FileInfo) -> FormattedResponse {
        let file_type = if info.is_directory {
            "Directory"
        } else {
            "File"
        };

        let size_str = if info.is_directory {
            "-".to_string()
        } else {
            format_size(info.size)
        };

        let perms = format_permissions(info.permissions);
        let modified = format_timestamp(info.modified_time);

        let details = format!(
            "{}\nType: {}\nSize: {}\nPermissions: {}\nModified: {}",
            info.name, file_type, size_str, perms, modified
        );

        FormattedResponse {
            success: true,
            message: "File information: ".to_string(),
            details: Some(details),
            current_dir: None,
        }
    }

    fn format_dir_listing(&self, response: &Response) -> FormattedResponse {
        if let Some(ref details) = response.details {
            if let response::Details::DirectoryListing(dir_listing) = details {
                return self.format_dir_listing_detail(dir_listing);
            }
        }

        FormattedResponse {
            success: true,
            message: "Empty directory".to_string(),
            details: None,
            current_dir: None,
        }
    }

    fn format_dir_listing_detail(&self, listing: &DirectoryListing) -> FormattedResponse {
        if listing.entries.is_empty() {
            return FormattedResponse {
                success: true,
                message: "Directory is empty".to_string(),
                details: None,
                current_dir: None,
            };
        }

        let mut output = String::new();
        output.push_str(&format!("Found {} entries:\n\n", listing.entries.len()));

        output.push_str(&format!(
            "{:40} {: >10} {:>12} {}\n",
            "Name", "Type", "Size", "Modified"
        ));
        output.push_str(&"-".repeat(80));
        output.push('\n');

        for entry in &listing.entries {
            let file_type = if entry.is_directory { "DIR" } else { "FILE" };
            let size = if entry.is_directory {
                "-".to_string()
            } else {
                format_size(entry.size)
            };
            let modified = format_timestamp(entry.modified_time);

            output.push_str(&format!(
                "{:40} {:>10} {:>12} {}\n",
                entry.name, file_type, size, modified
            ));
        }

        FormattedResponse {
            success: true,
            message: "Directory listing:".to_string(),
            details: Some(output),
            current_dir: None,
        }
    }

    fn format_error(&self, response: &Response) -> FormattedResponse {
        FormattedResponse {
            success: false,
            message: response.error_message.clone(),
            details: None,
            current_dir: None,
        }
    }
}

impl ResponseFormatter for DefaultResponseFormatter {
    fn format_response(&self, response: &Response) -> FormattedResponse {
        debug!("Formatting response type: {:?}", response.r#type);

        if !response.success {
            return FormattedResponse {
                success: false,
                message: response.error_message.clone(),
                details: None,
                current_dir: None,
            };
        }

        let response_type = ResponseType::try_from(response.r#type).unwrap_or(ResponseType::Error);

        match response_type {
            ResponseType::Pong => self.format_pong(response),
            ResponseType::Success => self.format_success(response),
            ResponseType::ChangedDir => self.format_change_dir(response),
            ResponseType::FileContent => self.format_file_content(response),
            ResponseType::FileInfo => self.format_file_info(response),
            ResponseType::DirListing => self.format_dir_listing(response),
            ResponseType::Error => self.format_error(response),
            ResponseType::Terminated => FormattedResponse {
                success: true,
                message: "Server terminated".to_string(),
                details: None,
                current_dir: None,
            },
        }
    }
}

pub struct ResponseManager {
    formatter: Box<dyn ResponseFormatter>,
}

impl ResponseManager {
    pub fn new(formatter: Box<dyn ResponseFormatter>) -> Self {
        Self { formatter }
    }

    pub fn format_response(&self, response: &Response) -> FormattedResponse {
        self.formatter.format_response(response)
    }
}

impl Default for ResponseManager {
    fn default() -> Self {
        Self {
            formatter: Box::new(DefaultResponseFormatter),
        }
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

fn format_permissions(perms: u32) -> String {
    let user = (perms >> 6) & 0x7;
    let group = (perms >> 3) & 0x7;
    let other = perms & 0x7;

    let format_triple = |bits: u32| -> String {
        format!(
            "{}{}{}",
            if bits & 0x4 != 0 { 'r' } else { '-' },
            if bits & 0x2 != 0 { 'w' } else { '-' },
            if bits & 0x1 != 0 { 'x' } else { '-' },
        )
    };

    format!(
        "{}{}{} ({:o})",
        format_triple(user),
        format_triple(group),
        format_triple(other),
        perms
    )
}

fn format_timestamp(timestamp: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let datetime = UNIX_EPOCH + Duration::from_secs(timestamp);
    let datetime: chrono::DateTime<chrono::Local> = datetime.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_formatter() {
        let formatter = DefaultResponseFormatter::new();

        let response = Response {
            r#type: ResponseType::Success as i32,
            success: true,
            error_message: String::new(),
            data: b"Test data".to_vec(),
            details: None,
        };

        let formatted = formatter.format_response(&response);
        assert!(formatted.success);
        assert!(formatted.message.contains("Test data"));
    }

    #[test]
    fn test_response_manager_wrapper() {
        let manager = ResponseManager::default();

        let response = Response {
            r#type: ResponseType::Pong as i32,
            success: true,
            error_message: String::new(),
            data: vec![],
            details: None,
        };

        let formatted = manager.format_response(&response);
        assert!(formatted.success);
        assert!(formatted.message.contains("PONG"));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
    }

    #[test]
    fn test_format_permissions() {
        assert_eq!(format_permissions(0o755), "rwxr-xr-x (755)");
        assert_eq!(format_permissions(0o644), "rw-r--r-- (644)");
    }
}
