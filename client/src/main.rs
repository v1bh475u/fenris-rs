mod app;
mod client;
mod connection_manager;
mod request_manager;
mod response_manager;
mod ui;

use anyhow::Result;
use client::Client;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::fs::File::create("fenris-client.log")?)
        .init();

    let mut terminal = ui::terminal::init()?;

    let mut client = Client::new();
    let result = client.run(&mut terminal).await;

    ui::terminal::restore()?;

    result
}
