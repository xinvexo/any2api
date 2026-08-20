#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> anyhow::Result<()> {
    any2api_memory_reclaimer::disable_transparent_huge_pages()
        .map_err(|error| anyhow::anyhow!("failed to disable transparent huge pages: {error}"))?;
    any2api::run()
}
