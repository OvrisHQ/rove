# Rove Plugin Development Guide

## Overview

rove plugins are WebAssembly modules built with the Extism PDK. They run in a sandboxed environment and communicate with the host engine through well-defined host functions.

## Creating a New Plugin

### 1. Project Setup

```bash
cargo init --lib my-plugin
```

Add to `Cargo.toml`:
```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
extism-pdk = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

Add to workspace `Cargo.toml`:
```toml
[workspace]
members = ["plugins/my-plugin"]
```

### 2. Define Input/Output Types

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct MyInput {
    param: String,
}

#[derive(Serialize)]
struct MyOutput {
    result: String,
    success: bool,
}
```

### 3. Implement Plugin Functions

```rust
use extism_pdk::*;

#[plugin_fn]
pub fn my_function(input: String) -> FnResult<String> {
    let input: MyInput = serde_json::from_str(&input)?;
    
    // Use host functions for file/command access
    let result = unsafe { host::read_file(&input.param)? };
    
    let output = MyOutput { result, success: true };
    Ok(serde_json::to_string(&output)?)
}
```

### 4. Declare Host Functions

```rust
mod host {
    use extism_pdk::*;
    
    #[host_fn]
    extern "ExtismHost" {
        pub fn read_file(path: &str) -> String;
        pub fn write_file(path: &str, content: &str);
        pub fn list_directory(path: &str) -> String;
    }
}
```

### 5. Build for WASM

```bash
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p my-plugin
```

The output `.wasm` file will be in `target/wasm32-wasip1/release/`.

## Available Host Functions

| Function | Description | Security |
|----------|-------------|----------|
| `read_file(path)` | Read file contents | FileSystemGuard validated |
| `write_file(path, content)` | Write to file | FileSystemGuard validated |
| `list_directory(path)` | List directory entries | FileSystemGuard validated |
| `exec_git(args)` | Execute git command | CommandExecutor validated |

## Security Constraints

- All file paths validated through FileSystemGuard
- Plugins cannot access paths outside the workspace
- Plugins cannot access sensitive files (.ssh, .env, etc.)
- Plugins cannot publish to the message bus
- Plugin crashes are isolated and don't affect the engine

## Testing

Add `rlib` to your crate types for testing:
```toml
[lib]
crate-type = ["cdylib", "rlib"]
```

Write unit tests for serialization/deserialization:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_input_deserialization() {
        let json = r#"{"param": "test"}"#;
        let input: MyInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.param, "test");
    }
}
```

## Plugin Manifest

Plugins must be declared in the manifest with their BLAKE3 hash:
```json
{
  "plugins": [
    {
      "name": "my-plugin",
      "hash": "<blake3-hash>",
      "version": "0.1.0"
    }
  ]
}
```

Use `scripts/build-manifest.py` to generate the manifest automatically.
