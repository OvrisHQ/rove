-- Initial schema for Rove database
-- Requirements: 12.1, 12.3, 12.4, 12.5, 12.6

-- Note: WAL mode, synchronous mode, and foreign keys are configured
-- via connection options in the Rust code, not via PRAGMA statements

-- Tasks table: stores task execution history
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    input TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'running', 'completed', 'failed')),
    provider_used TEXT,
    duration_ms INTEGER,
    created_at INTEGER NOT NULL,
    completed_at INTEGER
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);

-- Task steps table: stores individual steps in task execution
CREATE TABLE IF NOT EXISTS task_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL,
    step_order INTEGER NOT NULL,
    step_type TEXT NOT NULL CHECK(step_type IN ('user_message', 'assistant_message', 'tool_call', 'tool_result')),
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

-- Index for efficient task step retrieval
CREATE INDEX IF NOT EXISTS idx_task_steps_task_id ON task_steps(task_id, step_order);

-- Plugins table: stores plugin metadata and state
CREATE TABLE IF NOT EXISTS plugins (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    version TEXT NOT NULL,
    wasm_path TEXT NOT NULL,
    wasm_hash TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Index for enabled plugins lookup
CREATE INDEX IF NOT EXISTS idx_plugins_enabled ON plugins(enabled);

-- Secrets cache table: temporary encrypted secret storage
CREATE TABLE IF NOT EXISTS secrets_cache (
    key TEXT PRIMARY KEY,
    encrypted_value BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER
);

-- Index for expired secrets cleanup
CREATE INDEX IF NOT EXISTS idx_secrets_cache_expires_at ON secrets_cache(expires_at);

-- Rate limits table: tracks operations for rate limiting
CREATE TABLE IF NOT EXISTS rate_limits (
    source TEXT NOT NULL,
    tier INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    PRIMARY KEY (source, tier, timestamp)
);

-- Index for efficient rate limit queries
CREATE INDEX IF NOT EXISTS idx_rate_limits_timestamp ON rate_limits(timestamp);
CREATE INDEX IF NOT EXISTS idx_rate_limits_source_tier ON rate_limits(source, tier, timestamp DESC);
