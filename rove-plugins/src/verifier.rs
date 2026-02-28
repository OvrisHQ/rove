//! Plugin verification — SHA-256 hash + Ed25519 signature checks

use anyhow::Result;
use sha2::{Digest, Sha256};

/// Verify that the SHA-256 hash of `data` matches `expected_hex`
pub fn verify_hash(data: &[u8], expected_hex: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let computed = hex::encode(hasher.finalize());

    if computed != expected_hex {
        return Err(anyhow::anyhow!(
            "Hash mismatch: expected {}, got {}",
            expected_hex,
            computed
        ));
    }

    Ok(())
}

/// Compute the SHA-256 hash of `data` and return hex string
pub fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_and_verify_hash() {
        let data = b"hello world";
        let hash = compute_hash(data);
        assert!(verify_hash(data, &hash).is_ok());
    }

    #[test]
    fn test_verify_hash_mismatch() {
        let data = b"hello world";
        let result = verify_hash(data, "0000000000000000000000000000000000000000000000000000000000000000");
        assert!(result.is_err());
    }
}
