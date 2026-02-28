#!/usr/bin/env python3
"""
Sign manifest for Rove
Signs manifest.json with Ed25519 team private key

For local development, this creates a placeholder signature.
For production, use the offline team private key.
"""

import json
import sys
from pathlib import Path

def sign_manifest_local(manifest_path: Path):
    """Sign manifest with placeholder signature for local development"""
    print("Signing manifest (local development mode)...")
    
    with open(manifest_path, 'r') as f:
        manifest = json.load(f)
    
    # Add placeholder signature for local development
    # In production, this would use ed25519-dalek to sign with real key
    manifest["signature"] = "LOCAL_DEV_PLACEHOLDER_SIGNATURE"
    manifest["signed_at"] = "local-development"
    
    with open(manifest_path, 'w') as f:
        json.dump(manifest, f, indent=2)
    
    print(f"✓ Manifest signed (local dev mode): {manifest_path}")
    print("  Note: This is a placeholder signature for local development")
    print("  Production builds require a real Ed25519 signature")

def main():
    base_dir = Path(__file__).parent.parent.resolve()
    manifest_path = base_dir / "manifest" / "manifest.json"
    
    if not manifest_path.exists():
        print(f"Error: Manifest not found: {manifest_path}", file=sys.stderr)
        print("Run build-manifest.py first", file=sys.stderr)
        return 1
    
    try:
        sign_manifest_local(manifest_path)
        return 0
    except Exception as e:
        print(f"Error signing manifest: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1

if __name__ == "__main__":
    sys.exit(main())
