use anyhow::Result;
use clap::Parser;
use common::{DefaultFileOperations, FileOperations};
use server::{Server, ServerConfig};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "fenris-server")]
#[command(about = "Fast Encrypted Network Robust Information Storage - Server")]
struct Args {
    #[arg(long, short, default_value = "5555")]
    port: u16,

    #[arg(long, short = 'd', default_value = "/tmp")]
    base_dir: PathBuf,

    #[arg(long, default_value = "1024")]
    max_connections: usize,

    #[arg(long, default_value = "10")]
    handshake_timeout: u64,

    #[arg(long, default_value = "300")]
    idle_timeout: u64,

    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(args.log_level.clone())
        .init();

    let file_ops: Arc<dyn FileOperations> =
        Arc::new(DefaultFileOperations::new(args.base_dir.clone()));

    let config = ServerConfig::builder()
        .max_connections(args.max_connections)
        .handshake_timeout(Duration::from_secs(args.handshake_timeout))
        .idle_timeout(if args.idle_timeout > 0 {
            Some(Duration::from_secs(args.idle_timeout))
        } else {
            None
        })
        .build();

    let bind_addr = format!("{}:{}", "localhost", args.port);
    let (server, handle) = Server::bind(&bind_addr, file_ops, config).await?;

    println!("Fenris Server v{}", env!("CARGO_PKG_VERSION"));
    println!("Listening on {}", server.local_addr()?);
    println!("Base directory: {:?}", args.base_dir.canonicalize()?);
    println!("Max connections: {}", args.max_connections);
    println!("Press Ctrl+C to stop");

    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        println!("\nReceived Ctrl+C, shutting down...");
        shutdown_handle.shutdown();
    });

    server.run().await?;
    Ok(())
}
