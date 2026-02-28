//! Rove Engine Library
//!
//! This library provides the core functionality of the Rove engine.
//! It is used by both the main binary and integration tests.

/// Configuration management module
pub mod config;

/// Cryptographic operations module
pub mod crypto;

/// Secret management module
pub mod secrets;

/// File system security module
pub mod fs_guard;

/// Command execution security module
pub mod command_executor;

/// Injection detection module
pub mod injection_detector;

/// Risk assessment module
pub mod risk_assessor;

/// Database persistence module
pub mod db;

/// Rate limiting module
pub mod rate_limiter;

/// Message bus for inter-component communication
pub mod message_bus;

/// LLM provider abstraction layer
pub mod llm;

/// Agent loop core module
pub mod agent;

/// Conductor orchestration module
pub mod conductor;

/// Built-in native core tools
pub mod tools;

/// Telegram bot module
pub mod bot;

/// Runtime module for loading and managing core tools and plugins
pub mod runtime;

/// Telemetry and Observability
pub mod telemetry;

/// Daemon lifecycle management module
pub mod daemon;

/// CLI interface module
pub mod cli;

/// Command handlers module
pub mod handlers;

/// WebSocket client for external UI connection
pub mod ws_client;

/// Platform-specific utilities module
pub mod platform;
