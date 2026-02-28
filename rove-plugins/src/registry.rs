//! Plugin registry — fetches manifests from CDN/GitHub

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single plugin entry in the registry manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub hash: String,
    pub signature: String,
    pub download_url: String,
    #[serde(default)]
    pub min_engine_version: Option<String>,
}

/// The full registry manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryManifest {
    pub version: String,
    pub plugins: Vec<RegistryEntry>,
    #[serde(default)]
    pub signature: String,
}

/// Fetch the plugin registry manifest
pub async fn fetch_manifest() -> Result<RegistryManifest> {
    let client = reqwest::Client::builder()
        .user_agent("rove-plugins/0.1.0")
        .build()?;

    // Try GitHub raw first
    let url = "https://raw.githubusercontent.com/OvrisHQ/rove/main/manifest/plugins.json";

    let response = client
        .get(url)
        .send()
        .await?
        .error_for_status()
        .context("Failed to fetch plugin registry")?;

    let manifest: RegistryManifest = response.json().await?;
    Ok(manifest)
}

/// Get the local plugin install directory (~/.rove/plugins/)
pub fn plugin_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".rove").join("plugins"))
}

/// Get the local manifest cache path
pub fn cache_path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".rove").join("cache").join("manifests").join("plugins.json"))
}

/// Cache manifest locally
pub async fn cache_manifest(manifest: &RegistryManifest) -> Result<()> {
    let path = cache_path()?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let json = serde_json::to_string_pretty(manifest)?;
    tokio::fs::write(&path, json).await?;
    Ok(())
}

/// Load cached manifest if available
pub async fn load_cached_manifest() -> Result<Option<RegistryManifest>> {
    let path = cache_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let contents = tokio::fs::read_to_string(&path).await?;
    let manifest: RegistryManifest = serde_json::from_str(&contents)?;
    Ok(Some(manifest))
}

/// Find a plugin entry by ID in the manifest
pub fn find_plugin<'a>(manifest: &'a RegistryManifest, plugin_id: &str) -> Option<&'a RegistryEntry> {
    manifest.plugins.iter().find(|p| p.id == plugin_id)
}
