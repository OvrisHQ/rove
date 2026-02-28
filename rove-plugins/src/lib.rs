//! Rove Official Plugin Registry
//!
//! This crate handles downloading, verifying, installing, and managing
//! official Rove plugins. Plugins are WASM modules signed with the
//! official plugin key.

pub mod registry;
pub mod installer;
pub mod verifier;

/// Plugin trust tier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustTier {
    /// Signed by official plugin key — full permissions per manifest
    Official,
    /// Signed by community key — requires one-time consent
    Community,
    /// Hash-only verification — sandboxed, no network
    Unverified,
}

/// Metadata for an installed plugin
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Semantic version
    pub version: String,
    /// SHA-256 hash of the WASM binary
    pub hash: String,
    /// Trust tier
    pub trust: TrustTier,
    /// Whether the plugin is currently enabled
    pub enabled: bool,
}
