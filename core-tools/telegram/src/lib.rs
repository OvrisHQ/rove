//! Telegram Bot Core Tool
//!
//! Provides Telegram bot integration for Rove

use sdk::{CoreContext, CoreTool, EngineError, ToolInput, ToolOutput};

/// Telegram bot controller
pub struct TelegramBot {
    ctx: Option<CoreContext>,
}

impl TelegramBot {
    /// Create a new TelegramBot instance
    pub fn new() -> Self {
        Self { ctx: None }
    }
}

impl Default for TelegramBot {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreTool for TelegramBot {
    fn name(&self) -> &str {
        "telegram"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn start(&mut self, ctx: CoreContext) -> Result<(), EngineError> {
        self.ctx = Some(ctx);
        tracing::info!("Telegram bot started");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), EngineError> {
        tracing::info!("Telegram bot stopped");
        Ok(())
    }

    fn handle(&self, _input: ToolInput) -> Result<ToolOutput, EngineError> {
        Ok(ToolOutput::empty())
    }
}

/// FFI export for creating the tool
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn create_tool() -> *mut dyn CoreTool {
    Box::into_raw(Box::new(TelegramBot::new()))
}
