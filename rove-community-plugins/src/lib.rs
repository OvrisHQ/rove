//! Rove Community Plugin Registry
//!
//! This crate handles community-contributed plugins that are signed
//! with the community key. Community plugins require one-time user
//! consent before installation and have per-manifest permissions.

use rove_plugins::TrustTier;

/// Community plugin metadata with consent tracking
#[derive(Debug, Clone)]
pub struct CommunityPlugin {
    /// Plugin identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Semantic version
    pub version: String,
    /// Author or organization
    pub author: String,
    /// Whether user has consented to this plugin
    pub consented: bool,
    /// Trust tier (always Community for this crate)
    pub trust: TrustTier,
}

impl CommunityPlugin {
    /// Create a new community plugin entry
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>, author: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            author: author.into(),
            consented: false,
            trust: TrustTier::Community,
        }
    }
}
