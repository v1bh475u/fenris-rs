use common::{FenrisMetadata, FenrisOutput};
use tracing::debug;

pub struct FormattedResponse {
    pub success: bool,
    pub message: String,
    pub details: Option<String>,
    pub current_dir: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ResponseManager;

impl ResponseManager {
    pub fn format_response(&self, response: &FenrisOutput) -> FormattedResponse {
        debug!("Formatting domain response: {:?}", response);

        match response {
            FenrisOutput::Pong => self.format_pong(),
            FenrisOutput::Success { message } => self.format_success(message),
            FenrisOutput::ObjectContent {
                data,
                total_size,
                truncated,
            } => self.format_object_content(data, *total_size, *truncated),
            FenrisOutput::ObjectContentChunk(chunk) => {
                self.format_object_content(&chunk.data, chunk.total_size, !chunk.is_last)
            }
            FenrisOutput::ObjectInfo { metadata } => self.format_object_info(metadata),
            FenrisOutput::NamespaceListing { entries } => self.format_namespace_listing(entries),
            FenrisOutput::NamespaceChanged { path } => {
                self.format_namespace_changed(&path.to_string_lossy())
            }
            FenrisOutput::TransferReady { chunk_size } => FormattedResponse {
                success: true,
                message: format!("Transfer ready ({} byte chunks)", chunk_size),
                details: None,
                current_dir: None,
            },
            FenrisOutput::TransferProgress { offset } => FormattedResponse {
                success: true,
                message: format!("Transferred {} bytes", offset),
                details: None,
                current_dir: None,
            },
            FenrisOutput::Terminated => FormattedResponse {
                success: true,
                message: "Server terminated".to_string(),
                details: None,
                current_dir: None,
            },
            FenrisOutput::Error { message } => FormattedResponse {
                success: false,
                message: message.clone(),
                details: None,
                current_dir: None,
            },
        }
    }

    fn format_pong(&self) -> FormattedResponse {
        FormattedResponse {
            success: true,
            message: "PONG - Server is alive! ".to_string(),
            details: None,
            current_dir: None,
        }
    }

    fn format_success(&self, message: &str) -> FormattedResponse {
        let message = if message.is_empty() {
            "Operation successful".to_string()
        } else {
            message.to_string()
        };

        FormattedResponse {
            success: true,
            message,
            details: None,
            current_dir: None,
        }
    }

    fn format_namespace_changed(&self, path: &str) -> FormattedResponse {
        let path = if path.is_empty() { "/" } else { path };

        FormattedResponse {
            success: true,
            message: format!("Changed directory to {}", path),
            details: None,
            current_dir: Some(path.to_string()),
        }
    }

    fn format_object_content(
        &self,
        data: &[u8],
        total_size: u64,
        already_truncated: bool,
    ) -> FormattedResponse {
        let content = String::from_utf8_lossy(data).to_string();
        let preview_text: String = content.chars().take(500).collect();
        let display_truncated = already_truncated || content.chars().count() > 500;
        let preview = if display_truncated {
            format!("{}...  ({} bytes total)", preview_text, total_size)
        } else {
            content
        };

        FormattedResponse {
            success: true,
            message: format!("File content ({} bytes):", total_size),
            details: Some(preview),
            current_dir: None,
        }
    }

    fn format_object_info(&self, metadata: &FenrisMetadata) -> FormattedResponse {
        let object_type = if metadata.is_namespace {
            "Directory"
        } else {
            "File"
        };

        let size = if metadata.is_namespace {
            "-".to_string()
        } else {
            format_size(metadata.size)
        };

        let permissions = format_permissions(metadata.permissions);
        let modified = format_timestamp(metadata.modified_time);

        let details = format!(
            "{}\nType: {}\nSize: {}\nPermissions: {}\nModified: {}",
            metadata.name, object_type, size, permissions, modified
        );

        FormattedResponse {
            success: true,
            message: "File information: ".to_string(),
            details: Some(details),
            current_dir: None,
        }
    }

