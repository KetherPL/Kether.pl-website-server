// SPDX-License-Identifier: GPL-3.0-only

use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};

/// REPL (Read-Eval-Print Loop) for interactive command execution
/// 
/// This module provides a basic REPL interface using the reedline crate.
/// It allows users to interact with the server through a command-line interface
/// while the server and SteamBot run in the background.
pub struct Repl {
    editor: Reedline,
    prompt: DefaultPrompt,
}

impl Repl {
    /// Creates a new REPL instance
    /// 
    /// Initializes a new Reedline editor with a default prompt.
    /// 
    /// # Returns
    /// A new `Repl` instance ready to run
    pub fn new() -> Self {
        Self {
            editor: Reedline::create(),
            prompt: DefaultPrompt::new(DefaultPromptSegment::Empty, DefaultPromptSegment::Empty), //Default was exec dir + current time
        }
    }

    /// Runs the REPL loop
    /// 
    /// This function starts the interactive REPL loop. It runs the blocking
    /// reedline operations in a blocking task to avoid blocking the async runtime.
    /// 
    /// The REPL supports the following commands:
    /// - `quit` / `exit` - Exit the REPL
    /// - `help` - Show available commands
    /// - Empty input - Ignored (no-op)
    /// 
    /// # Returns
    /// * `Ok(())` - If the REPL exits normally
    /// * `Err(String)` - If an error occurs
    pub async fn run(mut self) -> Result<(), String> {
        // Run the blocking REPL loop in a blocking task
        tokio::task::spawn_blocking(move || {
            loop {
                // Read a line of input
                match self.editor.read_line(&self.prompt) {
                    Ok(Signal::Success(input)) => {
                        let cmd = input.trim();
                        match cmd {
                            "q" | "quit" | "exit" => {
                                println!("Exiting REPL...");
                                break;
                            }
                            "h" | "help" => {
                                println!("Available commands:");
                                println!("  h, help - Show this help message");
                                println!("  q, quit, exit - Exit the REPL");
                            }
                            "" => {} // ignore empty input
                            other => {
                                println!("Unknown command: {}. Type 'help' for available commands.", other);
                            }
                        }
                    }
                    Ok(Signal::CtrlC) => {
                        println!("Interrupted (Ctrl+C). Type 'q' / 'quit' to exit.");
                    }
                    Ok(Signal::CtrlD) => {
                        println!("EOF received. Exiting...");
                        break;
                    }
                    Err(err) => {
                        println!("Error reading line: {}", err);
                        break;
                    }
                }
            }

            Ok::<(), String>(())
        })
        .await
        .map_err(|e| format!("REPL task join error: {}", e))?
    }
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

/// Starts a key listener that activates the REPL when 'C' is pressed
/// 
/// This function runs in the background and listens for 'C' key press (single key, no Enter needed).
/// When 'C' is detected, it spawns the REPL. The REPL takes over the
/// terminal until the user exits it, then normal operation resumes.
/// 
/// # Returns
/// * `Ok(())` - Never returns successfully (runs indefinitely)
/// * `Err(String)` - If key listener setup fails
pub async fn start_key_listener() -> Result<(), String> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind};
    
    println!("Type 'C' and press Enter to open the REPL console");
    
    loop {
        // Run the key listener in a blocking task since crossterm events are blocking
        let key_detected = tokio::task::spawn_blocking(move || {
            loop {
                // Poll for events with a timeout to avoid blocking indefinitely
                if let Ok(true) = event::poll(std::time::Duration::from_millis(100))
                    && let Ok(Event::Key(key_event)) = event::read()
                {
                    // Only process key press events (not key release)
                    if key_event.kind == KeyEventKind::Press {
                        match key_event.code {
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                return true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        })
        .await
        .map_err(|e| format!("Key listener task join error: {}", e))?;
        
        if key_detected {
            // After detecting 'C', spawn the REPL
            println!("\nOpening REPL console... (Type 'quit' or 'exit' to close)");
            let repl = Repl::new();
            if let Err(e) = repl.run().await {
                eprintln!("REPL error: {}", e);
            }
            println!("REPL closed. Type 'C' to open again.");
        }
    }
}

