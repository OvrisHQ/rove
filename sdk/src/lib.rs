//! Rove SDK
//!
//! Shared library providing traits, types, and utilities for Rove components.
//! This crate is used by both the engine and plugins/core-tools.

/// Core tool trait and types
pub mod core_tool;

/// Error types and handling
pub mod errors;

/// Tool input/output types
pub mod types;

/// Manifest types
pub mod manifest;

/// Helper utilities
pub mod helpers;

// Re-export commonly used types
pub use core_tool::{
    AgentHandle, AgentHandleImpl, BusHandle, BusHandleImpl, ConfigHandle, ConfigHandleImpl,
    CoreContext, CoreTool, CryptoHandle, CryptoHandleImpl, DbHandle, DbHandleImpl, NetworkHandle,
    NetworkHandleImpl,
};
pub use errors::{EngineError, RoveErrorExt};
pub use manifest::{CoreToolEntry, Manifest, PluginEntry, PluginPermissions};
pub use types::{ToolError, ToolInput, ToolOutput};
