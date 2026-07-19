#[tokio::main]
async fn main() -> anyhow::Result<()> {
    any2api::run().await
}
