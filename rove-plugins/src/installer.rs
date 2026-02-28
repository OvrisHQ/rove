//! Plugin installer — download, verify, install WASM plugins

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;

use crate::registry::{self, RegistryEntry};
use crate::verifier;

/// Download and install a plugin from the registry
pub async fn install_plugin(entry: &RegistryEntry) -> Result<PathBuf> {
    let plugin_dir = registry::plugin_dir()?;
    tokio::fs::create_dir_all(&plugin_dir).await?;

    let dest = plugin_dir.join(format!("{}.wasm", entry.id));

    info!("Downloading plugin: {} v{}", entry.name, entry.version);

    // Download into memory first (never to disk before verification)
    let client = reqwest::Client::builder()
        .user_agent("rove-plugins/0.1.0")
        .build()?;

    let bytes = client
        .get(&entry.download_url)
        .send()
        .await?
        .error_for_status()
        .context("Failed to download plugin")?
        .bytes()
        .await?;

    // Verify hash before writing to disk
    verifier::verify_hash(&bytes, &entry.hash)?;
    info!("  Hash verified: {}", &entry.hash[..16]);

    // Write verified binary to disk
    tokio::fs::write(&dest, &bytes).await?;

    info!("  Installed to: {}", dest.display());
    Ok(dest)
}

/// Remove an installed plugin
pub async fn remove_plugin(plugin_id: &str) -> Result<()> {
    let plugin_dir = registry::plugin_dir()?;
    let path = plugin_dir.join(format!("{}.wasm", plugin_id));

    if path.exists() {
        tokio::fs::remove_file(&path).await?;
        info!("Removed plugin: {}", plugin_id);
    }

    Ok(())
}

/// List installed plugin files
pub async fn list_installed() -> Result<Vec<String>> {
    let plugin_dir = registry::plugin_dir()?;
    if !plugin_dir.exists() {
        return Ok(vec![]);
    }

    let mut plugins = Vec::new();
    let mut entries = tokio::fs::read_dir(&plugin_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                plugins.push(stem.to_string());
            }
        }
    }

    Ok(plugins)
}
