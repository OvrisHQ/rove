# Rove Core Tool Development Guide

## Overview

Core tools are native shared libraries (.so/.dylib/.dll) loaded directly into the Rove engine process. They have full access to system resources and communicate through the `CoreTool` trait defined in the SDK.

## CoreTool Trait

```rust
pub trait CoreTool {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn start(&mut self, ctx: CoreContext) -> Result<(), EngineError>;
    fn stop(&mut self) -> Result<(), EngineError>;
    fn handle(&self, input: ToolInput) -> Result<ToolOutput, EngineError>;
}
```

## Creating a New Core Tool

### 1. Create a new crate

```bash
cargo init --lib core-tools/my-tool
```

Add to workspace and configure as a cdylib:
```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
sdk = { path = "../../sdk" }
tracing = "0.1"
```

### 2. Implement the CoreTool trait

```rust
use sdk::{CoreContext, CoreTool, EngineError, ToolInput, ToolOutput};

pub struct MyTool {
    ctx: Option<CoreContext>,
}

impl MyTool {
    pub fn new() -> Self {
        Self { ctx: None }
    }
}

impl CoreTool for MyTool {
    fn name(&self) -> &str { "my-tool" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }

    fn start(&mut self, ctx: CoreContext) -> Result<(), EngineError> {
        self.ctx = Some(ctx);
        tracing::info!("My tool started");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), EngineError> {
        tracing::info!("My tool stopped");
        Ok(())
    }

    fn handle(&self, input: ToolInput) -> Result<ToolOutput, EngineError> {
        // Handle tool input and return output
        Ok(ToolOutput::empty())
    }
}
```

### 3. Export the FFI constructor

```rust
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn create_tool() -> *mut dyn CoreTool {
    Box::into_raw(Box::new(MyTool::new()))
}
```

### 4. Build

```bash
cargo build --release -p my-tool
```

Output: `target/release/libmy_tool.dylib` (macOS) / `.so` (Linux) / `.dll` (Windows)

## CoreContext API

The `CoreContext` provides handles to engine subsystems:

| Handle | Purpose |
|--------|---------|
| `AgentHandle` | Submit tasks to the agent |
| `DbHandle` | Database operations |
| `ConfigHandle` | Read configuration |
| `CryptoHandle` | Cryptographic operations |
| `NetworkHandle` | Network access |
| `BusHandle` | Message bus pub/sub |

## Lifecycle

1. Engine loads library via `dlopen`
2. Engine calls `create_tool()` FFI function
3. Engine calls `start(ctx)` with CoreContext
4. Engine calls `handle(input)` for each tool invocation
5. On shutdown, engine calls `stop()`

## Security

Core tools undergo 4-gate verification before loading:
1. Tool declared in signed manifest
2. BLAKE3 hash matches manifest
3. Team signature on manifest verified
4. Individual tool signature verified

## Existing Core Tools

| Tool | Crate | Description |
|------|-------|-------------|
| Telegram Bot | `core-tools/telegram` | Long-polling Telegram interface |
| UI Server | `core-tools/ui-server` | WebSocket server for UI |
| API Server | `core-tools/api-server` | REST API for task submission |
