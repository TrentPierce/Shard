#[tokio::main]
async fn main() -> anyhow::Result<()> {
    shard_daemon::run(std::env::args().collect()).await
}
