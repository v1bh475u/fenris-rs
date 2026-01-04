use anyhow::Result;
use clap::Parser;
use common::{DefaultFileOperations, FileOperations};
use server::{Server, ServerConfig};
use std::{path::PathBuf, sync::Arc};

#[derive(Parser, Debug)]
#[command(name = "fenris-server")]
#[command(about = "Fast Encrypted Network Robust Information Storage", long_about = None)]
struct Args {
    #[arg(long, short = 'H', default_value = "127.0.0.1")]
    host: String,

    #[arg(long, short, default_value = "5555")]
    port: u16,

    #[arg(long, short = 'd', default_value = "/tmp")]
    base_dir: PathBuf,

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

    let bind_addr = format!("{}:{}", args.host, args.port);

    let (server, handle) = Server::bind(&bind_addr, file_ops, ServerConfig::default()).await?;

    println!("Fenris Server");
    println!("Listening on {}", server.local_addr()?);
    println!("Base directory: {:?}", args.base_dir.canonicalize()?);
    println!("Press Ctrl+C to stop");

    let shutdown = handle.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        shutdown.shutdown();
    });

    server.run().await?;
    Ok(())
}
