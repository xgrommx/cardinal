use anyhow::bail;
use anyhow::Result;
use std::env;
use std::process::Command;
fn main() -> Result<()> {
    let mut envs = vec![];
    envs.push(("DATABASE_URL", "sqlite:target/rows.db"));
    let args: Vec<_> = env::args_os().collect();
    let status = Command::new("cargo").args(&args[1..]).envs(envs).status()?;
    if !status.success() {
        bail!("xtask failed.");
    }
    Ok(())
}
