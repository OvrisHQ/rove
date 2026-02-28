# Rove Architecture Documentation

## Overview

Rove is a local-first AI agent engine written in Rust with a dual-runtime architecture:
- **Native core tools** loaded as shared libraries (.so/.dylib/.dll)
- **WASM plugins** loaded via Extism for sandboxed execution

## System Architecture

```
┌──────────────────────────────────────────────────┐
│                    CLI / API                      │
│              (clap derive, REST)                  │
├──────────────────────────────────────────────────┤
│                  Agent Core                       │
│        (think-act-observe loop, 20 iter max)     │
├──────────────┬───────────────────────────────────┤
│  LLM Router  │         Tool Registry             │
│  (failover,  │  (FilesystemTool, TerminalTool,   │
│   ranking)   │   VisionTool)                     │
├──────────────┴───────────────────────────────────┤
│              Security Layer                       │
│  ┌────────────┬──────────────┬─────────────┐     │
│  │ FileSystem │  Command     │  Injection  │     │
│  │ Guard      │  Executor    │  Detector   │     │
│  └────────────┴──────────────┴─────────────┘     │
│  ┌────────────┬──────────────┬─────────────┐     │
│  │ Risk       │  Rate        │  Secret     │     │
│  │ Assessor   │  Limiter     │  Manager    │     │
│  └────────────┴──────────────┴─────────────┘     │
├──────────────────────────────────────────────────┤
│              Runtime Layer                        │
│  ┌─────────────────┬────────────────────────┐    │
│  │ Native Runtime  │    WASM Runtime         │    │
│  │ (4-gate verify) │  (2-gate verify,        │    │
│  │ dlopen/.dylib   │   Extism, crash isolate)│    │
│  └─────────────────┴────────────────────────┘    │
├──────────────────────────────────────────────────┤
│              Persistence Layer                    │
│  ┌──────────┬────────────┬──────────────────┐    │
│  │ SQLite   │  Message   │   Crypto         │    │
│  │ (WAL)    │  Bus       │   Module         │    │
│  └──────────┴────────────┴──────────────────┘    │
└──────────────────────────────────────────────────┘
```

## Component Interactions

### Agent Loop
1. User submits task via CLI (`rove run "..."`), Telegram, or API
2. RiskAssessor classifies operation tier (0/1/2)
3. RateLimiter checks operation limits
4. AgentCore enters think-act-observe loop (max 20 iterations)
5. LLMRouter selects provider (local-preferred for sensitive, cloud for complex)
6. LLM response parsed: tool call → dispatch → result fed back, or final answer → return

### LLM Provider Selection
- **Local (Ollama)**: Preferred for sensitive data, free, no network
- **Cloud (OpenAI, Anthropic, Gemini, NIM)**: Used for complex tasks, API key from keychain
- **Failover**: Providers tried in ranked order, 300s timeout per call

### Tool Dispatch
The `ToolRegistry` holds optional references to each core tool:
- `read_file`, `write_file`, `list_dir`, `file_exists` → FilesystemTool
- `run_command` → TerminalTool
- `capture_screen` → VisionTool

All paths validated through FileSystemGuard before I/O.

## Data Flow

```
User Input → AgentCore → LLMRouter → Provider → Response
                ↕                        ↕
           ToolRegistry            Working Memory
                ↕                        ↕
         FileSystemGuard          Task Repository
                ↕                        ↕
           Workspace               SQLite (WAL)
```

## Configuration

Config stored in `~/.rove/config.toml` with sections:
- `[core]` - workspace, log_level, data_dir
- `[llm]` - provider settings, sensitivity/complexity thresholds
- `[tools]` - core tool enablement (tg-controller, ui-server, api-server)
- `[plugins]` - plugin enablement (fs-editor, terminal, screenshot, git)
- `[security]` - risk tier limits, confirmation settings
- `[steering]` - skill system configuration
- `[ws_client]` - WebSocket client for external UI

## Database Schema

SQLite with WAL mode:
- `tasks` - Task records with status, input, provider, duration
- `task_steps` - Individual steps (user message, tool call, tool result, assistant message)
- `plugins` - Plugin metadata and status
- `rate_limits` - Rate limit tracking by source and tier
