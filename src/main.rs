//! Headless runner: `openulanzi [config.toml]`.
use anyhow::{Context, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let path = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "config.toml".into()));
    let path = std::path::absolute(&path).context("resolving config path")?;
    openulanzi::runtime::run(path, None, Default::default(), |_| {})
}
