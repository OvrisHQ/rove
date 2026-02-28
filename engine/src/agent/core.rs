//! Agent Core
//!
//! This module implements the core agent loop that orchestrates task execution.
//! The agent processes tasks through an iterative think-act-observe cycle:
//!
//! 1. Assess risk tier before execution
//! 2. Execute up to 20 iterations per task
//! 3. Call LLM provider (with 30s timeout)
//! 4. Handle response (tool call or final answer)
//! 5. If tool call: execute tool, add result to memory, continue loop
//! 6. If final answer: return result
//!
//! # Limits
//!
//! - Max 20 iterations per task
//! - 30-second timeout per LLM call
//! - 5MB result size limit
//!
//! Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7

use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::db::tasks::{StepType, TaskRepository, TaskStatus};
use crate::llm::router::LLMRouter;
use crate::llm::{LLMResponse, Message};
use crate::rate_limiter::RateLimiter;
use crate::risk_assessor::{Operation, OperationSource, RiskAssessor};
use crate::tools::ToolRegistry;
use sdk::errors::EngineError;

use super::WorkingMemory;

/// Maximum number of iterations per task
const MAX_ITERATIONS: usize = 20;

/// Timeout for each LLM call in seconds
const LLM_TIMEOUT_SECS: u64 = 300;

/// Maximum result size in bytes (5MB)
const MAX_RESULT_SIZE: usize = 5 * 1024 * 1024;

/// Task input for agent processing
#[derive(Debug, Clone)]
pub struct Task {
    /// Task input text
    pub input: String,

    /// Source of the task (local or remote)
    pub source: OperationSource,
}

impl Task {
    /// Create a new task
    pub fn new(input: impl Into<String>, source: OperationSource) -> Self {
        Self {
            input: input.into(),
            source,
        }
    }
}

/// Task result after processing
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// Task ID
    pub task_id: String,

    /// Final answer from the agent
    pub answer: String,

    /// Provider used for the task
    pub provider_used: String,

    /// Duration in milliseconds
    pub duration_ms: i64,

    /// Number of iterations executed
    pub iterations: usize,
}

impl TaskResult {
    /// Create a success result
    pub fn success(
        task_id: String,
        answer: String,
        provider_used: String,
        duration_ms: i64,
        iterations: usize,
    ) -> Self {
        Self {
            task_id,
            answer,
            provider_used,
            duration_ms,
            iterations,
        }
    }
}

/// Agent Core that orchestrates the agent loop
pub struct AgentCore {
    /// LLM router for provider selection
    router: Arc<LLMRouter>,

    /// Working memory for conversation history
    memory: WorkingMemory,

    /// Risk assessor for operation classification
    risk_assessor: RiskAssessor,

    /// Rate limiter for operation throttling
    rate_limiter: Arc<RateLimiter>,

    /// Task repository for persistence
    task_repo: Arc<TaskRepository>,

    /// Tool registry for dispatching tool calls
    tools: Arc<ToolRegistry>,
}

