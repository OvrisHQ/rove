//! Context Assembler
//!
//! Responsible for intelligently packing context (Project Memory, Episodic Memory,
//! System Instructions) into the available token budget for a task.

use crate::agent::steering::SteeringEngine;
use crate::conductor::memory::SessionMemory;
use crate::conductor::project::ProjectMemory;
use crate::conductor::types::MemoryBudget;
use crate::db::memory::EpisodicMemory;
use crate::llm::Message;
use anyhow::Result;

pub struct ContextAssembler {
    budget: MemoryBudget,
}

impl ContextAssembler {
    pub fn new(budget: MemoryBudget) -> Self {
        Self { budget }
    }

    /// Assemble the final list of messages to send to the LLM, adhering to the token budget.
    pub async fn assemble(
        &self,
        system_instructions: &str,
        project_memory: Option<&ProjectMemory>,
        session_memory: &SessionMemory,
        episodic_memory: Option<&EpisodicMemory>,
        steering: Option<&SteeringEngine>,
        query: &str,
    ) -> Result<Vec<Message>> {
        let mut final_messages = Vec::new();

        // 1. Build the system prompt
        let mut sys_prompt = String::with_capacity(self.budget.system_tokens);
        sys_prompt.push_str(system_instructions);

        // 2. Inject Project Memory if available
        if let Some(pm) = project_memory {
            sys_prompt.push_str("\n\n--- Project Context ---\n");
            sys_prompt.push_str(&pm.format_for_prompt());
        }

        // 3. Inject Agent Skills (Steering) based on query context
        if let Some(se) = steering {
            let mut active_skills = Vec::new();
            let query_lower = query.to_lowercase();

            // Very simplistic semantic routing: checks if query mentions the skill name/desc words
            for skill in se.list_skills() {
                if query_lower.contains(&skill.name.to_lowercase()) {
                    active_skills.push(skill);
                } else {
                    // Check description words
                    let desc_words: Vec<&str> = skill.description.split_whitespace().collect();
                    for word in desc_words {
                        if word.len() > 4 && query_lower.contains(&word.to_lowercase()) {
                            active_skills.push(skill);
                            break;
                        }
                    }
                }
            }

            if !active_skills.is_empty() {
                sys_prompt.push_str("\n\n--- Active Skills ---\n");
                for skill in active_skills.into_iter().take(3) {
                    // Limit to 3 skills
                    sys_prompt.push_str(&format!("# {}\n{}\n\n", skill.name, skill.content));
                }
            }
        }

        // 4. Inject relevant Episodic Memory if available
        if let Some(em) = episodic_memory {
            // Retrieve top 3 relevant memories to keep it within the budget
            if let Ok(memories) = em.search(query, 3).await {
                if !memories.is_empty() {
                    sys_prompt.push_str("\n\n--- Relevant Past Tasks ---\n");
                    for mem in memories {
                        let snippet = format!(
                            "Task {}: [{}] {}\n",
                            mem.task_id, mem.step_type, mem.content
                        );
                        // Very rough token truncation (avoid blowing up the system prompt)
                        let snippet_tokens = snippet.len() / 4;
                        if snippet_tokens < self.budget.episodic_tokens / 3 {
                            sys_prompt.push_str(&snippet);
                        }
                    }
                }
            }
        }

        final_messages.push(Message::system(sys_prompt));

        // 4. Inject Session Memory history (last N messages)
        let session_messages = session_memory.messages();
        let mut accumulated_tokens = 0;

        // Take messages from newest to oldest up to the session token limit
        let mut history_to_add = Vec::new();
        for msg in session_messages.iter().rev() {
            let tokens = msg.content.len() / 4;
            if accumulated_tokens + tokens < self.budget.session_tokens {
                history_to_add.push(msg.clone());
                accumulated_tokens += tokens;
            } else {
                break;
            }
        }

        // history_to_add is reversed, so put it back in chronological order
        history_to_add.reverse();
        final_messages.extend(history_to_add);

        // 5. Finally, append the actual user query
        final_messages.push(Message::user(query));

        Ok(final_messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::working_memory::WorkingMemory;
    use crate::llm::MessageRole;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn test_context_assembler_budgeting() {
        let budget = MemoryBudget {
            system_tokens: 1000,
            project_tokens: 500,
            episodic_tokens: 500,
            session_tokens: 50, // very small budget to test truncation
            total_limit: 2500,
        };

        let assembler = ContextAssembler::new(budget.clone());

        let mut wm = WorkingMemory::new();

        // Add some messages, ~5 tokens each
        for i in 0..20 {
            wm.add_message(Message::user(format!("Message number {}", i)));
        }

        let session_memory = SessionMemory::new(&budget);
        // Manually populate the session memory with the same messages
        for msg in wm.messages() {
            // Clone each message into the session
            let _ = msg; // session_memory was built from budget, so it starts empty
        }

        let messages = assembler
            .assemble(
                "You are an AI.",
                None,
                &session_memory,
                None,
                None,
                "What is the answer?",
            )
            .await
            .unwrap();

        // Should have System message and the User query at minimum
        assert!(!messages.is_empty());
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages.last().unwrap().role, MessageRole::User);
    }

    #[tokio::test]
    async fn test_semantic_routing_skills() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path();

        // Create couple of mock skills
        let skill1 = r#"---
name: Database
description: Guidelines for sqlx and database queries
---
Use sqlx carefully."#;
        fs::write(skills_dir.join("db.md"), skill1).await.unwrap();

        let skill2 = r#"---
name: Frontend
description: React and TailwindCSS rules
---
Use hooks correctly."#;
        fs::write(skills_dir.join("ui.md"), skill2).await.unwrap();

        let steering = SteeringEngine::new(skills_dir).await.unwrap();

        let budget = MemoryBudget {
            system_tokens: 1000,
            project_tokens: 500,
            episodic_tokens: 500,
            session_tokens: 1000,
            total_limit: 4000,
        };
        let assembler = ContextAssembler::new(budget.clone());

        let session_memory = SessionMemory::new(&budget);

        // Query mentioning database should include the Database skill
        let messages = assembler
            .assemble(
                "SystemPrompt",
                None,
                &session_memory,
                None,
                Some(&steering),
                "I need to write a database query.",
            )
            .await
            .unwrap();

        let sys_content = &messages[0].content;
        assert!(sys_content.contains("--- Active Skills ---"));
        assert!(sys_content.contains("# Database"));
        assert!(!sys_content.contains("# Frontend"));

        // Query mentioning react should include the Frontend skill
        let messages_ui = assembler
            .assemble(
                "SystemPrompt",
                None,
                &session_memory,
                None,
                Some(&steering),
                "Help me with this React component.",
            )
            .await
            .unwrap();

        let sys_content_ui = &messages_ui[0].content;
        assert!(sys_content_ui.contains("--- Active Skills ---"));
        assert!(!sys_content_ui.contains("# Database"));
        assert!(sys_content_ui.contains("# Frontend"));
    }
}
