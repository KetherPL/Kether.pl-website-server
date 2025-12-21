// SPDX-License-Identifier: GPL-3.0-only

use SC_Sub_Poster::EnhancedGroupChatMessage;
use std::collections::HashMap;

/// Trait for command handlers
/// 
/// Commands implement this trait to handle specific command execution.
pub trait CommandHandler: Send + Sync {
    /// Executes the command with the given arguments
    /// 
    /// # Arguments
    /// * `args` - The command arguments (everything after the command name)
    /// * `message` - The original message that triggered the command
    /// 
    /// # Returns
    /// The response message to send back to the chat
    fn execute(&self, args: &str, message: &EnhancedGroupChatMessage) -> String;
}

/// Registry for command handlers
/// 
/// Maps command names to their handlers and provides command execution.
pub struct CommandRegistry {
    handlers: HashMap<String, Box<dyn CommandHandler>>,
}

impl CommandRegistry {
    /// Creates a new command registry with all registered commands
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };
        
        // Register the test command
        registry.register("test", Box::new(TestCommand));
        
        registry
    }
    
    /// Registers a command handler
    /// 
    /// # Arguments
    /// * `name` - The command name (case-insensitive)
    /// * `handler` - The command handler implementation
    fn register(&mut self, name: &str, handler: Box<dyn CommandHandler>) {
        self.handlers.insert(name.to_lowercase(), handler);
    }
    
    /// Handles a command execution
    /// 
    /// # Arguments
    /// * `command` - The command name (case-insensitive)
    /// * `args` - The command arguments
    /// * `message` - The original message
    /// 
    /// # Returns
    /// * `Some(String)` - The response message if command is found
    /// * `None` - If command is not found
    pub fn handle(&self, command: &str, args: &str, message: &EnhancedGroupChatMessage) -> Option<String> {
        let command_lower = command.to_lowercase();
        if let Some(handler) = self.handlers.get(&command_lower) {
            Some(handler.execute(args, message))
        } else {
            None
        }
    }
}

/// Test command handler
/// 
/// Responds with "Test successful!" when executed.
struct TestCommand;

impl CommandHandler for TestCommand {
    fn execute(&self, _args: &str, _message: &EnhancedGroupChatMessage) -> String {
        "Test successful!".to_string()
    }
}