impl AgentCore {
    /// Create a new agent core
    pub fn new(
        router: Arc<LLMRouter>,
        risk_assessor: RiskAssessor,
        rate_limiter: Arc<RateLimiter>,
        task_repo: Arc<TaskRepository>,
        tools: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            router,
            memory: WorkingMemory::new(),
            risk_assessor,
            rate_limiter,
            task_repo,
            tools,
        }
    }

    /// Process a task through the agent loop
    ///
    /// This is the main entry point for task execution. It:
    /// 1. Assesses risk tier
    /// 2. Checks rate limits
    /// 3. Executes the agent loop (up to MAX_ITERATIONS)
    /// 4. Persists task and steps to database
    ///
    /// Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7
    pub async fn process_task(&mut self, task: Task) -> Result<TaskResult> {
        let task_id = Uuid::new_v4().to_string();
        let _start_time = Instant::now();

        info!("Starting task {}: {}", task_id, task.input);

        // Create task in database
        self.task_repo
            .create_task(&task_id, &task.input)
            .await
            .context("Failed to create task in database")?;

        // Update status to running
        self.task_repo
            .update_task_status(&task_id, TaskStatus::Running)
            .await
            .context("Failed to update task status")?;

        // Execute the task and handle result
        let result = self.execute_task_loop(&task_id, task).await;

        match result {
            Ok(task_result) => {
                // Complete task in database
                self.task_repo
                    .complete_task(
                        &task_id,
                        &task_result.provider_used,
                        task_result.duration_ms,
                    )
                    .await
                    .context("Failed to complete task in database")?;

                info!(
                    "Task {} completed in {}ms after {} iterations",
                    task_id, task_result.duration_ms, task_result.iterations
                );

                Ok(task_result)
            }
            Err(e) => {
                // Mark task as failed
                self.task_repo
                    .fail_task(&task_id)
                    .await
                    .context("Failed to mark task as failed")?;

                error!("Task {} failed: {}", task_id, e);
                Err(e)
            }
        }
    }

    /// Execute the main task loop
    ///
    /// Requirements: 2.1, 2.2, 2.3, 2.4, 2.6, 2.7
    async fn execute_task_loop(&mut self, task_id: &str, task: Task) -> Result<TaskResult> {
        let start_time = Instant::now();

        // Step 1: Assess risk tier (Requirement 2.1)
        let operation = Operation::new("execute_task", vec![], task.source.clone());
        let risk_tier = self
            .risk_assessor
            .assess(&operation)
            .context("Failed to assess risk tier")?;

        debug!("Task {} assessed as {:?}", task_id, risk_tier);

        // Check rate limits based on risk tier
        self.rate_limiter
            .check_limit(task_id, risk_tier)
            .await
            .context("Rate limit exceeded")?;

        // Record the operation
        self.rate_limiter
            .record_operation(task_id, risk_tier)
            .await
            .context("Failed to record operation")?;

        // Initialize working memory with system prompt + user message
        self.memory.clear();
        let system_prompt = self.tools.system_prompt();
        self.memory.add_message(Message::system(&system_prompt));
        let user_message = Message::user(&task.input);
        self.memory.add_message(user_message.clone());

        // Persist initial user message
        self.task_repo
            .add_task_step(task_id, 0, StepType::UserMessage, &task.input)
            .await
            .context("Failed to persist user message")?;

        let mut iteration = 0;
        let mut _last_provider_used = String::new();

        // Step 2: Execute up to MAX_ITERATIONS (Requirement 2.2)
        while iteration < MAX_ITERATIONS {
            iteration += 1;
            debug!(
                "Task {} iteration {}/{}",
                task_id, iteration, MAX_ITERATIONS
            );

            // Step 3: Call LLM with 30s timeout (Requirement 2.3)
            let llm_result = timeout(
                Duration::from_secs(LLM_TIMEOUT_SECS),
                self.router.call(self.memory.messages()),
            )
            .await;

            let response = match llm_result {
                Ok(Ok(response)) => response,
                Ok(Err(e)) => {
                    error!("LLM call failed: {}", e);
                    return Err(e.into());
                }
                Err(_) => {
                    error!("LLM call timed out after {}s", LLM_TIMEOUT_SECS);
                    return Err(EngineError::LLMTimeout.into());
                }
            };

            // Track which provider was used (from router's last call)
            // For now, we'll use a placeholder - in a real implementation,
            // the router would need to expose which provider was used
            _last_provider_used = "ollama".to_string(); // TODO: Get from router

            // Step 4: Handle response (Requirement 2.6, 2.7)
            match response {
                LLMResponse::ToolCall(tool_call) => {
                    debug!("Tool call: {} ({})", tool_call.name, tool_call.id);

                    // Add assistant message to memory before tool result
                    // (Ollama requires user→assistant→tool ordering)
                    let assistant_msg = Message::assistant(
                        serde_json::json!({
                            "function": &tool_call.name,
                            "arguments": serde_json::from_str::<serde_json::Value>(&tool_call.arguments).unwrap_or_default()
                        }).to_string()
                    );
                    self.memory.add_message(assistant_msg);

                    // Persist tool call
                    let tool_call_content = serde_json::to_string(&tool_call)
                        .context("Failed to serialize tool call")?;
                    self.task_repo
                        .add_task_step(
                            task_id,
                            (iteration * 2 - 1) as i64,
                            StepType::ToolCall,
                            &tool_call_content,
                        )
                        .await
                        .context("Failed to persist tool call")?;

                    // Execute tool via registry
                    let tool_result = self
                        .tools
                        .dispatch(&tool_call.name, &tool_call.arguments)
                        .await;

                    // Step 4: Enforce 5MB result size limit (Requirement 2.4)
                    if tool_result.len() > MAX_RESULT_SIZE {
                        warn!(
                            "Tool result exceeds size limit: {} bytes > {} bytes",
                            tool_result.len(),
                            MAX_RESULT_SIZE
                        );
                        return Err(EngineError::ResultSizeExceeded {
                            size: tool_result.len(),
                            limit: MAX_RESULT_SIZE,
                        }
                        .into());
                    }

                    // Add tool result to memory
                    let result_message = Message::tool_result(&tool_result, &tool_call.id);
                    self.memory.add_message(result_message);

                    // Persist tool result
                    self.task_repo
                        .add_task_step(
                            task_id,
                            (iteration * 2) as i64,
                            StepType::ToolResult,
                            &tool_result,
                        )
                        .await
                        .context("Failed to persist tool result")?;

                    // Continue loop
                }
                LLMResponse::FinalAnswer(answer) => {
                    debug!("Final answer received");

                    // Step 4: Enforce 5MB result size limit (Requirement 2.4)
                    if answer.content.len() > MAX_RESULT_SIZE {
                        warn!(
                            "Final answer exceeds size limit: {} bytes > {} bytes",
                            answer.content.len(),
                            MAX_RESULT_SIZE
                        );
                        return Err(EngineError::ResultSizeExceeded {
                            size: answer.content.len(),
                            limit: MAX_RESULT_SIZE,
                        }
                        .into());
                    }

                    // Persist final answer
                    self.task_repo
                        .add_task_step(
                            task_id,
                            (iteration * 2 - 1) as i64,
                            StepType::AssistantMessage,
                            &answer.content,
                        )
                        .await
                        .context("Failed to persist final answer")?;

                    // Calculate duration
                    let duration_ms = start_time.elapsed().as_millis() as i64;

                    // Return result (Requirement 2.5 - persistence happens in process_task)
                    return Ok(TaskResult::success(
                        task_id.to_string(),
                        answer.content,
                        "ollama".to_string(), // TODO: Get actual provider from router
                        duration_ms,
                        iteration,
                    ));
                }
            }
        }

        // Max iterations exceeded (Requirement 2.2)
        error!(
            "Task {} exceeded max iterations ({})",
            task_id, MAX_ITERATIONS
        );
        Err(EngineError::MaxIterationsExceeded.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LLMConfig;
    use crate::db::Database;
    use crate::llm::router::LLMRouter;
    use tempfile::TempDir;

    async fn setup_test_agent() -> (TempDir, AgentCore) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();
        let pool = db.pool().clone();

        let llm_config = Arc::new(LLMConfig {
            default_provider: "ollama".to_string(),
            sensitivity_threshold: 0.7,
            complexity_threshold: 0.8,
            ollama: Default::default(),
            openai: Default::default(),
            anthropic: Default::default(),
            gemini: Default::default(),
            nvidia_nim: Default::default(),
        });

        let router = Arc::new(LLMRouter::new(vec![], llm_config));
        let risk_assessor = RiskAssessor::new();
        let rate_limiter = Arc::new(RateLimiter::new(pool.clone()));
        let task_repo = Arc::new(TaskRepository::new(pool));

        let agent = AgentCore::new(
            router,
            risk_assessor,
            rate_limiter,
            task_repo,
            Arc::new(ToolRegistry::empty()),
        );

        (temp_dir, agent)
    }

    #[test]
    fn test_task_creation() {
        let task = Task::new("Test task", OperationSource::Local);
        assert_eq!(task.input, "Test task");
        assert_eq!(task.source, OperationSource::Local);
    }

    #[test]
    fn test_task_result_creation() {
        let result = TaskResult::success(
            "task-123".to_string(),
            "Answer".to_string(),
            "ollama".to_string(),
            1000,
            5,
        );

        assert_eq!(result.task_id, "task-123");
        assert_eq!(result.answer, "Answer");
        assert_eq!(result.provider_used, "ollama");
        assert_eq!(result.duration_ms, 1000);
        assert_eq!(result.iterations, 5);
    }

    #[tokio::test]
    async fn test_agent_core_creation() {
        let (_temp_dir, agent) = setup_test_agent().await;

        // Agent should be created successfully
        assert_eq!(agent.memory.messages().len(), 0);
    }

    // Note: Full integration tests would require mock LLM providers
    // and tool implementations, which are beyond the scope of this task.
    // These tests verify the basic structure and setup.
}
