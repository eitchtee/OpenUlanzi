//! Headless runner: `openulanzi [config.toml]`.
use anyhow::{Context, Result};
use openulanzi::plugin::Plugins;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn main() -> Result<()> {
    let path = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "config.toml".into()));
    let path = std::path::absolute(&path).context("resolving config path")?;
    let plugins = Plugins::load(path.parent().unwrap_or(Path::new(".")));
    plugins.start();
    openulanzi::runtime::run(path, None, Arc::new(plugins), Default::default(), |_| {})
}
