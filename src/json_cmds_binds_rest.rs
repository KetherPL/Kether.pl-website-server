// SPDX-License-Identifier: GPL-3.0-only

use rocket::options;
use rocket::{get, post, put, routes, serde::json::Json, State};
use rocket::http::Status;
use rocket::serde::{Deserialize, Serialize};
use crate::json_storage::JsonDatabase;
use crate::config::Config;

// Data structures for JSON storage
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Bind {
    pub id: i32,
    pub author: String,
    pub text: String,
    pub upvote: Vec<i64>,   // Array of Steam IDs who upvoted
    pub downvote: Vec<i64>, // Array of Steam IDs who downvoted
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct NewBind {
    pub author: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DelBind {
    pub id: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct BindSuggestion {
    pub id: i32,
    pub author: String,
    pub text: String,
    pub proposed_by: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct NewBindSuggestion {
    pub author: String,
    pub text: String,
    pub proposed_by: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DelBindSuggestion {
    pub id: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Command {
    pub id: i32,
    pub command: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct NewCommand {
    pub command: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DelCommand {
    pub id: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct BindVoting {
    pub id: i32,
    pub voter_steam_id: i64,
    pub voted_bind_id: i32,
    pub vote: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct NewBindVoting {
    pub voter_steam_id: i64,
    pub voted_bind_id: i32,
    pub vote: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DelBindVoting {
    pub id: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DelBindVotingByUser {
    pub voter_steam_id: i64,
    pub voted_bind_id: i32,
}

// --- Frontend Admin verification structs ---
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct VerifyAdminRequest {
    pub steam_id: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct VerifyAdminResponse {
    pub is_admin: bool,
}

// --- Binds ---

#[get("/binds/<bind_id>")]
pub fn get_bind_by_id(db: &State<JsonDatabase>, bind_id: i32) -> Result<Json<Bind>, Status> {
    db.get_bind(bind_id)
        .map(Json)
        .map_err(|e| {
            eprintln!("Error getting bind: {}", e);
            Status::NotFound
        })
}

#[get("/binds/getBinds")]
pub fn get_all_binds(db: &State<JsonDatabase>) -> Result<Json<Vec<Bind>>, Status> {
    db.get_all_binds()
        .map(Json)
        .map_err(|e| {
            eprintln!("Error getting binds: {}", e);
            Status::InternalServerError
        })
}

#[post("/binds/addBind", data = "<new_bind>")]
pub async fn create_new_bind(db: &State<JsonDatabase>, new_bind: Json<NewBind>) -> Result<Json<Bind>, Status> {
    db.create_bind(new_bind.author.clone(), new_bind.text.clone()).await
        .map(Json)
        .map_err(|e| {
            eprintln!("Error creating bind: {}", e);
            if e.contains("already exists") {
                Status::Conflict
            } else {
                Status::InternalServerError
            }
        })
}

#[post("/binds/deleteBind", data = "<bind_to_delete>")]
pub async fn delete_existing_bind(db: &State<JsonDatabase>, bind_to_delete: Json<DelBind>) -> Result<Json<usize>, Status> {
    db.delete_bind(bind_to_delete.id).await
        .map(|_| Json(1))
        .map_err(|e| {
            eprintln!("Error deleting bind: {}", e);
            Status::NotFound
        })
}

#[post("/binds/updateBind", data = "<bind_to_update>")]
pub async fn update_existing_bind(db: &State<JsonDatabase>, bind_to_update: Json<Bind>) -> Result<Json<Bind>, Status> {
    db.update_bind(
        bind_to_update.id,
        bind_to_update.author.clone(),
        bind_to_update.text.clone(),
        bind_to_update.upvote.clone(),
        bind_to_update.downvote.clone(),
    ).await
    .map(Json)
    .map_err(|e| {
        eprintln!("Error updating bind: {}", e);
        if e.contains("not found") {
            Status::NotFound
        } else if e.contains("already exists") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })
}

#[options("/binds/addBind")]
pub fn options_create_new_bind() -> Status {
    Status::Ok
}

#[options("/binds/deleteBind")]
pub fn options_delete_existing_bind() -> Status {
    Status::Ok
}

#[options("/binds/updateBind")]
pub fn options_update_existing_bind() -> Status {
    Status::Ok
}

// --- Bind Suggestions ---

#[get("/bind_suggestions/<bind_suggestion_id>")]
pub fn get_bind_suggestion_by_id(db: &State<JsonDatabase>, bind_suggestion_id: i32) -> Result<Json<BindSuggestion>, Status> {
    db.get_bind_suggestion(bind_suggestion_id)
        .map(Json)
        .map_err(|e| {
            eprintln!("Error getting bind suggestion: {}", e);
            Status::NotFound
        })
}

#[get("/bind_suggestions/getBindSuggestions")]
pub fn get_all_bind_suggestions(db: &State<JsonDatabase>) -> Result<Json<Vec<BindSuggestion>>, Status> {
    db.get_all_bind_suggestions()
        .map(Json)
        .map_err(|e| {
            eprintln!("Error getting bind suggestions: {}", e);
            Status::InternalServerError
        })
}

#[post("/bind_suggestions/addBindSuggestion", data = "<new_bind_suggestion>")]
pub async fn create_new_bind_suggestion(
    db: &State<JsonDatabase>,
    new_bind_suggestion: Json<NewBindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
    db.create_bind_suggestion(
        new_bind_suggestion.author.clone(),
        new_bind_suggestion.text.clone(),
        new_bind_suggestion.proposed_by.clone(),
    ).await
    .map(Json)
    .map_err(|e| {
        eprintln!("Error creating bind suggestion: {}", e);
        if e.contains("already exists") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })
}

#[post("/bind_suggestions/deleteBindSuggestion", data = "<bind_suggestion_to_delete>")]
pub async fn delete_existing_bind_suggestion(
    db: &State<JsonDatabase>,
    bind_suggestion_to_delete: Json<DelBindSuggestion>,
) -> Result<Json<usize>, Status> {
    db.delete_bind_suggestion(bind_suggestion_to_delete.id).await
        .map(|_| Json(1))
        .map_err(|e| {
            eprintln!("Error deleting bind suggestion: {}", e);
            Status::NotFound
        })
}

#[post("/bind_suggestions/updateBindSuggestion", data = "<bind_suggestion_to_update>")]
pub async fn update_existing_bind_suggestion(
    db: &State<JsonDatabase>,
    bind_suggestion_to_update: Json<BindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
    db.update_bind_suggestion(
        bind_suggestion_to_update.id,
        bind_suggestion_to_update.author.clone(),
        bind_suggestion_to_update.text.clone(),
        bind_suggestion_to_update.proposed_by.clone(),
    ).await
    .map(Json)
    .map_err(|e| {
        eprintln!("Error updating bind suggestion: {}", e);
        if e.contains("not found") {
            Status::NotFound
        } else if e.contains("already exists") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })
}

#[options("/bind_suggestions/addBindSuggestion")]
pub fn options_create_new_bind_suggestion() -> Status {
    Status::Ok
}

#[options("/bind_suggestions/deleteBindSuggestion")]
pub fn options_delete_existing_bind_suggestion() -> Status {
    Status::Ok
}

#[options("/bind_suggestions/updateBindSuggestion")]
pub fn options_update_existing_bind_suggestion() -> Status {
    Status::Ok
}

// --- Commands ---

#[get("/commands/<command_id>")]
pub fn get_command_by_id(db: &State<JsonDatabase>, command_id: i32) -> Result<Json<Command>, Status> {
    db.get_command(command_id)
        .map(Json)
        .map_err(|e| {
            eprintln!("Error getting command: {}", e);
            Status::NotFound
        })
}

#[get("/commands/getCommands")]
pub fn get_all_commands(db: &State<JsonDatabase>) -> Result<Json<Vec<Command>>, Status> {
    db.get_all_commands()
        .map(Json)
        .map_err(|e| {
            eprintln!("Error getting commands: {}", e);
            Status::InternalServerError
        })
}

#[post("/commands/addCommand", data = "<new_command>")]
pub async fn create_new_command(db: &State<JsonDatabase>, new_command: Json<NewCommand>) -> Result<Json<Command>, Status> {
    db.create_command(new_command.command.clone(), new_command.description.clone()).await
        .map(Json)
        .map_err(|e| {
            eprintln!("Error creating command: {}", e);
            if e.contains("already exists") {
                Status::Conflict
            } else {
                Status::InternalServerError
            }
        })
}

#[post("/commands/deleteCommand", data = "<command_to_delete>")]
pub async fn delete_existing_command(db: &State<JsonDatabase>, command_to_delete: Json<DelCommand>) -> Result<Json<usize>, Status> {
    db.delete_command(command_to_delete.id).await
        .map(|_| Json(1))
        .map_err(|e| {
            eprintln!("Error deleting command: {}", e);
            Status::NotFound
        })
}

#[post("/commands/updateCommand", data = "<command_to_update>")]
pub async fn update_existing_command(db: &State<JsonDatabase>, command_to_update: Json<Command>) -> Result<Json<Command>, Status> {
    db.update_command(
        command_to_update.id,
        command_to_update.command.clone(),
        command_to_update.description.clone(),
    ).await
    .map(Json)
    .map_err(|e| {
        eprintln!("Error updating command: {}", e);
        if e.contains("not found") {
            Status::NotFound
        } else if e.contains("already exists") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })
}

#[options("/commands/addCommand")]
pub fn options_create_new_command() -> Status {
    Status::Ok
}

#[options("/commands/deleteCommand")]
pub fn options_delete_existing_command() -> Status {
    Status::Ok
}

#[options("/commands/updateCommand")]
pub fn options_update_existing_command() -> Status {
    Status::Ok
}

// --- Bind Votings ---

#[get("/bind_votings/<bind_voting_id>")]
pub fn get_bind_voting_by_id(db: &State<JsonDatabase>, bind_voting_id: i32) -> Result<Json<BindVoting>, Status> {
    // Flatten all votes from all binds and find the specific one
    let binds = db.get_all_binds().map_err(|e| {
        eprintln!("Error getting binds: {}", e);
        Status::InternalServerError
    })?;
    
    let mut vote_id = 0;
    for bind in binds {
        for steam_id in &bind.upvote {
            vote_id += 1;
            if vote_id == bind_voting_id {
                return Ok(Json(BindVoting {
                    id: vote_id,
                    voter_steam_id: steam_id.clone(),
                    voted_bind_id: bind.id,
                    vote: "Upvote".to_string(),
                }));
            }
        }
        for steam_id in &bind.downvote {
            vote_id += 1;
            if vote_id == bind_voting_id {
                return Ok(Json(BindVoting {
                    id: vote_id,
                    voter_steam_id: steam_id.clone(),
                    voted_bind_id: bind.id,
                    vote: "Downvote".to_string(),
                }));
            }
        }
    }
    
    Err(Status::NotFound)
}

#[get("/bind_votings/getBindVotings")]
pub fn get_all_bind_votings(db: &State<JsonDatabase>) -> Result<Json<Vec<BindVoting>>, Status> {
    // Flatten all votes from all binds into BindVoting objects
    let binds = db.get_all_binds().map_err(|e| {
        eprintln!("Error getting binds: {}", e);
        Status::InternalServerError
    })?;
    
    let mut votings = Vec::new();
    let mut vote_id = 0;
    
    for bind in binds {
        for steam_id in bind.upvote {
            vote_id += 1;
            votings.push(BindVoting {
                id: vote_id,
                voter_steam_id: steam_id,
                voted_bind_id: bind.id,
                vote: "Upvote".to_string(),
            });
        }
        for steam_id in bind.downvote {
            vote_id += 1;
            votings.push(BindVoting {
                id: vote_id,
                voter_steam_id: steam_id,
                voted_bind_id: bind.id,
                vote: "Downvote".to_string(),
            });
        }
    }
    
    Ok(Json(votings))
}

#[post("/bind_votings/addBindVoting", data = "<new_bind_voting>")]
pub async fn create_new_bind_voting(
    db: &State<JsonDatabase>,
    new_bind_voting: Json<NewBindVoting>,
) -> Result<Json<BindVoting>, Status> {
    db.add_vote(
        new_bind_voting.voted_bind_id,
        new_bind_voting.voter_steam_id,
        &new_bind_voting.vote,
    ).await
    .map_err(|e| {
        eprintln!("Error adding vote: {}", e);
        if e.contains("not found") {
            Status::NotFound
        } else {
            Status::InternalServerError
        }
    })?;
    
    // Return a synthetic BindVoting object
    Ok(Json(BindVoting {
        id: 0, // ID is synthetic since we don't store individual vote records
        voter_steam_id: new_bind_voting.voter_steam_id,
        voted_bind_id: new_bind_voting.voted_bind_id,
        vote: new_bind_voting.vote.clone(),
    }))
}

#[post("/bind_votings/deleteBindVoting", data = "<bind_voting_to_delete>")]
pub async fn delete_existing_bind_voting(
    db: &State<JsonDatabase>,
    bind_voting_to_delete: Json<DelBindVotingByUser>,
) -> Result<Json<usize>, Status> {
    db.remove_vote(
        bind_voting_to_delete.voted_bind_id,
        bind_voting_to_delete.voter_steam_id,
    ).await
    .map(|_| Json(1))
    .map_err(|e| {
        eprintln!("Error removing vote: {}", e);
        if e.contains("not found") {
            Status::NotFound
        } else {
            Status::InternalServerError
        }
    })
}

#[options("/bind_votings/addBindVoting")]
pub fn options_create_new_bind_voting() -> Status {
    Status::Ok
}

#[options("/bind_votings/deleteBindVoting")]
pub fn options_delete_existing_bind_voting() -> Status {
    Status::Ok
}

// --- By ID (PUT endpoints) ---

#[put("/binds/<bind_id>", data = "<bind_to_update>")]
pub async fn update_existing_bind_by_id(
    db: &State<JsonDatabase>,
    bind_id: i32,
    bind_to_update: Json<Bind>,
) -> Result<Json<Bind>, Status> {
    db.update_bind(
        bind_id,
        bind_to_update.author.clone(),
        bind_to_update.text.clone(),
        bind_to_update.upvote.clone(),
        bind_to_update.downvote.clone(),
    ).await
    .map(Json)
    .map_err(|e| {
        eprintln!("Error updating bind by ID: {}", e);
        if e.contains("not found") {
            Status::NotFound
        } else if e.contains("already exists") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })
}

#[put("/bind_suggestions/<bind_suggestion_id>", data = "<bind_suggestion_to_update>")]
pub async fn update_existing_bind_suggestion_by_id(
    db: &State<JsonDatabase>,
    bind_suggestion_id: i32,
    bind_suggestion_to_update: Json<BindSuggestion>,
) -> Result<Json<BindSuggestion>, Status> {
    db.update_bind_suggestion(
        bind_suggestion_id,
        bind_suggestion_to_update.author.clone(),
        bind_suggestion_to_update.text.clone(),
        bind_suggestion_to_update.proposed_by.clone(),
    ).await
    .map(Json)
    .map_err(|e| {
        eprintln!("Error updating bind suggestion by ID: {}", e);
        if e.contains("not found") {
            Status::NotFound
        } else if e.contains("already exists") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })
}

#[put("/commands/<command_id>", data = "<command_to_update>")]
pub async fn update_existing_command_by_id(
    db: &State<JsonDatabase>,
    command_id: i32,
    command_to_update: Json<Command>,
) -> Result<Json<Command>, Status> {
    db.update_command(
        command_id,
        command_to_update.command.clone(),
        command_to_update.description.clone(),
    ).await
    .map(Json)
    .map_err(|e| {
        eprintln!("Error updating command by ID: {}", e);
        if e.contains("not found") {
            Status::NotFound
        } else if e.contains("already exists") {
            Status::Conflict
        } else {
            Status::InternalServerError
        }
    })
}

// --- Admin verification ---

#[post("/admin/verify", data = "<request>")]
pub fn verify_admin(
    config: &State<Config>,
    request: Json<VerifyAdminRequest>,
) -> Result<Json<VerifyAdminResponse>, Status> {
    let is_admin = config.is_admin(request.steam_id);
    Ok(Json(VerifyAdminResponse { is_admin }))
}

// --- Mount the routes ---
pub fn mount_json_routes() -> Vec<rocket::Route> {
    routes![
        // Binds
        get_bind_by_id,
        get_all_binds,
        create_new_bind,
        delete_existing_bind,
        update_existing_bind,
        // Bind suggestions
        get_bind_suggestion_by_id,
        get_all_bind_suggestions,
        create_new_bind_suggestion,
        delete_existing_bind_suggestion,
        update_existing_bind_suggestion,
        // Commands
        get_command_by_id,
        get_all_commands,
        create_new_command,
        delete_existing_command,
        update_existing_command,
        // Bind votings
        get_bind_voting_by_id,
        get_all_bind_votings,
        create_new_bind_voting,
        delete_existing_bind_voting,
        // By ID (PUT endpoints)
        update_existing_bind_by_id,
        update_existing_bind_suggestion_by_id,
        update_existing_command_by_id,
        // Options (CORS)
        options_create_new_bind,
        options_delete_existing_bind,
        options_update_existing_bind,
        options_create_new_bind_suggestion,
        options_delete_existing_bind_suggestion,
        options_update_existing_bind_suggestion,
        options_create_new_command,
        options_delete_existing_command,
        options_update_existing_command,
        options_create_new_bind_voting,
        options_delete_existing_bind_voting,
        // Admin verification
        verify_admin,
    ]
}

