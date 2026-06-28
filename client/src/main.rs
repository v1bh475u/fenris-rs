mod app;
mod batch;
mod client;
mod connection_manager;
mod request_manager;
mod response_manager;
mod ui;

use anyhow::Result;
use batch::{BatchConfig, BatchOutputFormat};
use clap::{Args as ClapArgs, Parser, Subcommand};
use client::TuiClient;
use common::ServerIdentityPublicKey;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "fenris-client")]
#[command(about = "Fast Encrypted Network Robust Information Storage - Client")]
struct Args {
    #[arg(long)]
    server_identity: String,

    #[command(subcommand)]
    mode: Option<ClientMode>,
}

#[derive(Debug, Subcommand)]
enum ClientMode {
    Tui,
    Batch(BatchArgs),
}

#[derive(Debug, ClapArgs)]
struct BatchArgs {
    #[arg(long, default_value = "127.0.0.1")]
    address: String,

    #[arg(long, default_value_t = 5555)]
    port: u16,

    #[arg(long)]
    commands_file: String,

    #[arg(long, value_enum, default_value = "human")]
    output: BatchOutputFormat,
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    let args = Args::parse();
    let server_identity = parse_server_identity(&args.server_identity)?;

    tracing_subscriber::fmt()
        .with_writer(std::fs::File::create("fenris-client.log")?)
        .with_ansi(false)
        .init();

    match args.mode.unwrap_or(ClientMode::Tui) {
        ClientMode::Tui => run_tui(server_identity).await?,
        ClientMode::Batch(args) => {
            let commands = batch::read_commands_from_source(&args.commands_file)?;
            let summary = batch::run_batch(
                BatchConfig {
                    address: args.address,
                    port: args.port,
                    commands,
                    output: args.output,
                },
                server_identity,
            )
            .await?;

            if !summary.is_success() {
                return Ok(ExitCode::FAILURE);
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

async fn run_tui(server_identity: ServerIdentityPublicKey) -> Result<()> {
    let mut terminal = ui::terminal::init()?;

    let mut client = TuiClient::with_server_identity(server_identity);
    let result = client.run(&mut terminal).await;

    ui::terminal::restore()?;

    result
}

fn parse_server_identity(input: &str) -> Result<ServerIdentityPublicKey> {
    ServerIdentityPublicKey::from_hex_or_file(input).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_require_server_identity() {
        assert!(Args::try_parse_from(["fenris-client"]).is_err());
    }

    #[test]
    fn args_default_to_tui_mode() {
        let identity = common::ServerIdentityKey::generate().public_key();

        let args = Args::try_parse_from(["fenris-client", "--server-identity", &identity.to_hex()])
            .unwrap();

        assert!(args.mode.is_none());
    }

    #[test]
    fn args_parse_tui_subcommand() {
        let identity = common::ServerIdentityKey::generate().public_key();

        let args = Args::try_parse_from([
            "fenris-client",
            "--server-identity",
            &identity.to_hex(),
            "tui",
        ])
        .unwrap();

        assert!(matches!(args.mode, Some(ClientMode::Tui)));
    }

    #[test]
    fn args_parse_batch_defaults() {
        let identity = common::ServerIdentityKey::generate().public_key();

        let args = Args::try_parse_from([
            "fenris-client",
            "--server-identity",
            &identity.to_hex(),
            "batch",
            "--commands-file",
            "commands.txt",
        ])
        .unwrap();

        let Some(ClientMode::Batch(batch)) = args.mode else {
            panic!("expected batch mode");
        };
        assert_eq!(batch.address, "127.0.0.1");
        assert_eq!(batch.port, 5555);
        assert_eq!(batch.commands_file, "commands.txt");
        assert_eq!(batch.output, BatchOutputFormat::Human);
    }

    #[test]
    fn args_parse_batch_jsonl_output() {
        let identity = common::ServerIdentityKey::generate().public_key();

        let args = Args::try_parse_from([
            "fenris-client",
            "--server-identity",
            &identity.to_hex(),
            "batch",
            "--commands-file",
            "-",
            "--address",
            "localhost",
            "--port",
            "6000",
            "--output",
            "jsonl",
        ])
        .unwrap();

        let Some(ClientMode::Batch(batch)) = args.mode else {
            panic!("expected batch mode");
        };
        assert_eq!(batch.address, "localhost");
        assert_eq!(batch.port, 6000);
        assert_eq!(batch.commands_file, "-");
        assert_eq!(batch.output, BatchOutputFormat::Jsonl);
    }

    #[test]
    fn parse_server_identity_accepts_hex_input() {
        let identity = common::ServerIdentityKey::generate().public_key();

        assert_eq!(parse_server_identity(&identity.to_hex()).unwrap(), identity);
    }

    #[test]
    fn parse_server_identity_accepts_path_input() {
        let temp_dir = tempfile::tempdir().unwrap();
        let identity_path = temp_dir.path().join("server.pub");
        let identity = common::ServerIdentityKey::generate().public_key();
        std::fs::write(&identity_path, identity.to_hex()).unwrap();

        assert_eq!(
            parse_server_identity(identity_path.to_str().unwrap()).unwrap(),
            identity
        );
    }
}
