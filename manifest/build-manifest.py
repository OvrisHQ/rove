#!/usr/bin/env python3
"""
Build manifest for Rove
Computes SHA256 hashes for all core tools and plugins
Generates manifest.json with all entries
"""

import hashlib
import json
import os
import sys
from pathlib import Path
from typing import Dict, List

def compute_sha256(file_path: Path) -> str:
    """Compute SHA256 hash of a file"""
    sha256 = hashlib.sha256()
    with open(file_path, 'rb') as f:
        while chunk := f.read(8192):
            sha256.update(chunk)
    return sha256.hexdigest()

def find_plugins(base_dir: Path) -> List[Dict]:
    """Find all WASM plugins in target directory"""
    plugins = []
    wasm_dir = base_dir / "target" / "wasm32-wasip1" / "release"
    
    if not wasm_dir.exists():
        print(f"Warning: WASM directory not found: {wasm_dir}")
        return plugins
    
    for wasm_file in wasm_dir.glob("*.wasm"):
        # Skip deps directory files
        if wasm_file.parent.name == "deps":
            continue
            
        file_hash = compute_sha256(wasm_file)
        plugin_name = wasm_file.stem.replace('_', '-')
        
        plugins.append({
            "id": plugin_name,
            "version": "0.1.0",
            "path": f"plugins/{plugin_name}.wasm",
            "hash": file_hash,
            "size": wasm_file.stat().st_size
        })
        
        print(f"Found plugin: {plugin_name} (hash: {file_hash[:16]}...)")
    
    return plugins

def find_core_tools(base_dir: Path) -> List[Dict]:
    """Find all core tools (native libraries)"""
    tools = []
    
    # For now, return empty list since we're focusing on plugins
    # In production, this would scan for .so/.dylib/.dll files
    
    return tools

def build_manifest(base_dir: Path, output_path: Path):
    """Build the complete manifest"""
    print("Building Rove manifest...")
    print(f"Base directory: {base_dir}")
    
    plugins = find_plugins(base_dir)
    core_tools = find_core_tools(base_dir)
    
    manifest = {
        "version": "1.0.0",
        "generated_at": "local-build",
        "plugins": plugins,
        "core_tools": core_tools,
        "signature": None  # Will be added by sign-manifest.py
    }
    
    # Write manifest
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, 'w') as f:
        json.dump(manifest, f, indent=2)
    
    print(f"\nManifest written to: {output_path}")
    print(f"  Plugins: {len(plugins)}")
    print(f"  Core tools: {len(core_tools)}")
    
    return manifest

def main():
    # Get base directory (Rove root)
    base_dir = Path(__file__).parent.parent.resolve()
    output_path = base_dir / "manifest" / "manifest.json"
    
    try:
        manifest = build_manifest(base_dir, output_path)
        print("\n✓ Manifest built successfully")
        return 0
    except Exception as e:
        print(f"\n✗ Error building manifest: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1

if __name__ == "__main__":
    sys.exit(main())
