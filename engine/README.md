# Rove Engine

Main binary and core logic crate for the Rove agent. Contains the ReAct agent loop, LLM router with multi-provider failover, tool registry, and security subsystems including filesystem guards, injection detection, and risk assessment.

- **Binary:** `rove`
- **Library:** `rove_engine`
- **Key modules:** `agent`, `llm`, `tools`, `security` (fs_guard, injection_detector, risk_assessor, crypto), `config`, `daemon`, `handlers`, `cli`, `ws_client`, `bot/telegram`
