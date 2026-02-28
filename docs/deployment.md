# Rove Deployment Documentation

## Installation

### Prerequisites
- Rust toolchain (1.75+)
- Ollama (for local LLM, recommended)
- Optional: API keys for cloud providers

### Build from Source

```bash
git clone <repo-url>
cd flowBot

# Build engine
cargo build --release -p engine

# Build WASM plugins
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1 -p fs-editor
cargo build --release --target wasm32-wasip1 -p terminal-plugin
cargo build --release --target wasm32-wasip1 -p git-plugin
cargo build --release --target wasm32-wasip1 -p screenshot-plugin

# Or use the build script
./scripts/build-all.sh
```

## Setup

### Interactive Setup Wizard

```bash
rove setup
```

The wizard prompts for:
1. **Workspace directory** - Where Rove operates (default: ~/projects)
2. **Default LLM provider** - ollama, openai, anthropic, gemini, nvidia_nim
3. **API keys** - Stored securely in OS keychain
4. **Telegram bot** - Optional bot token and user ID
5. **Risk tier** - Maximum allowed operation tier (0-2)

### Manual Configuration

Create `~/.rove/config.toml`:

```toml
[core]
workspace = "~/projects"
log_level = "info"
data_dir = "~/.rove"

[llm]
default_provider = "ollama"

[tools]
tg-controller = false
ui-server = false
api-server = false

[plugins]
fs-editor = true
terminal = true
screenshot = false
git = true

[security]
max_risk_tier = 2
confirm_tier1 = true
confirm_tier1_delay = 10
require_explicit_tier2 = true
```

## Daemon Management

```bash
# Start daemon
rove start

# Check status
rove status

# Stop daemon
rove stop

# Run system diagnostics
rove doctor
```

## Direct Task Execution

```bash
# Execute a task
rove run "List all files in the current directory"

# JSON output
rove --json run "What is 2+2?"

# Custom config
rove --config my_config.toml run "..."
```

## Task History

```bash
# Show last 10 tasks
rove history

# Show last 20 tasks
rove history --limit 20

# Replay task steps
rove replay <task-id>
```

## Plugin Management

```bash
# List plugins
rove plugins list
```

## Skill Management

```bash
# List available skills
rove skill list

# Activate a skill
rove skill on code-review

# Deactivate a skill
rove skill off code-review

# Create a new skill
rove skill add my-skill --description "My custom skill"
```

## API Key Management

API keys are stored in the OS keychain. To add/update keys:

```bash
rove setup  # Interactive wizard
```

Or use the OS keychain directly:
- **macOS**: Keychain Access app
- **Linux**: `secret-tool` CLI
- **Windows**: Credential Manager

Key names: `openai_api_key`, `anthropic_api_key`, `gemini_api_key`, `nvidia_nim_api_key`, `telegram_bot_token`

## Graceful Shutdown

On SIGTERM:
1. New tasks refused
2. In-progress tasks allowed 30 seconds to complete
3. Core tools stopped
4. Plugins closed
5. SQLite WAL flushed
6. PID file removed

## Troubleshooting

```bash
# Run diagnostics
rove doctor

# Debug logging
rove --log debug run "test"

# Check if Ollama is running
curl http://localhost:11434/api/tags
```
