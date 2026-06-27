mod app;
mod client;
mod connection_manager;
mod request_manager;
mod response_manager;
mod ui;

use anyhow::Result;
use clap::Parser;
use client::Client;
use common::ServerIdentityPublicKey;

#[derive(Parser, Debug)]
#[command(name = "fenris-client")]
#[command(about = "Fast Encrypted Network Robust Information Storage - Client")]
struct Args {
    #[arg(long)]
    server_identity: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let server_identity = parse_server_identity(&args.server_identity)?;

    tracing_subscriber::fmt()
        .with_writer(std::fs::File::create("fenris-client.log")?)
        .with_ansi(false)
        .init();

    let mut terminal = ui::terminal::init()?;

    let mut client = Client::with_server_identity(server_identity);
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
