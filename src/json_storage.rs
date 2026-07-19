// SPDX-License-Identifier: GPL-3.0-only

use crate::json_api::models::{Bind, BindSuggestion, Command};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use rocket::serde::{Deserialize, Serialize};
use tokio::fs;

/// File names for JSON storage
const COMMANDS_FILE: &str = "cmds.json";
const BINDS_FILE: &str = "binds.json";
const BIND_SUGGESTIONS_FILE: &str = "bind_sgs.json";

/// Internal representation of the JSON database
/// 
/// This struct holds all data in memory as HashMaps for fast access.
/// Each collection is stored as a map from ID to the data object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct JsonDatabaseInternal {
    commands: HashMap<String, CommandData>,
    binds: HashMap<String, BindData>,
    bind_suggestions: HashMap<String, BindSuggestionData>,
}

/// Command data as stored in JSON (without id field)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct CommandData {
    command: String,
    description: String,
}

/// Bind data as stored in JSON (without id field, with voting)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct BindData {
    author: String,
    text: String,
    #[serde(default)]
    upvote: Vec<i64>,
    #[serde(default)]
    downvote: Vec<i64>,
}

/// Bind suggestion data as stored in JSON (without id field)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct BindSuggestionData {
    author: String,
    text: String,
    proposed_by: String,
}

/// Thread-safe JSON database wrapper
/// 
/// Provides concurrent read access and exclusive write access to the
/// in-memory database. Changes are persisted to JSON files atomically.
#[derive(Debug, Clone)]
pub struct JsonDatabase {
    inner: Arc<RwLock<JsonDatabaseInternal>>,
    save_tx: tokio::sync::mpsc::Sender<()>,
}

impl JsonDatabase {
    /// Creates a new JsonDatabase and loads data from JSON files
    /// 
    /// Files are loaded from the same directory as the executable.
    /// If files don't exist, they are created with empty collections.
    /// 
    /// # Returns
    /// * `Ok(JsonDatabase)` - Successfully loaded database
    /// * `Err(String)` - If loading fails
    pub async fn load() -> Result<Self, String> {
        let base_path = crate::config::exe_dir()
            .map_err(|e| format!("Failed to get executable directory: {}", e))?;
        
        println!("Loading JSON database from: {:?}", base_path);
        
        // Load commands
        let commands = Self::load_json_file::<HashMap<String, CommandData>>(
            &base_path.join(COMMANDS_FILE)
        ).await.unwrap_or_else(|_| HashMap::new());
        
        // Load binds
        let binds = Self::load_json_file::<HashMap<String, BindData>>(
            &base_path.join(BINDS_FILE)
        ).await.unwrap_or_else(|_| HashMap::new());
        
        // Load bind suggestions
        let bind_suggestions = Self::load_json_file::<HashMap<String, BindSuggestionData>>(
            &base_path.join(BIND_SUGGESTIONS_FILE)
        ).await.unwrap_or_else(|_| HashMap::new());
        
        println!("Loaded {} commands, {} binds, {} bind suggestions", 
            commands.len(), binds.len(), bind_suggestions.len());
        
        let internal = JsonDatabaseInternal {
            commands,
            binds,
            bind_suggestions,
        };
        
                let inner = Arc::new(RwLock::new(internal));
        let (save_tx, mut save_rx) = tokio::sync::mpsc::channel(100);
        
        let bg_inner = inner.clone();
        let bg_base_path = base_path.clone();
        tokio::spawn(async move {
            loop {
                if save_rx.recv().await.is_none() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                while let Ok(_) = save_rx.try_recv() {}
                
                let (commands_clone, binds_clone, suggestions_clone) = {
                    if let Ok(db) = bg_inner.read() {
                        (db.commands.clone(), db.binds.clone(), db.bind_suggestions.clone())
                    } else {
                        continue;
                    }
                };
                
                let _ = Self::save_json_file(&bg_base_path.join(COMMANDS_FILE), &commands_clone).await;
                let _ = Self::save_json_file(&bg_base_path.join(BINDS_FILE), &binds_clone).await;
                let _ = Self::save_json_file(&bg_base_path.join(BIND_SUGGESTIONS_FILE), &suggestions_clone).await;
            }
        });
        
        Ok(JsonDatabase {
            inner,
            save_tx,
        })
    }
    