    fn format_namespace_listing(&self, entries: &[FenrisMetadata]) -> FormattedResponse {
        if entries.is_empty() {
            return FormattedResponse {
                success: true,
                message: "Directory is empty".to_string(),
                details: None,
                current_dir: None,
            };
        }

        let mut output = String::new();
        output.push_str(&format!("Found {} entries:\n\n", entries.len()));
        output.push_str(&format!(
            "{:40} {: >10} {:>12} {}\n",
            "Name", "Type", "Size", "Modified"
        ));
        output.push_str(&"-".repeat(80));
        output.push('\n');

        for entry in entries {
            let object_type = if entry.is_namespace { "DIR" } else { "FILE" };
            let size = if entry.is_namespace {
                "-".to_string()
            } else {
                format_size(entry.size)
            };
            let modified = format_timestamp(entry.modified_time);

            output.push_str(&format!(
                "{:40} {:>10} {:>12} {}\n",
                entry.name, object_type, size, modified
            ));
        }

        FormattedResponse {
            success: true,
            message: "Directory listing:".to_string(),
            details: Some(output),
            current_dir: None,
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
        let formatter = ResponseManager;

        let response = FenrisOutput::Success {
            message: "Test data".to_string(),
        };

        let formatted = formatter.format_response(&response);
        assert!(formatted.success);
        assert!(formatted.message.contains("Test data"));
    }

    #[test]
    fn test_response_manager_wrapper() {
        let manager = ResponseManager;

        let formatted = manager.format_response(&FenrisOutput::Pong);
        assert!(formatted.success);
        assert!(formatted.message.contains("PONG"));
    }

    #[test]
    fn test_format_object_content() {
        let manager = ResponseManager;

        let formatted = manager.format_response(&FenrisOutput::ObjectContent {
            data: b"hello".to_vec(),
            total_size: 5,
            truncated: false,
        });

        assert!(formatted.success);
        assert!(formatted.message.contains("5 bytes"));
        assert_eq!(formatted.details.as_deref(), Some("hello"));
    }

    #[test]
    fn test_format_truncated_object_content() {
        let manager = ResponseManager;

        let formatted = manager.format_response(&FenrisOutput::ObjectContent {
            data: b"preview".to_vec(),
            total_size: 2048,
            truncated: true,
        });

        assert!(formatted.success);
        assert!(formatted.message.contains("2048 bytes"));
        assert!(formatted.details.unwrap().contains("2048 bytes total"));
    }

    #[test]
    fn test_format_object_info() {
        let manager = ResponseManager;

        let formatted = manager.format_response(&FenrisOutput::ObjectInfo {
            metadata: FenrisMetadata {
                name: "file.txt".to_string(),
                size: 12,
                is_namespace: false,
                modified_time: 0,
                permissions: 0o644,
            },
        });

        assert!(formatted.success);
        assert!(formatted.message.contains("File information"));
        assert!(formatted.details.unwrap().contains("file.txt"));
    }

    #[test]
    fn test_format_namespace_listing() {
        let manager = ResponseManager;

        let formatted = manager.format_response(&FenrisOutput::NamespaceListing {
            entries: vec![FenrisMetadata {
                name: "dir".to_string(),
                size: 0,
                is_namespace: true,
                modified_time: 0,
                permissions: 0o755,
            }],
        });

        assert!(formatted.success);
        assert!(formatted.message.contains("Directory listing"));
        assert!(formatted.details.unwrap().contains("dir"));
    }

    #[test]
    fn test_format_namespace_changed() {
        let manager = ResponseManager;

        let formatted = manager.format_response(&FenrisOutput::NamespaceChanged {
            path: "/tmp".into(),
        });

        assert!(formatted.success);
        assert_eq!(formatted.current_dir.as_deref(), Some("/tmp"));
    }

    #[test]
    fn test_format_error_and_terminated() {
        let manager = ResponseManager;

        let formatted = manager.format_response(&FenrisOutput::Error {
            message: "bad".to_string(),
        });
        assert!(!formatted.success);
        assert_eq!(formatted.message, "bad");

        let formatted = manager.format_response(&FenrisOutput::Terminated);
        assert!(formatted.success);
        assert!(formatted.message.contains("terminated"));
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
