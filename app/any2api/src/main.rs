mod bootstrap;
mod settings;
mod shutdown;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bootstrap::run().await
}
