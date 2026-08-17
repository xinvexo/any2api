mod architecture;
mod package;

use anyhow::{Result, bail};

fn main() -> Result<()> {
    let mut args = std::env::args();
    let _program = args.next();
    match args.next().as_deref() {
        Some("architecture-check") => architecture::run(),
        Some("package") => package::run(args),
        Some(command) => bail!("unknown xtask command: {command}"),
        None => bail!("usage: cargo xtask <architecture-check|package>"),
    }
}
