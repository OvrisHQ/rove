/// Task persistence operations
///
/// This module provides functions for persisting tasks and task steps to the database.
/// All queries use parameterized queries for SQL injection prevention.
///
/// Requirements: 12.2, 12.4, 12.5, 12.7, 12.10
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::time::{SystemTime, UNIX_EPOCH};

/// Task status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
        }
    }
}

/// Task step type enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    UserMessage,
    AssistantMessage,
    ToolCall,
    ToolResult,
}

impl StepType {
    pub fn as_str(&self) -> &str {
        match self {
            StepType::UserMessage => "user_message",
            StepType::AssistantMessage => "assistant_message",
            StepType::ToolCall => "tool_call",
            StepType::ToolResult => "tool_result",
        }
    }
}

/// Task record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub input: String,
    pub status: TaskStatus,
    pub provider_used: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

/// Task step record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    pub id: Option<i64>,
    pub task_id: String,
    pub step_order: i64,
    pub step_type: StepType,
    pub content: String,
    pub created_at: i64,
}

/// Task repository for database operations
pub struct TaskRepository {
    pool: SqlitePool,
}

impl TaskRepository {
    /// Create a new task repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new task
    ///
    /// Requirements: 12.4, 12.10
    pub async fn create_task(&self, id: &str, input: &str) -> Result<Task> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let status = TaskStatus::Pending.as_str();

        // Use parameterized query to prevent SQL injection
        sqlx::query("INSERT INTO tasks (id, input, status, created_at) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(input)
            .bind(status)
            .bind(now)
            .execute(&self.pool)
            .await
            .context("Failed to create task")?;

        Ok(Task {
            id: id.to_string(),
            input: input.to_string(),
            status: TaskStatus::Pending,
            provider_used: None,
            duration_ms: None,
            created_at: now,
            completed_at: None,
        })
    }

    /// Update task status
    ///
    /// Requirements: 12.4, 12.10
    pub async fn update_task_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        let status_str = status.as_str();

        sqlx::query("UPDATE tasks SET status = ? WHERE id = ?")
            .bind(status_str)
            .bind(task_id)
            .execute(&self.pool)
            .await
            .context("Failed to update task status")?;

        Ok(())
    }

    /// Complete a task with results
    ///
    /// Requirements: 12.4, 12.10
    pub async fn complete_task(
        &self,
        task_id: &str,
        provider_used: &str,
        duration_ms: i64,
    ) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let status = TaskStatus::Completed.as_str();

        sqlx::query(
            "UPDATE tasks SET status = ?, provider_used = ?, duration_ms = ?, completed_at = ? WHERE id = ?"
        )
        .bind(status)
        .bind(provider_used)
        .bind(duration_ms)
        .bind(now)
        .bind(task_id)
        .execute(&self.pool)
        .await
        .context("Failed to complete task")?;

        Ok(())
    }

    /// Mark a task as failed
    ///
    /// Requirements: 12.4, 12.10
    pub async fn fail_task(&self, task_id: &str) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let status = TaskStatus::Failed.as_str();

        sqlx::query("UPDATE tasks SET status = ?, completed_at = ? WHERE id = ?")
            .bind(status)
            .bind(now)
            .bind(task_id)
            .execute(&self.pool)
            .await
            .context("Failed to mark task as failed")?;

        Ok(())
    }

    /// Get a task by ID
    ///
    /// Requirements: 12.4, 12.10
    pub async fn get_task(&self, task_id: &str) -> Result<Option<Task>> {
        let row = sqlx::query(
            "SELECT id, input, status, provider_used, duration_ms, created_at, completed_at FROM tasks WHERE id = ?"
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch task")?;

        Ok(row.map(|r| Task {
            id: r.get("id"),
            input: r.get("input"),
            status: match r.get::<String, _>("status").as_str() {
                "pending" => TaskStatus::Pending,
                "running" => TaskStatus::Running,
                "completed" => TaskStatus::Completed,
                "failed" => TaskStatus::Failed,
                _ => TaskStatus::Failed,
            },
            provider_used: r.get("provider_used"),
            duration_ms: r.get("duration_ms"),
            created_at: r.get("created_at"),
            completed_at: r.get("completed_at"),
        }))
    }

    /// Get recent tasks (last N tasks)
    ///
    /// Requirements: 12.4, 12.10
    pub async fn get_recent_tasks(&self, limit: i64) -> Result<Vec<Task>> {
        let rows = sqlx::query(
            "SELECT id, input, status, provider_used, duration_ms, created_at, completed_at FROM tasks ORDER BY created_at DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch recent tasks")?;

        Ok(rows
            .into_iter()
            .map(|r| Task {
                id: r.get("id"),
                input: r.get("input"),
                status: match r.get::<String, _>("status").as_str() {
                    "pending" => TaskStatus::Pending,
                    "running" => TaskStatus::Running,
                    "completed" => TaskStatus::Completed,
                    "failed" => TaskStatus::Failed,
                    _ => TaskStatus::Failed,
                },
                provider_used: r.get("provider_used"),
                duration_ms: r.get("duration_ms"),
                created_at: r.get("created_at"),
                completed_at: r.get("completed_at"),
            })
            .collect())
    }

    /// Add a step to a task
    ///
    /// Requirements: 12.5, 12.10
    pub async fn add_task_step(
        &self,
        task_id: &str,
        step_order: i64,
        step_type: StepType,
        content: &str,
    ) -> Result<TaskStep> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let step_type_str = step_type.as_str();

        let result = sqlx::query(
            "INSERT INTO task_steps (task_id, step_order, step_type, content, created_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(task_id)
        .bind(step_order)
        .bind(step_type_str)
        .bind(content)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("Failed to add task step")?;

        Ok(TaskStep {
            id: Some(result.last_insert_rowid()),
            task_id: task_id.to_string(),
            step_order,
            step_type,
            content: content.to_string(),
            created_at: now,
        })
    }

    /// Get all steps for a task
    ///
    /// Requirements: 12.5, 12.10
    pub async fn get_task_steps(&self, task_id: &str) -> Result<Vec<TaskStep>> {
        let rows = sqlx::query(
            "SELECT id, task_id, step_order, step_type, content, created_at FROM task_steps WHERE task_id = ? ORDER BY step_order ASC"
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch task steps")?;

        Ok(rows
            .into_iter()
            .map(|r| TaskStep {
                id: Some(r.get("id")),
                task_id: r.get("task_id"),
                step_order: r.get("step_order"),
                step_type: match r.get::<String, _>("step_type").as_str() {
                    "user_message" => StepType::UserMessage,
                    "assistant_message" => StepType::AssistantMessage,
                    "tool_call" => StepType::ToolCall,
                    "tool_result" => StepType::ToolResult,
                    _ => StepType::UserMessage,
                },
                content: r.get("content"),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    /// Delete old tasks (cleanup)
    ///
    /// Requirements: 12.4, 12.10
    pub async fn delete_old_tasks(&self, older_than_days: i64) -> Result<u64> {
        let cutoff = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64
            - (older_than_days * 24 * 60 * 60);

        let result = sqlx::query("DELETE FROM tasks WHERE created_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .context("Failed to delete old tasks")?;

        Ok(result.rows_affected())
    }
}
