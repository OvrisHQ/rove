//! Build script for embedding the team public key at compile time
//!
//! This script reads the team public key from the manifest directory and
//! embeds it into the binary. This ensures the key cannot be modified
//! without recompiling the engine.
//!
//! # Key Location
//!
//! The script looks for the public key in the following locations (in order):
//! 1. Environment variable `ROVE_TEAM_PUBLIC_KEY` (hex-encoded)
//! 2. File `manifest/team_public_key.bin` (raw bytes)
//! 3. File `manifest/team_public_key.hex` (hex-encoded)
//!
//! If no key is found, a placeholder key is generated for development builds.
//! **Production builds MUST provide a real key.**

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // 1. Get the current Git commit hash
    let commit_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", commit_hash);

    // 2. Get the current Build Timestamp (ISO 8601)
    let build_time = chrono::Utc::now().to_rfc3339();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", build_time);

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(&out_dir).join("team_public_key.bin");

    // Try to load the team public key from various sources
    let public_key_bytes = load_team_public_key();

    // Write the key to the output directory
    fs::write(&dest_path, &public_key_bytes).expect("Failed to write team public key");

    println!("cargo:rerun-if-changed=manifest/team_public_key.bin");
    println!("cargo:rerun-if-changed=manifest/team_public_key.hex");
    println!("cargo:rerun-if-env-changed=ROVE_TEAM_PUBLIC_KEY");

    // Warn if using placeholder key
    if is_placeholder_key(&public_key_bytes) {
        println!("cargo:warning=Using placeholder team public key for development");
        println!("cargo:warning=Production builds MUST provide a real key via:");
        println!("cargo:warning=  - ROVE_TEAM_PUBLIC_KEY environment variable");
        println!("cargo:warning=  - manifest/team_public_key.bin file");
        println!("cargo:warning=  - manifest/team_public_key.hex file");
    }
}

/// Load the team public key from available sources
///
/// Priority order:
/// 1. ROVE_TEAM_PUBLIC_KEY environment variable (hex)
/// 2. manifest/team_public_key.bin (raw bytes)
/// 3. manifest/team_public_key.hex (hex string)
/// 4. Generate placeholder for development
fn load_team_public_key() -> Vec<u8> {
    // Try environment variable first
    if let Ok(key_hex) = env::var("ROVE_TEAM_PUBLIC_KEY") {
        if let Ok(bytes) = hex::decode(&key_hex) {
            if bytes.len() == 32 {
                println!("cargo:warning=Loaded team public key from ROVE_TEAM_PUBLIC_KEY");
                return bytes;
            }
        }
        println!("cargo:warning=Invalid ROVE_TEAM_PUBLIC_KEY (must be 32 bytes hex)");
    }

    // build.rs runs from the crate dir (engine/), key files are at workspace root
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap_or(&manifest_dir);

    // Try binary file
    let bin_path = workspace_root.join("manifest/team_public_key.bin");
    if bin_path.exists() {
        if let Ok(bytes) = fs::read(&bin_path) {
            if bytes.len() == 32 {
                println!("cargo:warning=Loaded team public key from manifest/team_public_key.bin");
                return bytes;
            }
        }
        println!("cargo:warning=Invalid manifest/team_public_key.bin (must be 32 bytes)");
    }

    // Try hex file
    let hex_path = workspace_root.join("manifest/team_public_key.hex");
    if hex_path.exists() {
        if let Ok(hex_str) = fs::read_to_string(&hex_path) {
            let hex_str = hex_str.trim();
            if let Ok(bytes) = hex::decode(hex_str) {
                if bytes.len() == 32 {
                    println!(
                        "cargo:warning=Loaded team public key from manifest/team_public_key.hex"
                    );
                    return bytes;
                }
            }
        }
        println!("cargo:warning=Invalid manifest/team_public_key.hex (must be 32 bytes hex)");
    }

    // Generate placeholder for development
    println!("cargo:warning=No team public key found, generating placeholder");
    generate_placeholder_key()
}

/// Generate a placeholder key for development builds
///
/// This key is deterministic so that development builds are reproducible.
/// It is clearly marked as a placeholder and should never be used in production.
fn generate_placeholder_key() -> Vec<u8> {
    // Use a deterministic "key" that's obviously a placeholder (32 bytes of zeros)
    vec![0u8; 32]
}

/// Check if a key is the placeholder key
fn is_placeholder_key(key: &[u8]) -> bool {
    key.len() == 32 && key.iter().all(|&b| b == 0)
}
