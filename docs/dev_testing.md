# Rove Development Testing Guide

How to build, run, and test Rove locally during development.

> All `cargo` commands run from workspace root: `~/projects/flowBot`
> macOS: prefix commands with `TMPDIR=/tmp` if the sandbox blocks the linker.

## Quick Start

```bash
# Check everything compiles
cargo check --workspace

# Run all unit tests (engine + sdk)
TMPDIR=/tmp cargo test -p engine --lib
TMPDIR=/tmp cargo test -p sdk --lib

# Release build (4.1MB binary)
cargo build --release -p engine
```

## Running Tests

### Unit tests (209 engine + 30 SDK)

```bash
# All engine unit tests
TMPDIR=/tmp cargo test -p engine --lib

# All SDK unit tests
TMPDIR=/tmp cargo test -p sdk --lib

# Run a specific module's tests
TMPDIR=/tmp cargo test -p engine --lib -- agent::steering
TMPDIR=/tmp cargo test -p engine --lib -- crypto
TMPDIR=/tmp cargo test -p engine --lib -- llm::router
TMPDIR=/tmp cargo test -p engine --lib -- secrets
TMPDIR=/tmp cargo test -p engine --lib -- conductor

# Run a single test by name
TMPDIR=/tmp cargo test -p engine --lib -- test_activate_deactivate

# Show test output (even for passing tests)
TMPDIR=/tmp cargo test -p engine --lib -- --nocapture

# List all tests without running them
TMPDIR=/tmp cargo test -p engine --lib -- --list
```

### Integration tests

```bash
# All integration tests
TMPDIR=/tmp cargo test -p engine --tests

# Specific integration test file
TMPDIR=/tmp cargo test -p engine --test crypto_test
TMPDIR=/tmp cargo test -p engine --test db_integration_test
TMPDIR=/tmp cargo test -p engine --test security_tests
TMPDIR=/tmp cargo test -p engine --test property_tests
TMPDIR=/tmp cargo test -p engine --test agent_core_integration_test
TMPDIR=/tmp cargo test -p engine --test command_executor_integration_test
TMPDIR=/tmp cargo test -p engine --test risk_assessor_integration_test
TMPDIR=/tmp cargo test -p engine --test router_integration_test
TMPDIR=/tmp cargo test -p engine --test secrets_integration_test
TMPDIR=/tmp cargo test -p engine --test injection_detector_integration_test
TMPDIR=/tmp cargo test -p engine --test daemon_graceful_shutdown_test
TMPDIR=/tmp cargo test -p engine --test wasm_runtime_integration_test
TMPDIR=/tmp cargo test -p engine --test wasm_crash_handling_test
TMPDIR=/tmp cargo test -p engine --test native_runtime_integration_test
TMPDIR=/tmp cargo test -p engine --test platform_integration_test
TMPDIR=/tmp cargo test -p engine --test ollama_integration_test
```

### Everything at once

```bash
TMPDIR=/tmp cargo test --workspace
```

### SDK property tests

```bash
TMPDIR=/tmp cargo test -p sdk --tests
```

## Test Modules Reference

| Module | Tests | What it covers |
|---|---|---|
| agent::core | 3 | AgentCore creation, task/result types |
| agent::steering | 12 | TOML/MD skill loading, activation, conflicts, auto-activation, routing |
| agent::working_memory | 13 | Message history, trimming, token estimation, context limits |
| cli | 9 | CLI parsing, global flags, all subcommands |
| command_executor | 8 | Allowlist, shell rejection, metacharacters, pipe detection |
| conductor::context | 2 | Context assembly, budgeting, semantic routing |
| conductor::evaluator | 7 | Success/failure eval, loop detection, hallucination detection |
| conductor::executor | 2 | Executor creation, step types |
| conductor::planner | 4 | Plan parsing, default plans, markdown-wrapped JSON |
| config | 5 | Serialization, defaults, path expansion |
| crypto | 3 | File hashing, signature parsing, hash parsing |
| daemon | 7 | PID files, stale handling, status, shutdown cleanup |
| db | 4 | SQLite creation, migrations, WAL mode, foreign keys |
| fs_guard | 6 | Deny list, path traversal, workspace boundary, canonicalization |
| injection_detector | 15 | All injection phrases, case sensitivity, sanitization |
| llm (common) | 4 | Message/response types, serialization |
| llm::ollama | 5 | Provider properties, message conversion, tool call parsing |
| llm::router | 14 | Task analysis, provider ranking, sensitivity, complexity, cost |
| message_bus | 4 | Pub/sub, multiple subscribers, event filtering |
| platform | 18 | Line endings, path separators, library names, cross-platform |
| rate_limiter | 7 | Tier limits, circuit breaker, cleanup, separate sources |
| risk_assessor | 20 | Tier classification, dangerous flags, remote escalation |
| runtime::native | 4 | Runtime creation, manifest checks, tool tracking |
| runtime::wasm | 7 | Runtime creation, crash tracking, restart limits, events |
| secrets | 16 | Keychain ops, scrubbing patterns (OpenAI/Google/GitHub/Telegram/Bearer) |
| tools::filesystem | 7 | Read/write, path traversal blocking, directory listing |

