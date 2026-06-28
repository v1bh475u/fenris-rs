use anyhow::Result;
use clap::ValueEnum;
use common::{FenrisError, ServerIdentityPublicKey};
use serde::Serialize;
use std::io::{self, BufRead, Write};

use crate::connection_manager::{ConnectionManager, ServerInfo};
use crate::response_manager::FormattedResponse;

#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub address: String,
    pub port: u16,
    pub commands: Vec<String>,
    pub output: BatchOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BatchOutputFormat {
    Human,
    Jsonl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchSummary {
    pub total: usize,
    pub failed: usize,
    pub aborted: bool,
}

impl BatchSummary {
    pub fn is_success(&self) -> bool {
        self.failed == 0 && !self.aborted
    }
}

#[derive(Debug, Clone, Serialize)]
struct BatchCommandRecord<'a> {
    command: &'a str,
    success: bool,
    message: &'a str,
    details: Option<&'a str>,
    current_dir: Option<&'a str>,
}

#[derive(Debug, Clone)]
struct BatchCommandResult {
    command: String,
    response: FormattedResponse,
}

pub async fn run_batch(
    config: BatchConfig,
    server_identity: ServerIdentityPublicKey,
) -> Result<BatchSummary> {
    let mut manager = ConnectionManager::with_server_identity(
        crate::request_manager::RequestManager,
        crate::response_manager::ResponseManager,
        server_identity,
    );
    manager.set_server_info(ServerInfo::new(config.address, config.port))?;
    manager.connect().await?;

    let mut stdout = io::stdout().lock();
    let summary = run_commands(&mut manager, &config.commands, config.output, &mut stdout).await;

    manager.disconnect().await;
    summary
}

async fn run_commands<W: Write>(
    manager: &mut ConnectionManager,
    commands: &[String],
    output: BatchOutputFormat,
    writer: &mut W,
) -> Result<BatchSummary> {
    let mut failed = 0;
    let mut aborted = false;

    for command in commands {
        match manager.send_command(command).await {
            Ok(response) => {
                if !response.success {
                    failed += 1;
                }
                write_result(
                    writer,
                    output,
                    &BatchCommandResult {
                        command: command.clone(),
                        response,
                    },
                )?;
            }
            Err(error) => {
                failed += 1;
                let should_abort = should_abort(&error);
                write_result(
                    writer,
                    output,
                    &BatchCommandResult {
                        command: command.clone(),
                        response: FormattedResponse {
                            success: false,
                            message: error.to_string(),
                            details: None,
                            current_dir: None,
                        },
                    },
                )?;

                if should_abort {
                    aborted = true;
                    break;
                }
            }
        }
    }

    Ok(BatchSummary {
        total: commands.len(),
        failed,
        aborted,
    })
}

fn should_abort(error: &FenrisError) -> bool {
    matches!(
        error,
        FenrisError::ConnectionClosed | FenrisError::NetworkError(_)
    )
}

pub fn read_commands_from_source(source: &str) -> Result<Vec<String>> {
    if source == "-" {
        let stdin = io::stdin();
        return read_commands(stdin.lock());
    }

    let file = std::fs::File::open(source)?;
    read_commands(io::BufReader::new(file))
}

fn read_commands<R: BufRead>(reader: R) -> Result<Vec<String>> {
    let mut commands = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let command = line.trim();
        if !command.is_empty() {
            commands.push(command.to_string());
        }
    }

    Ok(commands)
}

fn write_result<W: Write>(
    writer: &mut W,
    output: BatchOutputFormat,
    result: &BatchCommandResult,
) -> Result<()> {
    match output {
        BatchOutputFormat::Human => write_human_result(writer, result),
        BatchOutputFormat::Jsonl => write_json_result(writer, result),
    }
}

fn write_human_result<W: Write>(writer: &mut W, result: &BatchCommandResult) -> Result<()> {
    writeln!(writer, "> {}", result.command)?;
    writeln!(writer, "{}", result.response.message)?;

    if let Some(details) = &result.response.details {
        writeln!(writer, "{}", details)?;
    }

    Ok(())
}

fn write_json_result<W: Write>(writer: &mut W, result: &BatchCommandResult) -> Result<()> {
    let record = BatchCommandRecord {
        command: &result.command,
        success: result.response.success,
        message: &result.response.message,
        details: result.response.details.as_deref(),
        current_dir: result.response.current_dir.as_deref(),
    };

    serde_json::to_writer(&mut *writer, &record)?;
    writeln!(writer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_commands_skips_empty_lines() {
        let input = io::Cursor::new("ping\n\n  ls /  \n\t\nhelp\n");

        let commands = read_commands(input).unwrap();

        assert_eq!(commands, vec!["ping", "ls /", "help"]);
    }

    #[test]
    fn read_commands_from_source_reads_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("commands.txt");
        std::fs::write(&path, "ping\n\nls /\n").unwrap();

        let commands = read_commands_from_source(path.to_str().unwrap()).unwrap();

        assert_eq!(commands, vec!["ping", "ls /"]);
    }

    #[test]
    fn human_output_includes_command_message_and_details() {
        let result = BatchCommandResult {
            command: "read file.txt".to_string(),
            response: FormattedResponse {
                success: true,
                message: "File content (5 bytes):".to_string(),
                details: Some("hello".to_string()),
                current_dir: None,
            },
        };
        let mut output = Vec::new();

        write_result(&mut output, BatchOutputFormat::Human, &result).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "> read file.txt\nFile content (5 bytes):\nhello\n"
        );
    }

    #[test]
    fn json_output_writes_one_record() {
        let result = BatchCommandResult {
            command: "cd /tmp".to_string(),
            response: FormattedResponse {
                success: true,
                message: "Changed directory to /tmp".to_string(),
                details: None,
                current_dir: Some("/tmp".to_string()),
            },
        };
        let mut output = Vec::new();

        write_result(&mut output, BatchOutputFormat::Jsonl, &result).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "{\"command\":\"cd /tmp\",\"success\":true,\"message\":\"Changed directory to /tmp\",\"details\":null,\"current_dir\":\"/tmp\"}\n"
        );
    }

    #[test]
    fn summary_success_requires_no_failures_or_abort() {
        assert!(
            BatchSummary {
                total: 2,
                failed: 0,
                aborted: false,
            }
            .is_success()
        );
        assert!(
            !BatchSummary {
                total: 2,
                failed: 1,
                aborted: false,
            }
            .is_success()
        );
        assert!(
            !BatchSummary {
                total: 2,
                failed: 0,
                aborted: true,
            }
            .is_success()
        );
    }

    #[test]
    fn aborts_only_for_unusable_connections() {
        assert!(should_abort(&FenrisError::ConnectionClosed));
        assert!(should_abort(&FenrisError::NetworkError(io::Error::other(
            "closed"
        ))));
        assert!(!should_abort(&FenrisError::InvalidProtocolMessage));
        assert!(!should_abort(&FenrisError::MissingField(
            "path".to_string()
        )));
    }
}
