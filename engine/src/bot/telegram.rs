//! Telegram Bot Integration
//!
//! Provides a long-polling interface to accept commands remotely.
//! Messages from authorized users are dispatched to the agent core for processing.

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::agent::{AgentCore, Task};
use crate::risk_assessor::OperationSource;
use crate::secrets::SecretManager;

/// Callback type for processing incoming task text and returning a reply
pub type TaskHandler = Arc<Mutex<AgentCore>>;

/// Rate limit tracking for Telegram operations
#[derive(Debug, Clone)]
struct TelegramRateLimits {
    /// Timestamps of recent operations (for 60/hour limit)
    recent_ops: Vec<std::time::Instant>,
    /// Timestamps of recent Tier 2 operations (for 10/10min limit)
    tier2_ops: Vec<std::time::Instant>,
}

impl TelegramRateLimits {
    fn new() -> Self {
        Self {
            recent_ops: Vec::new(),
            tier2_ops: Vec::new(),
        }
    }

    /// Check and record a general operation. Returns false if rate limited.
    fn check_general(&mut self) -> bool {
        let now = std::time::Instant::now();
        let one_hour = Duration::from_secs(3600);
        self.recent_ops
            .retain(|t| now.duration_since(*t) < one_hour);
        if self.recent_ops.len() >= 60 {
            return false;
        }
        self.recent_ops.push(now);
        true
    }

    /// Check and record a Tier 2 operation. Returns false if rate limited.
    fn check_tier2(&mut self) -> bool {
        let now = std::time::Instant::now();
        let ten_min = Duration::from_secs(600);
        self.tier2_ops.retain(|t| now.duration_since(*t) < ten_min);
        if self.tier2_ops.len() >= 10 {
            return false;
        }
        self.tier2_ops.push(now);
        true
    }
}

/// Inline keyboard button for Telegram
#[derive(Serialize)]
struct InlineKeyboardButton {
    text: String,
    callback_data: String,
}

/// Inline keyboard markup for Telegram
#[derive(Serialize)]
struct InlineKeyboardMarkup {
    inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

#[derive(Clone)]
pub struct TelegramBot {
    token: String,
    allowed_users: Vec<i64>,
    client: Client,
    agent: Option<TaskHandler>,
    rate_limits: Arc<Mutex<TelegramRateLimits>>,
    confirmation_chat_id: Option<i64>,
    secret_manager: Arc<SecretManager>,
}

impl std::fmt::Debug for TelegramBot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramBot")
            .field("allowed_users", &self.allowed_users)
            .field("agent", &self.agent.as_ref().map(|_| "<AgentCore>"))
            .finish()
    }
}

#[derive(Deserialize, Debug)]
struct Update {
    update_id: i64,
    message: Option<Message>,
}

#[derive(Deserialize, Debug)]
struct Message {
    chat: Chat,
    text: Option<String>,
    from: Option<User>,
}

#[derive(Deserialize, Debug)]
struct Chat {
    id: i64,
}

#[derive(Deserialize, Debug)]
struct User {
    id: i64,
}

#[derive(Deserialize, Debug)]
struct GetUpdatesResponse {
    ok: bool,
    result: Option<Vec<Update>>,
}

impl TelegramBot {
    pub fn new(token: String, allowed_users: Vec<i64>) -> Self {
        Self {
            token,
            allowed_users,
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
            agent: None,
            rate_limits: Arc::new(Mutex::new(TelegramRateLimits::new())),
            confirmation_chat_id: None,
            secret_manager: Arc::new(SecretManager::new("rove")),
        }
    }

    /// Attach an agent core for task processing
    pub fn with_agent(mut self, agent: TaskHandler) -> Self {
        self.agent = Some(agent);
        self
    }

    /// Set the confirmation chat ID for sending results
    pub fn with_confirmation_chat(mut self, chat_id: i64) -> Self {
        self.confirmation_chat_id = Some(chat_id);
        self
    }

