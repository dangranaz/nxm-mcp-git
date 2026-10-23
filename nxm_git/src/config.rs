use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;
use toml;

/// Configuration loaded from `~/.nxm/git-mcp.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct GitMcpConfig {
    /// Allowed root directories for repository operations.
    pub allowed_roots: Vec<PathBuf>,
}

impl GitMcpConfig {
    /// Load configuration from the default location.
    ///
    /// If the file is missing, returns an empty configuration (no allowed roots).
    /// Returns an error if the file exists but is malformed.
    pub fn load() -> Result<Self> {
        let mut path = std::env::home_dir().context("unable to determine home directory")?;
        path.push(".nxm");
        path.push("git-mcp.toml");
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let config: Self = toml::from_str(&content)
                    .with_context(|| format!("parsing TOML from {}", path.display()))?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                allowed_roots: Vec::new(),
            }),
            Err(e) => Err(e).with_context(|| format!("reading config from {}", path.display())),
        }
    }
}
