<p align="center">
  <h1 align="center">Rove</h1>
  <p align="center">Local-first AI agent engine written in Rust</p>
</p>

<p align="center">
  <a href="https://github.com/OvrisHQ/rove/actions/workflows/ci.yml"><img src="https://github.com/OvrisHQ/rove/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/OvrisHQ/rove/releases/latest"><img src="https://img.shields.io/github/v/release/OvrisHQ/rove?label=latest" alt="Latest Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/OvrisHQ/rove" alt="License"></a>
</p>

---

Rove is a local-first AI coding agent that runs on your machine. It uses a ReAct agent loop, routes across multiple LLM providers with automatic failover, executes tools through sandboxed WASM plugins, and enforces a 4-gate security model.

## Install

**Quick install (Linux / macOS):**

```bash
curl -fsSL https://raw.githubusercontent.com/OvrisHQ/rove/main/scripts/install.sh | sh
```

**Build from source:**

```bash
git clone https://github.com/OvrisHQ/rove.git
cd rove
cargo build --release -p engine
# Binary at target/release/rove
```

**Self-update (if already installed):**

```bash
rove update           # download and install latest
rove update --check   # check without downloading
```

## Quick Start

```bash
# First-time setup
rove setup

# Run a task
rove run "list files in current directory"

# Start the daemon
rove start

# Check system health
rove doctor

# Show task history
rove history
```

## Features

- **ReAct Agent Loop** — structured think / tool / observe cycle (max 20 iterations)
- **Multi-Provider LLM Router** — Ollama (local) → OpenAI → Anthropic → Gemini → NVIDIA NIM with automatic failover and provider ranking
- **WASM Plugin System** — sandboxed plugins via Extism (filesystem, terminal, git, screenshot)
- **4-Gate Security Model**
  - FileSystemGuard — path traversal prevention with double canonicalization
  - CommandExecutor — allowlist-based command execution
  - InjectionDetector — prompt injection detection
  - RiskAssessor — tiered risk levels (T0 read / T1 write / T2 destructive) with progressive confirmation
- **SQLite WAL Persistence** — write-ahead logging with connection pooling
- **Multiple Entry Points** — CLI, Telegram bot, WebSocket client
- **Self-Update** — update in-place from GitHub Releases

## Architecture

```
┌──────────────────────────────────────────────┐
│                   CLI / Bot / WS             │
├──────────────────────────────────────────────┤
│              AgentCore (ReAct Loop)          │
│         think → tool → observe → repeat      │
├──────────────┬───────────────────────────────┤
│  LLM Router  │        ToolRegistry           │
│  ┌─────────┐ │  ┌──────────┐ ┌─────────────┐ │
│  │ Ollama  │ │  │ read_file│ │run_command  │ │
│  │ OpenAI  │ │  │write_file│ │ list_dir    │ │
│  │Anthropic│ │  │file_exist│ │  capture    │ │
│  │ Gemini  │ │  └──────────┘ └─────────────┘ │
│  │  NVIDIA │ │                               │
│  └─────────┘ │                               │
├──────────────┴───────────────────────────────┤
│              Security Layer                  │
│  FSGuard · CommandExec · Injection · Risk    │
├──────────────────────────────────────────────┤
│          SQLite WAL + WASM Plugins           │
└──────────────────────────────────────────────┘
```

## Workspace Structure

```
rove/
├── engine/      — main binary + core logic
├── sdk/         — shared types, traits, errors
├── core-tools/  — telegram, ui-server, api-server
├── plugins/     — WASM plugins (fs-editor, terminal, git, screenshot)
├── manifest/    — build/sign scripts + public key
├── scripts/     — install + build scripts
└── docs/        — architecture, security, plugin guides
```

## Configuration

Configuration lives at `~/.rove/config.toml`. Run `rove setup` for interactive configuration or edit directly.

Key sections: `[core]` (workspace, logging), `[llm]` (providers, models), `[plugins]` (enable/disable), `[security]` (risk tiers, confirmation).

API keys are stored in your OS keychain, never in config files.

## Commands

| Command             | Description                     |
| ------------------- | ------------------------------- |
| `rove setup`        | Interactive setup wizard        |
| `rove run <task>`   | Execute a task immediately      |
| `rove start`        | Start daemon in background      |
| `rove stop`         | Stop running daemon             |
| `rove status`       | Show daemon and provider status |
| `rove history`      | Show task history               |
| `rove replay <id>`  | Replay task steps               |
| `rove doctor`       | System diagnostics              |
| `rove update`       | Self-update to latest release   |
| `rove plugins list` | List installed plugins          |
| `rove skill list`   | List agent skills               |

## Development

```bash
# Build (excluding WASM plugins)
cargo build --workspace --exclude fs-read --exclude fs-editor --exclude terminal --exclude git --exclude screenshot

# Test
cargo test --workspace --exclude fs-read --exclude fs-editor --exclude terminal --exclude git --exclude screenshot

# Lint
cargo clippy --workspace --exclude fs-read --exclude fs-editor --exclude terminal --exclude git --exclude screenshot -- -D warnings

# Format
cargo fmt --all -- --check
```

## License

[MIT](LICENSE)