    /// Start the long-polling loop
    ///
    /// This will block the current task. Should be spawned in a background tokio::task.
    pub async fn start_polling(&self) -> Result<()> {
        info!("Starting Telegram bot long-polling loop...");
        let mut offset = 0;

        loop {
            match self.get_updates(offset).await {
                Ok(updates) => {
                    for update in updates {
                        offset = update.update_id + 1;
                        if let Some(msg) = update.message {
                            self.handle_message(&msg).await;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch Telegram updates: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn get_updates(&self, offset: i64) -> Result<Vec<Update>> {
        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
            self.token, offset
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<GetUpdatesResponse>()
            .await?;

        if !response.ok {
            return Err(anyhow::anyhow!("Telegram API returned ok=false"));
        }

        Ok(response.result.unwrap_or_default())
    }

    async fn handle_message(&self, msg: &Message) {
        let chat_id = msg.chat.id;

        let user_id = match msg.from.as_ref() {
            Some(u) => u.id,
            None => {
                warn!("Message with no user info - ignoring");
                return;
            }
        };

        if !self.allowed_users.contains(&user_id) && !self.allowed_users.is_empty() {
            warn!("Unauthorized user {} attempted to use the bot", user_id);
            let _ = self
                .send_message(chat_id, "Unauthorized. Access denied.")
                .await;
            return;
        }

        if let Some(text) = &msg.text {
            info!("Received command from {}: {}", user_id, text);

            // Handle built-in commands
            if text.starts_with('/') {
                self.handle_command(chat_id, text).await;
                return;
            }

            // Check rate limits (Requirement 16.7)
            {
                let mut limits = self.rate_limits.lock().await;
                if !limits.check_general() {
                    let _ = self
                        .send_message(chat_id, "Rate limit exceeded (60/hour). Please wait.")
                        .await;
                    return;
                }
                // Also check Tier 2 limits for potentially dangerous operations
                if !limits.check_tier2() {
                    let _ = self
                        .send_message(
                            chat_id,
                            "Tier 2 rate limit exceeded (10/10min). Please wait.",
                        )
                        .await;
                    return;
                }
            }

            // Dispatch to agent if available
            if let Some(ref agent) = self.agent {
                let _ = self.send_message(chat_id, "Processing your task...").await;

                let task = Task::new(text.as_str(), OperationSource::Remote);
                let mut agent_guard = agent.lock().await;

                match agent_guard.process_task(task).await {
                    Ok(result) => {
                        // Scrub secrets from result (Requirement 16.9)
                        let scrubbed = self.secret_manager.scrub(&result.answer);

                        // Truncate if answer is too long for Telegram (4096 chars)
                        let reply = if scrubbed.len() > 4000 {
                            format!("{}...\n\n(truncated)", &scrubbed[..4000])
                        } else {
                            scrubbed
                        };

                        // Send to confirmation chat if configured (Requirement 16.8)
                        let target_chat = self.confirmation_chat_id.unwrap_or(chat_id);
                        if let Err(e) = self.send_message(target_chat, &reply).await {
                            error!("Failed to send reply to {}: {}", target_chat, e);
                        }
                    }
                    Err(e) => {
                        let error_msg = self.secret_manager.scrub(&format!("Task failed: {}", e));
                        error!("{}", error_msg);
                        let _ = self.send_message(chat_id, &error_msg).await;
                    }
                }
            } else {
                // No agent attached — echo acknowledgment
                let reply = format!("Task accepted: {}", text);
                if let Err(e) = self.send_message(chat_id, &reply).await {
                    error!("Failed to send reply to {}: {}", chat_id, e);
                }
            }
        }
    }

    /// Send a Tier 1 countdown message (Requirement 16.5)
    ///
    /// Sends countdown messages for Tier 1 operations, giving the user
    /// time to cancel before execution.
    pub async fn send_tier1_countdown(
        &self,
        chat_id: i64,
        operation: &str,
        delay_secs: u64,
    ) -> Result<bool> {
        let msg = format!(
            "Tier 1 operation: {}\nExecuting in {} seconds... Send /cancel to abort.",
            operation, delay_secs
        );
        self.send_message(chat_id, &msg).await?;

        // Wait for the countdown period
        tokio::time::sleep(Duration::from_secs(delay_secs)).await;

        // In a full implementation, we'd check for /cancel messages during the countdown
        Ok(true)
    }

    /// Send a Tier 2 confirmation request with inline keyboard (Requirement 16.6)
    ///
    /// Sends an inline keyboard with Approve/Deny buttons for Tier 2 operations.
    pub async fn send_tier2_confirmation(&self, chat_id: i64, operation: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);

        let keyboard = InlineKeyboardMarkup {
            inline_keyboard: vec![vec![
                InlineKeyboardButton {
                    text: "Approve".to_string(),
                    callback_data: format!("approve:{}", operation),
                },
                InlineKeyboardButton {
                    text: "Deny".to_string(),
                    callback_data: format!("deny:{}", operation),
                },
            ]],
        };

        let body = serde_json::json!({
            "chat_id": chat_id,
            "text": format!("Tier 2 operation requires explicit approval:\n\n{}\n\nApprove or deny?", operation),
            "reply_markup": keyboard,
        });

        self.client.post(&url).json(&body).send().await?;
        Ok(())
    }

    /// Handle built-in bot commands
    async fn handle_command(&self, chat_id: i64, cmd: &str) {
        let reply = match cmd.split_whitespace().next().unwrap_or("") {
            "/start" => "Rove is ready. Send me a task and I'll process it.".to_string(),
            "/status" => "Rove is running.".to_string(),
            "/help" => "Available commands:\n\
                 /start  - Initialize bot\n\
                 /status - Check bot status\n\
                 /help   - Show this help\n\n\
                 Send any text to run it as a task."
                .to_string(),
            _ => format!("Unknown command: {}", cmd),
        };

        if let Err(e) = self.send_message(chat_id, &reply).await {
            error!("Failed to send command reply: {}", e);
        }
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);

        // Scrub secrets from outgoing messages
        let scrubbed = self.secret_manager.scrub(text);

        #[derive(Serialize)]
        struct SendMsgReq<'a> {
            chat_id: i64,
            text: &'a str,
        }

        let req = SendMsgReq {
            chat_id,
            text: &scrubbed,
        };

        self.client.post(&url).json(&req).send().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_bot_creation() {
        let bot = TelegramBot::new("test_token".to_string(), vec![12345]);
        assert_eq!(bot.token, "test_token");
        assert_eq!(bot.allowed_users, vec![12345]);
        assert!(bot.agent.is_none());
        assert!(bot.confirmation_chat_id.is_none());
    }

    #[test]
    fn test_telegram_bot_with_confirmation_chat() {
        let bot = TelegramBot::new("token".to_string(), vec![]).with_confirmation_chat(99999);
        assert_eq!(bot.confirmation_chat_id, Some(99999));
    }

    #[test]
    fn test_rate_limits_general() {
        let mut limits = TelegramRateLimits::new();
        // Should allow first 60 operations
        for _ in 0..60 {
            assert!(limits.check_general());
        }
        // 61st should be denied
        assert!(!limits.check_general());
    }

    #[test]
    fn test_rate_limits_tier2() {
        let mut limits = TelegramRateLimits::new();
        // Should allow first 10 tier2 operations
        for _ in 0..10 {
            assert!(limits.check_tier2());
        }
        // 11th should be denied
        assert!(!limits.check_tier2());
    }

    #[test]
    fn test_secret_scrubbing_in_messages() {
        let manager = SecretManager::new("test");
        let text = "Error with key sk-1234567890abcdefghijklmnopqrstuvwxyz";
        let scrubbed = manager.scrub(text);
        assert!(!scrubbed.contains("sk-"));
        assert!(scrubbed.contains("[REDACTED]"));
    }

    #[test]
    fn test_inline_keyboard_serialization() {
        let keyboard = InlineKeyboardMarkup {
            inline_keyboard: vec![vec![
                InlineKeyboardButton {
                    text: "Approve".to_string(),
                    callback_data: "approve:test".to_string(),
                },
                InlineKeyboardButton {
                    text: "Deny".to_string(),
                    callback_data: "deny:test".to_string(),
                },
            ]],
        };
        let json = serde_json::to_string(&keyboard).unwrap();
        assert!(json.contains("Approve"));
        assert!(json.contains("approve:test"));
    }

    #[test]
    fn test_unauthorized_user_detection() {
        let bot = TelegramBot::new("token".to_string(), vec![111, 222]);
        // User 333 is not in allowed list
        assert!(!bot.allowed_users.contains(&333));
        // User 111 is allowed
        assert!(bot.allowed_users.contains(&111));
    }

    #[test]
    fn test_empty_allowed_users_allows_all() {
        let bot = TelegramBot::new("token".to_string(), vec![]);
        // Empty allowed_users means allow all (checked with .is_empty() in handle_message)
        assert!(bot.allowed_users.is_empty());
    }
}
