//! Conductor System Memory Management
//!
//! Handles Session Memory (short-term active context), Episodic Memory (long-term retrieval),
//! and Project Memory (workspace context).

use crate::agent::WorkingMemory;
use crate::conductor::types::MemoryBudget;
use crate::llm::Message;

/// SessionMemory manages the short-term active conversation context.
/// It wraps `WorkingMemory` and enforces the `session_tokens` limit from `MemoryBudget`.
#[derive(Debug, Clone)]
pub struct SessionMemory {
    working_memory: WorkingMemory,
    _max_tokens: usize,
}

impl SessionMemory {
    /// Create a new session memory managed by the given budget
    pub fn new(budget: &MemoryBudget) -> Self {
        Self {
            working_memory: WorkingMemory::with_limit(budget.session_tokens),
            _max_tokens: budget.session_tokens,
        }
    }

    /// Add a generic message to the session
    pub fn add(&mut self, message: Message) {
        self.working_memory.add_message(message);
    }

    /// Add a user message to the session
    pub fn add_user(&mut self, content: &str) {
        self.add(Message::user(content));
    }

    /// Add an assistant message to the session
    pub fn add_assistant(&mut self, content: &str) {
        self.add(Message::assistant(content));
    }

    /// Retrieve all currently active session messages
    pub fn messages(&self) -> &[Message] {
        self.working_memory.messages()
    }

    /// Get current token count
    pub fn token_count(&self) -> usize {
        self.working_memory.token_count()
    }

    /// Clear the session entirely
    pub fn clear(&mut self) {
        self.working_memory.clear();
    }
}
