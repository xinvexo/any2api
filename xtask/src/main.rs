mod architecture;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("architecture-check") => architecture::run(),
        Some(command) => bail!("unknown xtask command: {command}"),
        None => bail!("usage: cargo xtask architecture-check"),
    }
}