## Running the Engine

```bash
# Start daemon (foreground with logs)
RUST_LOG=info cargo run -p engine --bin rove -- start

# With a test config
cargo run -p engine --bin rove -- --config test_config.toml start

# Check status & provider availability
cargo run -p engine --bin rove -- status

# Stop daemon
cargo run -p engine --bin rove -- stop

# Run diagnostics
cargo run -p engine --bin rove -- doctor
```

## Executing Tasks

```bash
# Run a task immediately
cargo run -p engine --bin rove -- run "List all files in the current directory"

# View task history
cargo run -p engine --bin rove -- history --limit 5

# Replay a specific task's steps
cargo run -p engine --bin rove -- replay <TASK_ID>

# JSON output
cargo run -p engine --bin rove -- --json run "Read Cargo.toml"
```

## Steering / Skills CLI

```bash
# List all loaded skills
cargo run -p engine --bin rove -- skill list
cargo run -p engine --bin rove -- skill list --dir ./my-skills

# Show active skills + status
cargo run -p engine --bin rove -- skill status

# Activate/deactivate a skill
cargo run -p engine --bin rove -- skill on my-skill
cargo run -p engine --bin rove -- skill off my-skill

# Create a new skill (generates TOML template)
cargo run -p engine --bin rove -- skill add my-skill --description "Does cool things"

# Edit a skill in $EDITOR
cargo run -p engine --bin rove -- skill edit my-skill
```

## Building WASM Plugins

```bash
# Build all plugins to WASM
cargo build --target wasm32-wasip1 -p fs-read
cargo build --target wasm32-wasip1 -p fs-editor
cargo build --target wasm32-wasip1 -p terminal
cargo build --target wasm32-wasip1 -p git
cargo build --target wasm32-wasip1 -p screenshot

# Release builds (133-164KB each)
cargo build --release --target wasm32-wasip1 -p fs-read
cargo build --release --target wasm32-wasip1 -p fs-editor
cargo build --release --target wasm32-wasip1 -p terminal
cargo build --release --target wasm32-wasip1 -p git
cargo build --release --target wasm32-wasip1 -p screenshot
```

> Requires `rustup target add wasm32-wasip1` if not installed.

## Building Core Tools

```bash
cargo build -p telegram
cargo build -p ui-server
cargo build -p api-server
```

## Risk Tiers

| Tier | Example | Behavior |
|---|---|---|
| 0 (Read) | `run "Read Cargo.toml"` | Auto-execute |
| 1 (Write) | `run "Create test.txt"` | 10s countdown |
| 2 (Destructive) | `run "Delete test.txt"` | Explicit Y required |

## Command Execution Safety

The `command_executor` rejects shell patterns:

```bash
# Rejected: shell invocations
cargo run -p engine --bin rove -- run "Run bash -c 'echo Hello'"

# Safe: direct execve-style
cargo run -p engine --bin rove -- run "Run the 'ls' command with '-la' arguments"
```

## Test Config

Use `test_config.toml` in the workspace root for local testing. It disables network-dependent tools:

```bash
cargo run -p engine --bin rove -- --config test_config.toml status
```

Key settings: Ollama as default provider, all network tools disabled, Tier 2 requires explicit confirmation.

## Clippy

```bash
# Check engine lib (should be zero warnings)
cargo clippy -p engine --lib -- -D warnings

# Check everything
cargo clippy --workspace -- -W clippy::all
```

## All CLI Commands

```
rove setup              Interactive setup wizard
rove start              Start the daemon
rove stop               Stop the daemon
rove status             Show daemon status & providers
rove run <task>         Execute a task immediately
rove history [--limit]  Show task history
rove replay <id>        Replay task steps
rove plugins list       List installed plugins
rove config show        Show current config
rove config get <key>   Get a config value
rove config set <k> <v> Set a config value
rove doctor             Run diagnostics
rove bot start          Start Telegram bot
rove skill list         List all skills
rove skill status       Show active skills
rove skill on <name>    Activate a skill
rove skill off <name>   Deactivate a skill
rove skill add <name>   Create a new skill
rove skill edit <name>  Edit a skill in $EDITOR
```