    /// Loads a JSON file and deserializes it
    async fn load_json_file<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<T, String> {
        let content = fs::read_to_string(path).await
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        
        rocket::serde::json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
    }
    
    /// Saves a JSON file atomically (write to temp, then rename)
    async fn save_json_file<T: Serialize>(path: &PathBuf, data: &T) -> Result<(), String> {
        let json = rocket::serde::json::to_pretty_string(data)
            .map_err(|e| format!("Failed to serialize data: {}", e))?;
        
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, json).await
            .map_err(|e| format!("Failed to write temp file: {}", e))?;
        
        fs::rename(&temp_path, path).await
            .map_err(|e| format!("Failed to rename temp file: {}", e))?;
        
        Ok(())
    }
    
    /// Get next available ID for a collection
    fn next_id(map: &HashMap<String, impl Serialize>) -> i32 {
        map.keys()
            .filter_map(|k| k.parse::<i32>().ok())
            .max()
            .unwrap_or(0) + 1
    }
    
    // === Commands Operations ===
    
    pub fn get_all_commands(&self) -> Result<Vec<Command>, String> {
        let db = self.inner.read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        
        let mut commands: Vec<Command> = db.commands.iter()
            .map(|(id_str, data)| Command {
                id: id_str.parse().unwrap_or(0),
                command: data.command.clone(),
                description: data.description.clone(),
            })
            .collect();
        
        commands.sort_by_key(|c| c.id);
        Ok(commands)
    }
    
    pub fn get_command(&self, id: i32) -> Result<Command, String> {
        let db = self.inner.read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        
        let id_str = id.to_string();
        db.commands.get(&id_str)
            .map(|data| Command {
                id,
                command: data.command.clone(),
                description: data.description.clone(),
            })
            .ok_or_else(|| "Command not found".to_string())
    }
    
    pub async fn create_command(&self, command: String, description: String) -> Result<Command, String> {
        let (id, _commands_clone) = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            // Check for duplicate command
            if db.commands.values().any(|c| c.command == command) {
                return Err("Command already exists".to_string());
            }
            
            let id = Self::next_id(&db.commands);
            let id_str = id.to_string();
            
            let data = CommandData {
                command: command.clone(),
                description: description.clone(),
            };
            
            db.commands.insert(id_str, data);
            (id, db.commands.clone())
        }; // Lock is dropped here
        
        // Save to file (async, no lock held)
        let _ = self.save_tx.send(()).await;
        
        Ok(Command { id, command, description })
    }
    
    pub async fn update_command(&self, id: i32, command: String, description: String) -> Result<Command, String> {
        let _commands_clone = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            let id_str = id.to_string();
            
            if !db.commands.contains_key(&id_str) {
                return Err("Command not found".to_string());
            }
            
            // Check for duplicate command (excluding current)
            if db.commands.iter()
                .any(|(k, v)| k != &id_str && v.command == command) {
                return Err("Command already exists".to_string());
            }
            
            db.commands.insert(id_str, CommandData {
                command: command.clone(),
                description: description.clone(),
            });
            
            db.commands.clone()
        }; // Lock is dropped here
        
        let _ = self.save_tx.send(()).await;
        
        Ok(Command { id, command, description })
    }
    
    pub async fn delete_command(&self, id: i32) -> Result<(), String> {
        let _commands_clone = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            let id_str = id.to_string();
            
            if db.commands.remove(&id_str).is_none() {
                return Err("Command not found".to_string());
            }
            
            db.commands.clone()
        }; // Lock is dropped here
        
        let _ = self.save_tx.send(()).await;
        
        Ok(())
    }
    
    // === Binds Operations ===
    
    pub fn get_all_binds(&self) -> Result<Vec<Bind>, String> {
        let db = self.inner.read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        
        let mut binds: Vec<Bind> = db.binds.iter()
            .map(|(id_str, data)| Bind {
                id: id_str.parse().unwrap_or(0),
                author: data.author.clone(),
                text: data.text.clone(),
                upvote: data.upvote.clone(),
                downvote: data.downvote.clone(),
            })
            .collect();
        
        binds.sort_by_key(|b| b.id);
        Ok(binds)
    }
    
    pub fn get_bind(&self, id: i32) -> Result<Bind, String> {
        let db = self.inner.read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        
        let id_str = id.to_string();
        db.binds.get(&id_str)
            .map(|data| Bind {
                id,
                author: data.author.clone(),
                text: data.text.clone(),
                upvote: data.upvote.clone(),
                downvote: data.downvote.clone(),
            })
            .ok_or_else(|| "Bind not found".to_string())
    }
    
    pub async fn create_bind(&self, author: String, text: String) -> Result<Bind, String> {
        let (id, _binds_clone) = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            // Check for duplicate text
            if db.binds.values().any(|b| b.text == text) {
                return Err("Bind text already exists".to_string());
            }
            
            let id = Self::next_id(&db.binds);
            let id_str = id.to_string();
            
            let data = BindData {
                author: author.clone(),
                text: text.clone(),
                upvote: Vec::new(),
                downvote: Vec::new(),
            };
            
            db.binds.insert(id_str, data);
            (id, db.binds.clone())
        }; // Lock is dropped here
        
        let _ = self.save_tx.send(()).await;
        
        Ok(Bind { 
            id, 
            author, 
            text,
            upvote: Vec::new(),
            downvote: Vec::new(),
        })
    }
    
    pub async fn update_bind(&self, id: i32, author: String, text: String, upvote: Vec<i64>, downvote: Vec<i64>) -> Result<Bind, String> {
        let _binds_clone = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            let id_str = id.to_string();
            
            if !db.binds.contains_key(&id_str) {
                return Err("Bind not found".to_string());
            }
            
            // Check for duplicate text (excluding current)
            if db.binds.iter()
                .any(|(k, v)| k != &id_str && v.text == text) {
                return Err("Bind text already exists".to_string());
            }
            
            db.binds.insert(id_str, BindData {
                author: author.clone(),
                text: text.clone(),
                upvote: upvote.clone(),
                downvote: downvote.clone(),
            });
            
            db.binds.clone()
        }; // Lock is dropped here
        
        let _ = self.save_tx.send(()).await;
        
        Ok(Bind { id, author, text, upvote, downvote })
    }
    
    pub async fn delete_bind(&self, id: i32) -> Result<(), String> {
        let _binds_clone = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            let id_str = id.to_string();
            
            if db.binds.remove(&id_str).is_none() {
                return Err("Bind not found".to_string());
            }
            
            db.binds.clone()
        }; // Lock is dropped here
        
        let _ = self.save_tx.send(()).await;
        
        Ok(())
    }
    
    // === Bind Suggestions Operations ===
    
    pub fn get_all_bind_suggestions(&self) -> Result<Vec<BindSuggestion>, String> {
        let db = self.inner.read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        
        let mut suggestions: Vec<BindSuggestion> = db.bind_suggestions.iter()
            .map(|(id_str, data)| BindSuggestion {
                id: id_str.parse().unwrap_or(0),
                author: data.author.clone(),
                text: data.text.clone(),
                proposed_by: data.proposed_by.clone(),
            })
            .collect();
        
        suggestions.sort_by_key(|s| s.id);
        Ok(suggestions)
    }
    
    pub fn get_bind_suggestion(&self, id: i32) -> Result<BindSuggestion, String> {
        let db = self.inner.read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        
        let id_str = id.to_string();
        db.bind_suggestions.get(&id_str)
            .map(|data| BindSuggestion {
                id,
                author: data.author.clone(),
                text: data.text.clone(),
                proposed_by: data.proposed_by.clone(),
            })
            .ok_or_else(|| "Bind suggestion not found".to_string())
    }
    
    pub async fn create_bind_suggestion(&self, author: String, text: String, proposed_by: String) -> Result<BindSuggestion, String> {
        let (id, _suggestions_clone) = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            // Check for duplicate text
            if db.bind_suggestions.values().any(|s| s.text == text) {
                return Err("Bind suggestion text already exists".to_string());
            }
            
            let id = Self::next_id(&db.bind_suggestions);
            let id_str = id.to_string();
            
            let data = BindSuggestionData {
                author: author.clone(),
                text: text.clone(),
                proposed_by: proposed_by.clone(),
            };
            
            db.bind_suggestions.insert(id_str, data);
            (id, db.bind_suggestions.clone())
        }; // Lock is dropped here
        
        let _ = self.save_tx.send(()).await;
        
        Ok(BindSuggestion { id, author, text, proposed_by })
    }
    
    pub async fn update_bind_suggestion(&self, id: i32, author: String, text: String, proposed_by: String) -> Result<BindSuggestion, String> {
        let _suggestions_clone = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            let id_str = id.to_string();
            
            if !db.bind_suggestions.contains_key(&id_str) {
                return Err("Bind suggestion not found".to_string());
            }
            
            // Check for duplicate text (excluding current)
            if db.bind_suggestions.iter()
                .any(|(k, v)| k != &id_str && v.text == text) {
                return Err("Bind suggestion text already exists".to_string());
            }
            
            db.bind_suggestions.insert(id_str, BindSuggestionData {
                author: author.clone(),
                text: text.clone(),
                proposed_by: proposed_by.clone(),
            });
            
            db.bind_suggestions.clone()
        }; // Lock is dropped here
        
        let _ = self.save_tx.send(()).await;
        
        Ok(BindSuggestion { id, author, text, proposed_by })
    }
    
    pub async fn delete_bind_suggestion(&self, id: i32) -> Result<(), String> {
        let _suggestions_clone = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            let id_str = id.to_string();
            
            if db.bind_suggestions.remove(&id_str).is_none() {
                return Err("Bind suggestion not found".to_string());
            }
            
            db.bind_suggestions.clone()
        }; // Lock is dropped here
        
        let _ = self.save_tx.send(()).await;
        
        Ok(())
    }
    
    // === Voting Operations ===
    
    /// Add or update a vote on a bind
    /// 
    /// If the voter already voted, their vote is moved to the new category.
    /// A voter can only be in either upvote or downvote, not both.
    pub async fn add_vote(&self, bind_id: i32, voter_steam_id: i64, vote_type: &str) -> Result<(), String> {
        let _binds_clone = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            let id_str = bind_id.to_string();
            let bind_data = db.binds.get_mut(&id_str)
                .ok_or_else(|| "Bind not found".to_string())?;
            
            // Remove voter from both arrays first
            bind_data.upvote.retain(|id| id != &voter_steam_id);
            bind_data.downvote.retain(|id| id != &voter_steam_id);
            
            // Add to the appropriate array (case-insensitive)
            match vote_type.to_lowercase().as_str() {
                "upvote" => bind_data.upvote.push(voter_steam_id),
                "downvote" => bind_data.downvote.push(voter_steam_id),
                _ => return Err("Invalid vote type".to_string()),
            }
            
            db.binds.clone()
        }; // Lock is dropped here
        
        let _ = self.save_tx.send(()).await;
        
        Ok(())
    }
    
    /// Remove a vote from a bind
    pub async fn remove_vote(&self, bind_id: i32, voter_steam_id: i64) -> Result<(), String> {
        let _binds_clone = {
            let mut db = self.inner.write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            
            let id_str = bind_id.to_string();
            let bind_data = db.binds.get_mut(&id_str)
                .ok_or_else(|| "Bind not found".to_string())?;
            
            // Remove from both arrays
            bind_data.upvote.retain(|id| id != &voter_steam_id);
            bind_data.downvote.retain(|id| id != &voter_steam_id);
            
            db.binds.clone()
        }; // Lock is dropped here

        let _ = self.save_tx.send(()).await;
        Ok(())
    }
}

