---
name: architecture
description: System components and boundaries
---

# Rove Architecture

Rove is composed of the following key subsystems:

- **Conductor System**: Orchestrates plans and retrieves episodic/project memory.
- **Steering System**: Loads hot-reloadable agent instructions from `.md` files.
- **Brains**: Manages local models via `ollama` or cloud APIs.
- **Plugins**: WASM sandboxed capabilities using Extism.
- **Core Tools**: Native Rust tools requiring high privilege (fs, terminal).

## Boundary Rules

- Avoid native command execution when a WASM plugin exists.
- Memory budgets must be strictly adhered to by the ContextAssembler.
